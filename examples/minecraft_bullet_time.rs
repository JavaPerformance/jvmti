//! Control Minecraft "bullet time" by holding F8 and using the mouse wheel.
//!
//! This playful diagnostic agent observes the game's Java-side keyboard and
//! scroll callbacks, so it does not need Windows, macOS, X11, or Wayland input
//! APIs. Hold F8 and scroll down to add delay at the selected tick method;
//! hold F8 and scroll up to remove it. Armed wheel input is consumed before
//! Minecraft can use it for normal hotbar scrolling.
//!
//! Defaults target illustrative Mojang-mapped client names. Override every
//! mapping-sensitive value for the exact game, loader, and mapping set:
//!
//! `tick_class=L...;,tick_method=runTick,tick_signature=(Z)V,`
//! `scroll_class=L...;,scroll_method=onScroll,scroll_signature=(JDD)V,`
//! `keyboard_class=L...;,keyboard_method=keyPress,keyboard_signature=(JIIII)V,`
//! `scroll_slot=5,keyboard_key_slot=3,keyboard_action_slot=5,activation_key=297,`
//! `step_ms=10,max_delay_ms=250,initial_delay_ms=0`
//!
//! `scroll_slot` is the local-variable slot containing the vertical `double`
//! wheel delta at method entry. The default activation key is GLFW F8. This
//! example uses only public `jvmti-bindings` APIs and is intentionally not
//! production instrumentation.

use jvmti_bindings::prelude::*;
use std::ffi::c_void;
use std::ptr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::time::Duration;

struct MethodTarget {
    class_signature: String,
    method_name: String,
    method_signature: String,
}

struct BulletTimeConfig {
    tick: MethodTarget,
    scroll: MethodTarget,
    keyboard: MethodTarget,
    scroll_slot: jni::jint,
    keyboard_key_slot: jni::jint,
    keyboard_action_slot: jni::jint,
    activation_key: jni::jint,
    step_ms: u64,
    max_delay_ms: u64,
}

#[derive(Default)]
struct MinecraftBulletTime {
    config: OnceLock<BulletTimeConfig>,
    tick_method: AtomicPtr<c_void>,
    scroll_method: AtomicPtr<c_void>,
    keyboard_method: AtomicPtr<c_void>,
    activation_key_held: AtomicBool,
    delay_ms: AtomicU64,
    ticks: AtomicU64,
    scroll_events: AtomicU64,
    scroll_read_errors: AtomicU64,
    scroll_consume_errors: AtomicU64,
    keyboard_read_errors: AtomicU64,
}

fn option<'a>(options: &'a str, key: &str) -> Option<&'a str> {
    options
        .split(',')
        .find_map(|part| part.trim().strip_prefix(key))
}

fn text_option(options: &str, key: &str, default: &str) -> String {
    option(options, key).unwrap_or(default).to_owned()
}

fn u64_option(options: &str, key: &str, default: u64) -> u64 {
    option(options, key)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn jint_option(options: &str, key: &str, default: jni::jint) -> jni::jint {
    option(options, key)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn adjusted_delay(current: u64, delta: f64, step_ms: u64, max_delay_ms: u64) -> u64 {
    if !delta.is_finite() || delta == 0.0 {
        return current.min(max_delay_ms);
    }
    let wheel_steps = delta.abs().ceil() as u64;
    let adjustment = step_ms.saturating_mul(wheel_steps.max(1));
    if delta > 0.0 {
        current.saturating_sub(adjustment)
    } else {
        current.saturating_add(adjustment).min(max_delay_ms)
    }
}

fn updated_key_state(
    current: bool,
    key: jni::jint,
    action: jni::jint,
    activation_key: jni::jint,
) -> bool {
    const GLFW_RELEASE: jni::jint = 0;
    const GLFW_PRESS: jni::jint = 1;
    const GLFW_REPEAT: jni::jint = 2;

    if key != activation_key {
        return current;
    }
    match action {
        GLFW_RELEASE => false,
        GLFW_PRESS | GLFW_REPEAT => true,
        _ => current,
    }
}

impl MinecraftBulletTime {
    fn install_breakpoint(
        &self,
        context: &CallbackContext<'_>,
        class: jni::jclass,
        target: &MethodTarget,
        installed_method: &AtomicPtr<c_void>,
    ) {
        if !installed_method.load(Ordering::Acquire).is_null() {
            return;
        }
        let Ok(methods) = (unsafe { context.jvmti().get_class_methods(class) }) else {
            return;
        };
        for method in methods {
            let Ok((name, signature, _)) = (unsafe { context.jvmti().get_method_name(method) })
            else {
                continue;
            };
            if name != target.method_name || signature != target.method_signature {
                continue;
            }
            let Ok((start, _)) = (unsafe { context.jvmti().get_method_location(method) }) else {
                continue;
            };
            let method_ptr = method.cast::<c_void>();
            if installed_method
                .compare_exchange(
                    ptr::null_mut(),
                    method_ptr,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_err()
            {
                return;
            }
            if unsafe { context.jvmti().set_breakpoint(method, start) }.is_err() {
                let _ = installed_method.compare_exchange(
                    method_ptr,
                    ptr::null_mut(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
            }
            return;
        }
    }
}

impl Agent for MinecraftBulletTime {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let options = context.options_str().ok().flatten().unwrap_or_default();
        let max_delay_ms = u64_option(&options, "max_delay_ms=", 250);
        let initial_delay_ms = u64_option(&options, "initial_delay_ms=", 0).min(max_delay_ms);
        let _ = self.config.set(BulletTimeConfig {
            tick: MethodTarget {
                class_signature: text_option(
                    &options,
                    "tick_class=",
                    "Lnet/minecraft/client/Minecraft;",
                ),
                method_name: text_option(&options, "tick_method=", "runTick"),
                method_signature: text_option(&options, "tick_signature=", "(Z)V"),
            },
            scroll: MethodTarget {
                class_signature: text_option(
                    &options,
                    "scroll_class=",
                    "Lnet/minecraft/client/MouseHandler;",
                ),
                method_name: text_option(&options, "scroll_method=", "onScroll"),
                method_signature: text_option(&options, "scroll_signature=", "(JDD)V"),
            },
            keyboard: MethodTarget {
                class_signature: text_option(
                    &options,
                    "keyboard_class=",
                    "Lnet/minecraft/client/KeyboardHandler;",
                ),
                method_name: text_option(&options, "keyboard_method=", "keyPress"),
                method_signature: text_option(&options, "keyboard_signature=", "(JIIII)V"),
            },
            scroll_slot: jint_option(&options, "scroll_slot=", 5),
            keyboard_key_slot: jint_option(&options, "keyboard_key_slot=", 3),
            keyboard_action_slot: jint_option(&options, "keyboard_action_slot=", 5),
            activation_key: jint_option(&options, "activation_key=", 297),
            step_ms: u64_option(&options, "step_ms=", 10),
            max_delay_ms,
        });
        self.delay_ms.store(initial_delay_ms, Ordering::Release);

        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti
            .add_capabilities_with(|caps| {
                caps.set_can_generate_breakpoint_events(true);
                caps.set_can_access_local_variables(true);
            })
            .is_err()
            || jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[
                    jvmti::JVMTI_EVENT_CLASS_PREPARE,
                    jvmti::JVMTI_EVENT_BREAKPOINT,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn class_prepare(&self, context: CallbackContext<'_>, event: ClassEvent) {
        let Some(config) = self.config.get() else {
            return;
        };
        let Ok((signature, _)) = (unsafe { context.jvmti().get_class_signature(event.class()) })
        else {
            return;
        };
        if signature == config.tick.class_signature {
            self.install_breakpoint(&context, event.class(), &config.tick, &self.tick_method);
        }
        if signature == config.scroll.class_signature {
            self.install_breakpoint(&context, event.class(), &config.scroll, &self.scroll_method);
        }
        if signature == config.keyboard.class_signature {
            self.install_breakpoint(
                &context,
                event.class(),
                &config.keyboard,
                &self.keyboard_method,
            );
        }
    }

    fn breakpoint(&self, context: CallbackContext<'_>, event: LocationEvent) {
        let method = event.method().cast::<c_void>();
        if method == self.keyboard_method.load(Ordering::Acquire) {
            let Some(config) = self.config.get() else {
                return;
            };
            let key = unsafe {
                context
                    .jvmti()
                    .get_local_int(event.thread(), 0, config.keyboard_key_slot)
            };
            let action = unsafe {
                context
                    .jvmti()
                    .get_local_int(event.thread(), 0, config.keyboard_action_slot)
            };
            match (key, action) {
                (Ok(key), Ok(action)) => {
                    let current = self.activation_key_held.load(Ordering::Acquire);
                    self.activation_key_held.store(
                        updated_key_state(current, key, action, config.activation_key),
                        Ordering::Release,
                    );
                }
                _ => {
                    self.keyboard_read_errors.fetch_add(1, Ordering::Relaxed);
                }
            }
            return;
        }

        if method == self.scroll_method.load(Ordering::Acquire) {
            let Some(config) = self.config.get() else {
                return;
            };
            if !self.activation_key_held.load(Ordering::Acquire) {
                return;
            }
            // The configured signature and slot must identify a live `double`
            // local in this current breakpoint frame.
            let delta = unsafe {
                context
                    .jvmti()
                    .get_local_double(event.thread(), 0, config.scroll_slot)
            };
            match delta {
                Ok(delta) => {
                    // Consume armed wheel input so the same gesture does not
                    // also change the selected hotbar slot.
                    if unsafe {
                        context
                            .jvmti()
                            .set_local_double(event.thread(), 0, config.scroll_slot, 0.0)
                    }
                    .is_err()
                    {
                        self.scroll_consume_errors.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                    self.scroll_events.fetch_add(1, Ordering::Relaxed);
                    let _ = self.delay_ms.fetch_update(
                        Ordering::AcqRel,
                        Ordering::Acquire,
                        |current| {
                            Some(adjusted_delay(
                                current,
                                delta,
                                config.step_ms,
                                config.max_delay_ms,
                            ))
                        },
                    );
                }
                Err(_) => {
                    self.scroll_read_errors.fetch_add(1, Ordering::Relaxed);
                }
            }
            return;
        }

        if method == self.tick_method.load(Ordering::Acquire) {
            self.ticks.fetch_add(1, Ordering::Relaxed);
            let delay_ms = self.delay_ms.load(Ordering::Acquire);
            if delay_ms != 0 {
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
        }
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[minecraft-bullet-time] tick_installed={} scroll_installed={} keyboard_installed={} activation_key_held={} delay_ms={} ticks={} scroll_events={} scroll_read_errors={} scroll_consume_errors={} keyboard_read_errors={}",
            !self.tick_method.load(Ordering::Acquire).is_null(),
            !self.scroll_method.load(Ordering::Acquire).is_null(),
            !self.keyboard_method.load(Ordering::Acquire).is_null(),
            self.activation_key_held.load(Ordering::Acquire),
            self.delay_ms.load(Ordering::Acquire),
            self.ticks.load(Ordering::Relaxed),
            self.scroll_events.load(Ordering::Relaxed),
            self.scroll_read_errors.load(Ordering::Relaxed),
            self.scroll_consume_errors.load(Ordering::Relaxed),
            self.keyboard_read_errors.load(Ordering::Relaxed)
        );
    }
}

export_agent!(MinecraftBulletTime);

#[cfg(test)]
mod tests {
    use super::{adjusted_delay, updated_key_state};

    #[test]
    fn wheel_direction_and_bounds_control_delay() {
        assert_eq!(adjusted_delay(50, 1.0, 10, 250), 40);
        assert_eq!(adjusted_delay(50, -2.0, 10, 250), 70);
        assert_eq!(adjusted_delay(5, 1.0, 10, 250), 0);
        assert_eq!(adjusted_delay(245, -1.0, 10, 250), 250);
        assert_eq!(adjusted_delay(50, f64::NAN, 10, 250), 50);
    }

    #[test]
    fn only_the_activation_key_changes_armed_state() {
        const F8: i32 = 297;
        assert!(!updated_key_state(false, 65, 1, F8));
        assert!(updated_key_state(false, F8, 1, F8));
        assert!(updated_key_state(true, F8, 2, F8));
        assert!(!updated_key_state(true, F8, 0, F8));
        assert!(updated_key_state(true, F8, 99, F8));
    }
}
