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
//! example uses only public `jvmti-bindings` APIs. It intentionally sleeps the
//! game thread from a permanent breakpoint to make the effect obvious; that
//! can inhibit compilation and interact badly with safepoints or other agents.
//! Treat it as a bounded diagnostic toy, not production instrumentation.
//! Dynamic attach is attempted only when the live JVM advertises both required
//! capabilities. HotSpot normally makes them unavailable after `OnLoad`, so
//! use startup `-agentpath` there; an unsupported attach fails with diagnostics.

use jvmti_bindings::prelude::*;
use std::collections::HashSet;
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

struct ParsedOptions {
    config: BulletTimeConfig,
    initial_delay_ms: u64,
}

#[derive(Default)]
struct LocalAccessErrors {
    invalid_slot: AtomicU64,
    type_mismatch: AtomicU64,
    opaque_frame: AtomicU64,
    other: AtomicU64,
}

impl LocalAccessErrors {
    fn record(&self, error: jvmti::jvmtiError) {
        let counter = if error == jvmti::jvmtiError::INVALID_SLOT {
            &self.invalid_slot
        } else if error == jvmti::jvmtiError::TYPE_MISMATCH {
            &self.type_mismatch
        } else if error == jvmti::jvmtiError::OPAQUE_FRAME {
            &self.opaque_frame
        } else {
            &self.other
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct MinecraftBulletTime {
    config: OnceLock<BulletTimeConfig>,
    runtime_configured: AtomicBool,
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
    local_access_errors: LocalAccessErrors,
}

fn parse_u64(key: &str, value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|_| format!("invalid unsigned integer for {key}: {value:?}"))
}

fn parse_jint(key: &str, value: &str) -> Result<jni::jint, String> {
    value
        .parse()
        .map_err(|_| format!("invalid integer for {key}: {value:?}"))
}

fn parse_options(options: &str) -> Result<ParsedOptions, String> {
    let mut tick_class = "Lnet/minecraft/client/Minecraft;".to_owned();
    let mut tick_method = "runTick".to_owned();
    let mut tick_signature = "(Z)V".to_owned();
    let mut scroll_class = "Lnet/minecraft/client/MouseHandler;".to_owned();
    let mut scroll_method = "onScroll".to_owned();
    let mut scroll_signature = "(JDD)V".to_owned();
    let mut keyboard_class = "Lnet/minecraft/client/KeyboardHandler;".to_owned();
    let mut keyboard_method = "keyPress".to_owned();
    let mut keyboard_signature = "(JIIII)V".to_owned();
    let mut scroll_slot = 5;
    let mut keyboard_key_slot = 3;
    let mut keyboard_action_slot = 5;
    let mut activation_key = 297;
    let mut step_ms = 10;
    let mut max_delay_ms = 250;
    let mut initial_delay_ms = 0;
    let mut seen = HashSet::new();

    for raw in options.split(',') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let Some((key, value)) = raw.split_once('=') else {
            return Err(format!("option must use key=value syntax: {raw:?}"));
        };
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            return Err(format!("option key and value must be non-empty: {raw:?}"));
        }
        if !seen.insert(key) {
            return Err(format!("duplicate option: {key}"));
        }
        match key {
            "tick_class" => tick_class = value.to_owned(),
            "tick_method" => tick_method = value.to_owned(),
            "tick_signature" => tick_signature = value.to_owned(),
            "scroll_class" => scroll_class = value.to_owned(),
            "scroll_method" => scroll_method = value.to_owned(),
            "scroll_signature" => scroll_signature = value.to_owned(),
            "keyboard_class" => keyboard_class = value.to_owned(),
            "keyboard_method" => keyboard_method = value.to_owned(),
            "keyboard_signature" => keyboard_signature = value.to_owned(),
            "scroll_slot" => scroll_slot = parse_jint(key, value)?,
            "keyboard_key_slot" => keyboard_key_slot = parse_jint(key, value)?,
            "keyboard_action_slot" => keyboard_action_slot = parse_jint(key, value)?,
            "activation_key" => activation_key = parse_jint(key, value)?,
            "step_ms" => step_ms = parse_u64(key, value)?,
            "max_delay_ms" => max_delay_ms = parse_u64(key, value)?,
            "initial_delay_ms" => initial_delay_ms = parse_u64(key, value)?,
            _ => return Err(format!("unknown option: {key}")),
        }
    }

    for (key, slot) in [
        ("scroll_slot", scroll_slot),
        ("keyboard_key_slot", keyboard_key_slot),
        ("keyboard_action_slot", keyboard_action_slot),
    ] {
        if slot < 0 {
            return Err(format!("{key} must be non-negative"));
        }
    }
    if activation_key < 0 {
        return Err("activation_key must be non-negative".to_owned());
    }
    initial_delay_ms = initial_delay_ms.min(max_delay_ms);

    Ok(ParsedOptions {
        config: BulletTimeConfig {
            tick: MethodTarget {
                class_signature: tick_class,
                method_name: tick_method,
                method_signature: tick_signature,
            },
            scroll: MethodTarget {
                class_signature: scroll_class,
                method_name: scroll_method,
                method_signature: scroll_signature,
            },
            keyboard: MethodTarget {
                class_signature: keyboard_class,
                method_name: keyboard_method,
                method_signature: keyboard_signature,
            },
            scroll_slot,
            keyboard_key_slot,
            keyboard_action_slot,
            activation_key,
            step_ms,
            max_delay_ms,
        },
        initial_delay_ms,
    })
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

fn current_jni(context: &AgentLoadContext<'_>) -> Result<JniEnv, jni::jint> {
    let mut raw_env: *mut c_void = ptr::null_mut();
    let result = unsafe {
        jvmti_bindings::jvm_call!(
            context.vm().raw(),
            GetEnv,
            &mut raw_env,
            jni::JNI_VERSION_1_6
        )
    };
    if result != jni::JNI_OK {
        return Err(result);
    }
    if raw_env.is_null() {
        return Err(jni::JNI_ERR);
    }
    // SAFETY: Agent_OnAttach executes on an attached JVM thread and GetEnv
    // returned this thread's live JNI environment for the requested version.
    Ok(unsafe { JniEnv::from_raw(raw_env.cast()) })
}

impl MinecraftBulletTime {
    fn configure_options(&self, context: &AgentLoadContext<'_>) -> Result<(), ()> {
        if self.config.get().is_some() {
            return Ok(());
        }
        let options = match context.options_str() {
            Ok(Some(options)) => options,
            Ok(None) => "".into(),
            Err(error) => {
                eprintln!("[minecraft-bullet-time] invalid Modified UTF-8 options: {error}");
                return Err(());
            }
        };
        let parsed = match parse_options(&options) {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!("[minecraft-bullet-time] invalid options: {error}");
                return Err(());
            }
        };
        let initial_delay_ms = parsed.initial_delay_ms;
        if self.config.set(parsed.config).is_ok() {
            self.delay_ms.store(initial_delay_ms, Ordering::Release);
        }
        Ok(())
    }

    fn configure_runtime(
        &self,
        context: &AgentLoadContext<'_>,
        startup: bool,
    ) -> Result<Jvmti, ()> {
        self.configure_options(context)?;
        let jvmti = context.vm().jvmti().map_err(|error| {
            eprintln!(
                "[minecraft-bullet-time] unable to acquire JVM TI: {}",
                describe_jni_result(error)
            );
        })?;
        let phase = jvmti.get_phase().unwrap_or(0);
        let potential = match jvmti.get_potential_capabilities() {
            Ok(potential) => potential,
            Err(error) => {
                eprintln!(
                    "[minecraft-bullet-time] GetPotentialCapabilities failed mode={} phase={phase}: {error}",
                    if startup { "startup" } else { "attach" }
                );
                return Err(());
            }
        };
        if !potential.can_generate_breakpoint_events() || !potential.can_access_local_variables() {
            eprintln!(
                "[minecraft-bullet-time] required capabilities unavailable mode={} phase={phase} breakpoint={} local_variables={}; use startup -agentpath on this JVM",
                if startup { "startup" } else { "attach" },
                potential.can_generate_breakpoint_events(),
                potential.can_access_local_variables()
            );
            return Err(());
        }
        if let Err(error) = jvmti.add_capabilities_with(|caps| {
            caps.set_can_generate_breakpoint_events(true);
            caps.set_can_access_local_variables(true);
        }) {
            eprintln!("[minecraft-bullet-time] AddCapabilities failed: {error}");
            return Err(());
        }
        if let Err(error) = jvmti.set_default_agent_callbacks() {
            eprintln!("[minecraft-bullet-time] SetEventCallbacks failed: {error}");
            return Err(());
        }
        let startup_events = [
            jvmti::JVMTI_EVENT_VM_INIT,
            jvmti::JVMTI_EVENT_CLASS_PREPARE,
            jvmti::JVMTI_EVENT_BREAKPOINT,
            jvmti::JVMTI_EVENT_VM_DEATH,
        ];
        let attach_events = [
            jvmti::JVMTI_EVENT_CLASS_PREPARE,
            jvmti::JVMTI_EVENT_BREAKPOINT,
            jvmti::JVMTI_EVENT_VM_DEATH,
        ];
        let events = if startup {
            startup_events.as_slice()
        } else {
            attach_events.as_slice()
        };
        if let Err(error) = jvmti.enable_events_global(events) {
            eprintln!("[minecraft-bullet-time] event enablement failed: {error}");
            return Err(());
        }
        eprintln!(
            "[minecraft-bullet-time] callbacks configured mode={}",
            if startup { "startup" } else { "attach" }
        );
        Ok(jvmti)
    }

    fn install_breakpoint(
        &self,
        jvmti: &Jvmti,
        class: jni::jclass,
        label: &str,
        target: &MethodTarget,
        installed_method: &AtomicPtr<c_void>,
        source: &str,
    ) {
        if !installed_method.load(Ordering::Acquire).is_null() {
            return;
        }
        let methods = match unsafe { jvmti.get_class_methods(class) } {
            Ok(methods) => methods,
            Err(error) => {
                eprintln!(
                    "[minecraft-bullet-time] {label} GetClassMethods failed source={source}: {error}"
                );
                return;
            }
        };
        for method in methods {
            let Ok((name, signature, _)) = (unsafe { jvmti.get_method_name(method) }) else {
                continue;
            };
            if name != target.method_name || signature != target.method_signature {
                continue;
            }
            let start = match unsafe { jvmti.get_method_location(method) } {
                Ok((start, _)) => start,
                Err(error) => {
                    eprintln!(
                        "[minecraft-bullet-time] {label} GetMethodLocation failed source={source}: {error}"
                    );
                    return;
                }
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
            if let Err(error) = unsafe { jvmti.set_breakpoint(method, start) } {
                let _ = installed_method.compare_exchange(
                    method_ptr,
                    ptr::null_mut(),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
                eprintln!(
                    "[minecraft-bullet-time] {label} SetBreakpoint failed source={source}: {error}"
                );
            } else {
                eprintln!(
                    "[minecraft-bullet-time] installed {label} breakpoint source={source} class={} method={} signature={} location={start}",
                    target.class_signature, target.method_name, target.method_signature
                );
            }
            return;
        }
        eprintln!(
            "[minecraft-bullet-time] {label} method not found source={source} class={} method={} signature={}",
            target.class_signature, target.method_name, target.method_signature
        );
    }

    fn inspect_class(&self, jvmti: &Jvmti, class: jni::jclass, source: &str) {
        let Some(config) = self.config.get() else {
            return;
        };
        let Ok((signature, _)) = (unsafe { jvmti.get_class_signature(class) }) else {
            return;
        };
        if signature == config.tick.class_signature {
            self.install_breakpoint(
                jvmti,
                class,
                "tick",
                &config.tick,
                &self.tick_method,
                source,
            );
        }
        if signature == config.scroll.class_signature {
            self.install_breakpoint(
                jvmti,
                class,
                "scroll",
                &config.scroll,
                &self.scroll_method,
                source,
            );
        }
        if signature == config.keyboard.class_signature {
            self.install_breakpoint(
                jvmti,
                class,
                "keyboard",
                &config.keyboard,
                &self.keyboard_method,
                source,
            );
        }
    }

    fn scan_loaded_classes(
        &self,
        jvmti: &Jvmti,
        jni: &JniEnv,
        source: &str,
    ) -> Result<(), jvmti::jvmtiError> {
        let classes = jvmti.get_loaded_classes()?;
        let class_count = classes.len();
        for class in classes {
            // SAFETY: GetLoadedClasses returns JNI local references owned by
            // this current callback/thread; LocalRef releases each exactly once.
            let class = unsafe { LocalRef::from_raw(jni, class) };
            self.inspect_class(jvmti, class.get(), source);
        }
        eprintln!(
            "[minecraft-bullet-time] loaded-class scan source={source} classes={class_count}"
        );
        Ok(())
    }
}

impl Agent for MinecraftBulletTime {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        if self.configure_runtime(&context, true).is_err() {
            return jni::JNI_ERR;
        }
        self.runtime_configured.store(true, Ordering::Release);
        jni::JNI_OK
    }

    fn on_attach(&self, context: AgentLoadContext<'_>) -> jni::jint {
        if self.runtime_configured.load(Ordering::Acquire) {
            eprintln!("[minecraft-bullet-time] runtime already configured; attach is a no-op");
            return jni::JNI_OK;
        }
        let jni = match current_jni(&context) {
            Ok(jni) => jni,
            Err(error) => {
                eprintln!(
                    "[minecraft-bullet-time] attach GetEnv failed: {}",
                    describe_jni_result(error)
                );
                return jni::JNI_ERR;
            }
        };
        let Ok(jvmti) = self.configure_runtime(&context, false) else {
            return jni::JNI_ERR;
        };
        if let Err(error) = self.scan_loaded_classes(&jvmti, &jni, "attach") {
            eprintln!("[minecraft-bullet-time] attach GetLoadedClasses failed: {error}");
            return jni::JNI_ERR;
        }
        self.runtime_configured.store(true, Ordering::Release);
        jni::JNI_OK
    }

    fn vm_init(&self, context: CallbackContext<'_>, _event: ThreadEvent) {
        let Some(jni) = context.jni() else {
            eprintln!("[minecraft-bullet-time] VMInit did not provide JNI; scan skipped");
            return;
        };
        if let Err(error) = self.scan_loaded_classes(context.jvmti(), jni, "vm-init") {
            eprintln!("[minecraft-bullet-time] VMInit GetLoadedClasses failed: {error}");
        }
    }

    fn class_prepare(&self, context: CallbackContext<'_>, event: ClassEvent) {
        self.inspect_class(context.jvmti(), event.class(), "class-prepare");
    }

    fn breakpoint(&self, context: CallbackContext<'_>, event: LocationEvent) {
        let method = event.method().cast::<c_void>();
        if method == self.keyboard_method.load(Ordering::Acquire) {
            let Some(config) = self.config.get() else {
                return;
            };
            let key = match unsafe {
                context
                    .jvmti()
                    .get_local_int(event.thread(), 0, config.keyboard_key_slot)
            } {
                Ok(key) => key,
                Err(error) => {
                    self.keyboard_read_errors.fetch_add(1, Ordering::Relaxed);
                    self.local_access_errors.record(error);
                    return;
                }
            };
            let action = match unsafe {
                context
                    .jvmti()
                    .get_local_int(event.thread(), 0, config.keyboard_action_slot)
            } {
                Ok(action) => action,
                Err(error) => {
                    self.keyboard_read_errors.fetch_add(1, Ordering::Relaxed);
                    self.local_access_errors.record(error);
                    return;
                }
            };
            let current = self.activation_key_held.load(Ordering::Acquire);
            self.activation_key_held.store(
                updated_key_state(current, key, action, config.activation_key),
                Ordering::Release,
            );
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
                    if let Err(error) = unsafe {
                        context
                            .jvmti()
                            .set_local_double(event.thread(), 0, config.scroll_slot, 0.0)
                    } {
                        self.scroll_consume_errors.fetch_add(1, Ordering::Relaxed);
                        self.local_access_errors.record(error);
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
                Err(error) => {
                    self.scroll_read_errors.fetch_add(1, Ordering::Relaxed);
                    self.local_access_errors.record(error);
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
            "[minecraft-bullet-time] tick_installed={} scroll_installed={} keyboard_installed={} activation_key_held={} delay_ms={} ticks={} scroll_events={} scroll_read_errors={} scroll_consume_errors={} keyboard_read_errors={} local_invalid_slot={} local_type_mismatch={} local_opaque_frame={} local_other={}",
            !self.tick_method.load(Ordering::Acquire).is_null(),
            !self.scroll_method.load(Ordering::Acquire).is_null(),
            !self.keyboard_method.load(Ordering::Acquire).is_null(),
            self.activation_key_held.load(Ordering::Acquire),
            self.delay_ms.load(Ordering::Acquire),
            self.ticks.load(Ordering::Relaxed),
            self.scroll_events.load(Ordering::Relaxed),
            self.scroll_read_errors.load(Ordering::Relaxed),
            self.scroll_consume_errors.load(Ordering::Relaxed),
            self.keyboard_read_errors.load(Ordering::Relaxed),
            self.local_access_errors
                .invalid_slot
                .load(Ordering::Relaxed),
            self.local_access_errors
                .type_mismatch
                .load(Ordering::Relaxed),
            self.local_access_errors
                .opaque_frame
                .load(Ordering::Relaxed),
            self.local_access_errors.other.load(Ordering::Relaxed)
        );
    }
}

export_agent!(MinecraftBulletTime);

#[cfg(test)]
mod tests {
    use super::{adjusted_delay, parse_options, updated_key_state};

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

    #[test]
    fn options_are_exact_strict_and_bounded() {
        let parsed =
            parse_options("scroll_slot=7,activation_key=298,max_delay_ms=40,initial_delay_ms=99")
                .unwrap();
        assert_eq!(parsed.config.scroll_slot, 7);
        assert_eq!(parsed.config.activation_key, 298);
        assert_eq!(parsed.config.max_delay_ms, 40);
        assert_eq!(parsed.initial_delay_ms, 40);

        assert!(parse_options("tick_clas=wrong").is_err());
        assert!(parse_options("scroll_slot=-1").is_err());
        assert!(parse_options("step_ms=10,step_ms=20").is_err());
        assert!(parse_options("step_ms").is_err());
    }
}
