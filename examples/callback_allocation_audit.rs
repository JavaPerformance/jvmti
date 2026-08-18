//! Live proof that normal Rust callback dispatch does not touch the heap.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use jvmti_bindings::prelude::*;

struct CountingAllocator;

static TRACKING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static ALLOCATED_BYTES: AtomicU64 = AtomicU64::new(0);
static DEALLOCATIONS: AtomicU64 = AtomicU64::new(0);
static REALLOCATIONS: AtomicU64 = AtomicU64::new(0);

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACKING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if TRACKING.load(Ordering::Relaxed) {
            DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if TRACKING.load(Ordering::Relaxed) {
            REALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            ALLOCATED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[derive(Default)]
struct CallbackAllocationAudit {
    callbacks: AtomicU64,
}

impl Agent for CallbackAllocationAudit {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let jvmti = match context.vm().jvmti() {
            Ok(jvmti) => jvmti,
            Err(_) => return jni::JNI_ERR,
        };

        if jvmti
            .add_capabilities_with(|capabilities| {
                capabilities.set_can_generate_method_entry_events(true);
            })
            .and_then(|_| jvmti.set_default_agent_callbacks())
            .and_then(|_| {
                jvmti.enable_events_global(&[
                    jvmti::JVMTI_EVENT_METHOD_ENTRY,
                    jvmti::JVMTI_EVENT_VM_DEATH,
                ])
            })
            .is_err()
        {
            return jni::JNI_ERR;
        }

        ALLOCATIONS.store(0, Ordering::Relaxed);
        ALLOCATED_BYTES.store(0, Ordering::Relaxed);
        DEALLOCATIONS.store(0, Ordering::Relaxed);
        REALLOCATIONS.store(0, Ordering::Relaxed);
        TRACKING.store(true, Ordering::Release);
        jni::JNI_OK
    }

    fn method_entry(&self, _context: CallbackContext<'_>, _event: MethodEvent) {
        self.callbacks.fetch_add(1, Ordering::Relaxed);
    }

    fn vm_death(&self, _context: CallbackContext<'_>) {
        TRACKING.store(false, Ordering::Release);
        eprintln!(
            "callback_allocation_audit callbacks={} allocations={} allocated_bytes={} reallocations={} deallocations={}",
            self.callbacks.load(Ordering::Relaxed),
            ALLOCATIONS.load(Ordering::Relaxed),
            ALLOCATED_BYTES.load(Ordering::Relaxed),
            REALLOCATIONS.load(Ordering::Relaxed),
            DEALLOCATIONS.load(Ordering::Relaxed),
        );
    }
}

export_agent!(CallbackAllocationAudit);
