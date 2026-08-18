//! Count calls to a selected Minecraft server tick method with one breakpoint.
//!
//! Defaults are illustrative because class and method names differ by game
//! version and mapping set. Override both:
//! `class=Lnet/minecraft/server/MinecraftServer;,method=tickServer`.
//! This is a standalone public-API example, not production instrumentation.

use jvmti_bindings::prelude::*;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

struct TickConfig {
    class_signature: String,
    method_name: String,
}

#[derive(Default)]
struct MinecraftTickBreakpoint {
    config: OnceLock<TickConfig>,
    installed: AtomicBool,
    ticks: AtomicU64,
}

fn option(options: &str, key: &str, default: &str) -> String {
    options
        .split(',')
        .find_map(|part| part.strip_prefix(key))
        .unwrap_or(default)
        .to_owned()
}

impl Agent for MinecraftTickBreakpoint {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let options = context.options_str().ok().flatten().unwrap_or_default();
        let _ = self.config.set(TickConfig {
            class_signature: option(&options, "class=", "Lnet/minecraft/server/MinecraftServer;"),
            method_name: option(&options, "method=", "tickServer"),
        });

        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti
            .add_capabilities_with(|caps| caps.set_can_generate_breakpoint_events(true))
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
        if self.installed.load(Ordering::Acquire) {
            return;
        }
        let Some(config) = self.config.get() else {
            return;
        };
        let Ok((signature, _)) = (unsafe { context.jvmti().get_class_signature(event.class()) })
        else {
            return;
        };
        if signature != config.class_signature {
            return;
        }
        let Ok(methods) = (unsafe { context.jvmti().get_class_methods(event.class()) }) else {
            return;
        };
        for method in methods {
            let Ok((name, _, _)) = (unsafe { context.jvmti().get_method_name(method) }) else {
                continue;
            };
            if name != config.method_name {
                continue;
            }
            let Ok((start, _)) = (unsafe { context.jvmti().get_method_location(method) }) else {
                continue;
            };
            if unsafe { context.jvmti().set_breakpoint(method, start) }.is_ok() {
                self.installed.store(true, Ordering::Release);
                break;
            }
        }
    }

    fn breakpoint(&self, _context: CallbackContext<'_>, _event: LocationEvent) {
        self.ticks.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        eprintln!(
            "[minecraft-ticks] installed={} ticks={}",
            self.installed.load(Ordering::Acquire),
            self.ticks.load(Ordering::Relaxed)
        );
    }
}

export_agent!(MinecraftTickBreakpoint);
