# Migrating From 2.x to 3.0

Version 3.0 is an intentional source-breaking release. It removes parallel
reduced and `*_with_jvmti` callback APIs, gives every event one canonical
callback-scoped representation, and closes raw-pointer ownership gaps that
could not be fixed compatibly in 2.x.

Version 2.3.x is the final compatibility line for the old callback trait. Stay
on 2.3.x while migrating if the application cannot absorb all changes in one
update; do not mix 2.3 callback implementations with 3.0 raw bindings.

## Toolchain And Dependency Changes

Version 3.0 requires Rust 1.85 or newer and uses Edition 2024. Upgrade the
consumer toolchain before changing the dependency version. The crate has zero
third-party crate dependencies across every optional feature and development
target; the `embed` feature now uses a small internal platform loader.

Existing `&str` JNI helpers remain source-compatible. Performance-sensitive
code can migrate fixed names and signatures to allocation-free `&CStr` variants:

```rust,ignore
let class = jni.find_class_cstr(c"java/lang/String")?;
let method = unsafe { jni.get_method_id_cstr(class, c"length", c"()I")? };
```

## Who Must Read Which Sections

| Consumer | Required sections |
|---|---|
| Implements `Agent` | Upgrade checklist, lifecycle entry points, event callbacks, callback table |
| Uses `JniEnv`, `Jvmti`, `LocalRef`, `GlobalRef`, or allocation helpers | Ownership changes, raw-handle operations, exhaustive unsafe-method inventory |
| Imports `sys::jni` or `sys::jvmti` | Raw binding migration |
| Supports several JDK releases | Versioned operations and compatibility evidence |

## Upgrade Checklist

1. Change the dependency to `jvmti-bindings = "3"` and compile before making
   manual changes. Keep the resulting errors as the migration worklist.
2. Replace lifecycle arguments with `AgentLoadContext` and
   `AgentUnloadContext`.
3. Replace every implemented event method with the canonical signature in the
   callback table below. Delete every `*_with_jvmti` implementation.
4. Read raw callback values from the typed event payload. Obtain JVM TI from
   `context.jvmti()` and use `context.jni()` only when it returns `Some`.
5. Move JVMTI allocations into `JvmtiAllocation` and raw JNI references into
   explicitly audited ownership paths.
6. Add narrowly scoped `unsafe` blocks around operations that consume
   caller-supplied JNI/JVMTI handles. Document the invariant at each call site;
   do not wrap an entire agent implementation in one `unsafe` block.
7. If the application imports `sys`, apply every raw binding migration below.
   The 3.0 crate deliberately provides no alias for a known-wrong ABI.
8. Run the normal test suite on every supported JDK and repeat any native
   sanitizer or callback-delivery tests used by the application.
9. Compile once with Rust 1.85 and once with current stable Rust; do not infer
   MSRV compatibility from a newer compiler.

The repository's compile-checked counterpart to this guide is
`tests/migration_3.rs`.

## Lifecycle Entry Points

### Load and attach

Before, in 2.x:

```rust,ignore
fn on_load(&self, vm: *mut jni::JavaVM, options: &str) -> jni::jint {
    let Ok(jvmti) = Jvmti::new(vm) else {
        return jni::JNI_ERR;
    };
    // Configure the agent with jvmti and options.
    jni::JNI_OK
}
```

After, in 3.0:

```rust,ignore
fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
    let Ok(jvmti) = context.vm().jvmti() else {
        return jni::JNI_ERR;
    };
    let exact_bytes = context.option_bytes();
    let strict_text = match context.options_str() {
        Ok(text) => text,
        Err(_) => return jni::JNI_ERR,
    };
    // Configure the agent with jvmti, exact_bytes, and strict_text.
    jni::JNI_OK
}
```

`AgentLoadContext` preserves distinctions that 2.x erased: null versus empty
options, invalid UTF-8, the exact `JavaVM*`, and the reserved pointer.
`on_attach` takes the same context type. `AgentUnloadContext` supplies the
`JavaVM*` to `on_unload`:

```rust,ignore
fn on_unload(&self, context: AgentUnloadContext<'_>) {
    let vm = context.vm().raw();
    // Flush state associated with vm without retaining the borrowed context.
}
```

`Jvmti::new` now accepts `&JavaVmRef<'_>`. Code outside a trusted lifecycle
callback that genuinely owns a live raw `JavaVM*` must use:

```rust,ignore
let jvmti = unsafe { Jvmti::from_java_vm_raw(vm_ptr) }?;
```

### Panic containment

Version 3.0 catches unwinding at lifecycle and event FFI boundaries. The new
defaulted `Agent::callback_panicked(&'static str)` hook reports which callback
panicked. It must not panic and should avoid blocking or allocation because
some callback phases severely restrict legal JVM operations.

## Event Callback Model

Every standard event now has exactly one unsuffixed `Agent` method:

```rust,ignore
fn method_entry(&self, context: CallbackContext<'_>, event: MethodEvent) {
    let jvmti = context.jvmti();
    let Some(jni) = context.jni() else {
        return;
    };
    let thread = event.thread();
    let method = event.method();
}
```

`CallbackContext::jvmti()` is always present. `CallbackContext::jni()` returns
`None` when the native JVMTI callback has no `JNIEnv*`; version 3.0 never
manufactures a thread-local JNI environment. Contexts and borrowed payload
views cannot escape the callback lifetime.

### Complete callback migration table

The 2.3 signatures below omit `&self` for readability. `JVMTI*` means
`*mut jvmtiEnv`, `JNI*` means `*mut JNIEnv`, and the other names correspond to
the public JNI/JVMTI aliases. A row containing `with:` had a parallel
`*_with_jvmti` method in 2.3; both old methods are replaced by the single 3.0
method shown.

| Event | 2.3 callback signature(s) | 3.0 canonical signature | JNI in context | Payload migration |
|---|---|---|---|---|
| VM init | `vm_init(JNI*, jthread)`; with: `(JVMTI*, JNI*, jthread)` | `vm_init(CallbackContext<'_>, ThreadEvent)` | `Some` | `event.thread()` |
| VM death | `vm_death(JNI*)`; with: `(JVMTI*, JNI*)` | `vm_death(CallbackContext<'_>)` | `Some` | JVM TI is now always in the context |
| VM start | `vm_start(JNI*)`; with: `(JVMTI*, JNI*)` | `vm_start(CallbackContext<'_>)` | `Some` | JVM TI is now always in the context |
| Thread start | `thread_start(JNI*, jthread)` | `thread_start(CallbackContext<'_>, ThreadEvent)` | `Some` | `event.thread()` |
| Thread end | `thread_end(JNI*, jthread)` | `thread_end(CallbackContext<'_>, ThreadEvent)` | `Some` | `event.thread()` |
| Virtual-thread start | `virtual_thread_start(JNI*, jthread)` | `virtual_thread_start(CallbackContext<'_>, ThreadEvent)` | `Some` | `event.thread()` |
| Virtual-thread end | `virtual_thread_end(JNI*, jthread)` | `virtual_thread_end(CallbackContext<'_>, ThreadEvent)` | `Some` | `event.thread()` |
| Class load | `class_load(JNI*, jthread, jclass)`; with: `(JVMTI*, JNI*, jthread, jclass)` | `class_load(CallbackContext<'_>, ClassEvent)` | `Some` | `event.thread()`, `event.class()` |
| Class prepare | `class_prepare(JNI*, jthread, jclass)`; with: `(JVMTI*, JNI*, jthread, jclass)` | `class_prepare(CallbackContext<'_>, ClassEvent)` | `Some` | `event.thread()`, `event.class()` |
| Class-file load hook | Raw JNI/class/loader/name/domain/byte pointers and output pointers; with: leading `JVMTI*` | `class_file_load_hook<'callback>(CallbackContext<'callback>, ClassFileLoadHookEvent<'callback>)` | `Some` | Borrow `name()` and `class_data()`; publish bytes with `set_transformed_class()` |
| Method entry | `method_entry(JNI*, jthread, jmethodID)`; with: `(JVMTI*, JNI*, jthread, jmethodID)` | `method_entry(CallbackContext<'_>, MethodEvent)` | `Some` | `event.thread()`, `event.method()` |
| Method exit | `method_exit(JNI*, jthread, jmethodID)`; with: `(JVMTI*, JNI*, jthread, jmethodID)` | `method_exit(CallbackContext<'_>, MethodExitEvent)` | `Some` | Adds `was_popped_by_exception()` and `return_value()`; 2.3 discarded both |
| Native-method bind | `native_method_bind(JNI*, jthread, jmethodID, void*, void**)` | `native_method_bind(CallbackContext<'_>, NativeMethodBindEvent)` | `Some` | Read `address()`; redirect with unsafe `set_new_address()` |
| Compiled-method load | Raw method/code/map/compile-info pointers; with: leading `JVMTI*` | `compiled_method_load<'callback>(CallbackContext<'callback>, CompiledMethodLoadEvent<'callback>)` | `None` | `map()` is a typed `jvmtiAddrLocationMap` slice; `compile_info()` remains opaque |
| Compiled-method unload | `compiled_method_unload(jmethodID, void*)`; with: leading `JVMTI*` | `compiled_method_unload(CallbackContext<'_>, CompiledMethodUnloadEvent)` | `None` | `event.method()`, `event.code_address()` |
| Dynamic-code generated | `dynamic_code_generated(c_char*, void*, jint)`; with: leading `JVMTI*` | `dynamic_code_generated<'callback>(CallbackContext<'callback>, DynamicCodeGeneratedEvent<'callback>)` | `None` | Borrow `name()`; use `address()` and `length()` |
| Data-dump request | `data_dump_request()` | `data_dump_request(CallbackContext<'_>)` | `None` | JVM TI is newly available through the context |
| Exception | `exception(JNI*, jthread, jmethodID, jlocation, jobject, jmethodID, jlocation)` | `exception(CallbackContext<'_>, ExceptionEvent)` | `Some` | All raw fields have same-named accessors |
| Exception catch | `exception_catch(JNI*, jthread, jmethodID, jlocation, jobject)` | `exception_catch(CallbackContext<'_>, ExceptionCatchEvent)` | `Some` | All raw fields have same-named accessors |
| Single step | `single_step(JNI*, jthread, jmethodID, jlocation)` | `single_step(CallbackContext<'_>, LocationEvent)` | `Some` | `thread()`, `method()`, `location()` |
| Breakpoint | `breakpoint(JNI*, jthread, jmethodID, jlocation)` | `breakpoint(CallbackContext<'_>, LocationEvent)` | `Some` | `thread()`, `method()`, `location()` |
| Frame pop | `frame_pop(JNI*, jthread, jmethodID, jboolean)` | `frame_pop(CallbackContext<'_>, FramePopEvent)` | `Some` | Use boolean `was_popped_by_exception()` |
| Monitor wait | `monitor_wait(JNI*, jthread, jobject, jlong)` | `monitor_wait(CallbackContext<'_>, MonitorWaitEvent)` | `Some` | `thread()`, `object()`, `timeout()` |
| Monitor waited | `monitor_waited(JNI*, jthread, jobject, jboolean)` | `monitor_waited(CallbackContext<'_>, MonitorWaitedEvent)` | `Some` | Use boolean `timed_out()` |
| Monitor contended enter | `monitor_contended_enter(JNI*, jthread, jobject)` | `monitor_contended_enter(CallbackContext<'_>, MonitorEvent)` | `Some` | `thread()`, `object()` |
| Monitor contended entered | `monitor_contended_entered(JNI*, jthread, jobject)` | `monitor_contended_entered(CallbackContext<'_>, MonitorEvent)` | `Some` | `thread()`, `object()` |
| Field access | `field_access(JNI*, jthread, jmethodID, jlocation, jclass, jobject, jfieldID)` | `field_access(CallbackContext<'_>, FieldAccessEvent)` | `Some` | Accessors preserve the complete payload |
| Field modification | Prior fields plus `c_char` signature and `jvalue` | `field_modification(CallbackContext<'_>, FieldModificationEvent)` | `Some` | Adds typed accessors including `signature_type()` and `new_value()` |
| GC start | `garbage_collection_start()` | `garbage_collection_start(CallbackContext<'_>)` | `None` | JVM TI is available, but only GC-safe operations are legal |
| GC finish | `garbage_collection_finish()` | `garbage_collection_finish(CallbackContext<'_>)` | `None` | JVM TI is available, but only GC-safe operations are legal |
| Resource exhausted | `resource_exhausted(JNI*, jint, c_char*)` | `resource_exhausted<'callback>(CallbackContext<'callback>, ResourceExhaustedEvent<'callback>)` | `Some` | Adds the previously discarded `reserved()` value and borrowed `description()` |
| Object free | `object_free(jlong)` | `object_free(CallbackContext<'_>, ObjectFreeEvent)` | `None` | `event.tag()`; only object-free-safe JVMTI operations are legal |
| VM object allocation | `vm_object_alloc(JNI*, jthread, jobject, jclass, jlong)` | `vm_object_alloc(CallbackContext<'_>, ObjectAllocationEvent)` | `Some` | Use `object_opt()` because JDK 28 value objects may report null identity |
| Sampled object allocation | `sampled_object_alloc(JNI*, jthread, jobject, jclass, jlong)` | `sampled_object_alloc(CallbackContext<'_>, ObjectAllocationEvent)` | `Some` | Use `object_opt()` because JDK 28 value objects may report null identity |

### Class transformation ownership

In 2.x callers manually allocated memory, copied bytes, and wrote signed lengths
and output pointers. In 3.0 the callback performs checked allocation and explicit
ownership transfer:

```rust,ignore
fn class_file_load_hook<'callback>(
    &self,
    context: CallbackContext<'callback>,
    mut event: ClassFileLoadHookEvent<'callback>,
) {
    let transformed = transform(event.class_data());
    if let Err(error) = event.set_transformed_class(&context, &transformed) {
        report_transformation_failure(error);
    }
}
```

### Mutable native binding

`NativeMethodBindEvent::set_new_address` is unsafe because the replacement must
have the exact native ABI required by the Java method and remain valid while the
JVM can call it:

```rust,ignore
unsafe { event.set_new_address(replacement.cast()) };
```

## JVM TI Allocation and Environment Ownership

`Jvmti::allocate(usize)` now returns `JvmtiAllocation`, which deallocates on
drop and borrows the environment. The environment therefore cannot be disposed
while the allocation is live.

```rust,ignore
let mut allocation = jvmti.allocate(4096)?;
allocation.as_mut_slice().fill(0);
```

Raw allocation and deallocation remain explicit escape hatches:

```rust,ignore
let allocation = jvmti.allocate(4096)?;
let ptr = unsafe { allocation.into_raw() };
// Transfer ownership to the JVM, or eventually release it on the same env:
unsafe { jvmti.deallocate_raw(ptr)? };
```

The exact source changes are:

| 2.x | 3.0 | Migration |
|---|---|---|
| `allocate(jlong) -> Result<*mut u8, _>` | `allocate(usize) -> Result<JvmtiAllocation<'_>, _>` | Keep the guard or call unsafe `into_raw()` for an intentional transfer |
| `deallocate(*mut u8)` | `unsafe deallocate_raw(*mut u8)` | Prefer automatic guard cleanup |
| `dispose_environment(&self)` | `dispose_environment(self)` | End all borrows, then consume the environment |
| `get_jni_function_table() -> *mut JNIEnv` | `get_jni_function_table() -> JniFunctionTable<'_>` | Keep the owning guard; use `as_ptr()` only for audited raw integration |
| `set_jni_function_table(*const JNIEnv)` | `unsafe set_jni_function_table(*const JNINativeInterface_)` | Pass the correctly indirect table and document process-wide validity |

`JniFunctionTable` is deliberately opaque. An older VM may allocate only its
shorter JNI table prefix, so it cannot be safely borrowed as a complete JDK 28
`JNINativeInterface_`. `known_byte_len()` reports the audited prefix when the
interface milestone is known.

## JNI Reference Ownership

Adopting an arbitrary local reference is no longer safe by declaration:

```rust,ignore
let raw = jni.find_class("java/lang/String").expect("String class");
let owned = unsafe { LocalRef::from_raw(jni, raw) };
```

This replaces `LocalRef::new`. The caller must prove that the reference belongs
to the current JNI thread and local frame and that no other owner will delete
it. `GlobalRef::new` also changed from safe to unsafe because it consumes a
caller-supplied local reference and relies on its VM, thread, and lifetime
invariants:

```rust,ignore
let global = unsafe { GlobalRef::new(jni, raw_local) };
```

## Raw-Handle Operations

Methods whose correctness depends on a caller-supplied JNI/JVMTI handle are now
`unsafe`. This does not mean the implementation became less safe. It moves the
existing JVM ownership, thread, frame, lifetime, nullability, and same-VM
requirements to the Rust call site where they can be audited.

Prefer a small proof boundary:

```rust,ignore
// SAFETY: class came from this JNIEnv in the current local frame.
let parent = unsafe { jni.get_superclass(class) };

// SAFETY: thread came from this callback and is live in the same VM.
let state = unsafe { jvmti.get_thread_state(thread) }?;
```

### JNI methods changed from safe to unsafe

The following 63 `JniEnv` methods changed from safe to unsafe:

- Class and module handles: `define_class`, `get_superclass`,
  `is_assignable_from`, `get_object_class`, `is_instance_of`,
  `class_loader_parent`, `module_name`, `module_packages`,
  `module_class_loader`, `module_can_read`, `module_is_exported_to`, and
  `module_is_open_to`.
- Exceptions, strings, methods, and fields: `throw`, `throw_new`, `get_string`,
  `get_string_utf`, `get_string_length`, `get_string_utf_length`,
  `get_method_id`, `get_static_method_id`, `get_field_id`, and
  `get_static_field_id`.
- Objects and references: `alloc_object`, `new_object`, `is_same_object`,
  `new_global_ref`, `delete_global_ref`, `new_local_ref`, `delete_local_ref`,
  `new_weak_global_ref`, `delete_weak_global_ref`, and `pop_local_frame`.
- Arrays: `get_array_length`, `new_object_array`, `get_object_array_element`,
  `set_object_array_element`, `get_byte_array_region`, `set_byte_array_region`,
  `get_int_array_region`, `set_int_array_region`, `get_long_array_region`, and
  `set_long_array_region`.
- Invocation: `call_void_method`, `call_int_method`, `call_long_method`,
  `call_boolean_method`, `call_object_method`, `call_static_void_method`,
  `call_static_int_method`, and `call_static_object_method`.
- Field access: `get_object_field`, `get_int_field`, `get_long_field`,
  `set_object_field`, `set_int_field`, `set_long_field`,
  `get_static_object_field`, `get_static_int_field`, and
  `set_static_object_field`.
- Monitors and native registration: `monitor_enter`, `monitor_exit`,
  `register_natives`, and `unregister_natives`.

### JVM TI methods changed from safe to unsafe

The following 111 `Jvmti` method names changed from safe to unsafe:

```text
add_module_exports                 add_module_opens
add_module_provides                add_module_reads
add_module_uses                    clear_all_frame_pops
clear_breakpoint                   clear_field_access_watch
clear_field_modification_watch     destroy_raw_monitor
disable_event                      enable_event
follow_references                  force_early_return_double
force_early_return_float           force_early_return_int
force_early_return_long            force_early_return_object
force_early_return_void            get_arguments_size
get_bytecodes                      get_class_fields
get_class_loader                   get_class_methods
get_class_modifiers                get_class_signature
get_class_status                   get_class_version_numbers
get_classloader_classes            get_constant_pool
get_current_contended_monitor      get_field_declaring_class
get_field_modifiers                get_field_name
get_frame_count                    get_frame_location
get_implemented_interfaces        get_line_number_table
get_local_double                   get_local_float
get_local_instance                 get_local_int
get_local_long                     get_local_object
get_local_variable_table           get_max_locals
get_method_declaring_class         get_method_location
get_method_modifiers               get_method_name
get_named_module                   get_object_hash_code
get_object_monitor_usage           get_object_size
get_owned_monitor_info             get_owned_monitor_stack_depth_info
get_source_debug_extension         get_source_file_name
get_stack_trace                    get_tag
get_thread_cpu_time                get_thread_group_children
get_thread_group_info              get_thread_info
get_thread_list_stack_traces       get_thread_local_storage
get_thread_state                   interrupt_thread
is_array_class                     is_field_synthetic
is_interface                       is_method_native
is_method_obsolete                 is_method_synthetic
is_modifiable_class                is_modifiable_module
iterate_over_heap                  iterate_over_instances_of_class
iterate_over_objects_reachable_from_object
iterate_over_reachable_objects     iterate_through_heap
notify_frame_pop                   pop_frame
raw_monitor_enter                  raw_monitor_exit
raw_monitor_notify                 raw_monitor_notify_all
raw_monitor_wait                   redefine_classes
resume_all_virtual_threads         resume_thread
resume_thread_list                 retransform_classes
run_agent_thread                   set_breakpoint
set_environment_local_storage      set_event_notification_mode
set_field_access_watch             set_field_modification_watch
set_jni_function_table             set_local_double
set_local_float                    set_local_int
set_local_long                     set_local_object
set_tag                            set_thread_local_storage
stop_thread                        suspend_all_virtual_threads
suspend_thread                     suspend_thread_list
```

The unsafe change is the only signature change for most methods. Additional
signature changes that require more than an `unsafe` block are:

- `suspend_all_virtual_threads` and `resume_all_virtual_threads` now take an
  exception-thread slice, matching JDK 21+: pass `&[]` for no exceptions.
- `run_agent_thread` now uses the corrected `jvmtiStartFunction`, whose callback
  receives `(jvmtiEnv*, JNIEnv*, void*)` rather than omitting `JNIEnv*`.
- Legacy heap iteration methods use corrected legacy callback types, while
  `follow_references` and `iterate_through_heap` use the complete modern
  `jvmtiHeapCallbacks` table.
- `set_jni_function_table` takes `*const JNINativeInterface_`, not
  `*const JNIEnv`.

## Other Public API Changes

- `set_global_agent` now returns `Result<(), GlobalAgentAlreadySet>` instead of
  `Result<(), ()>`. Match or propagate the typed error.
- Safe-wrapper `ExtensionFunctionInfo::func` now contains
  `Option<jvmtiExtensionFunction>` instead of `*mut c_void`; callers must
  acknowledge the vendor-defined variadic ABI before invoking it.
- `JniEnv::from_raw` and `Jvmti::from_raw` remain unsafe. The new
  `Jvmti::from_java_vm_raw` is the unsafe replacement for passing an arbitrary
  raw `JavaVM*` to `Jvmti::new`.
- New wrapper and event types are `#[non_exhaustive]` where future JDK additions
  may extend them. Construct them through crate APIs and match public enums with
  a fallback arm.

## Raw `sys` Binding Migration

`sys` is public and intentionally mirrors upstream headers. Version 3.0 does
not retain aliases for declarations whose 2.3 ABI was wrong. Raw users must
apply the following changes.

| 2.3 declaration or assumption | 3.0 declaration | Required migration |
|---|---|---|
| Closed `jvmtiError` enum | `#[repr(transparent)] jvmtiError(jint)` | Keep known associated constants, add an unknown fallback, and use `raw()` / `from_raw()` for storage or extensions |
| `jvmtiExtensionParamInfo` | `jvmtiParamInfo` | Rename imports and pointer fields |
| Four-field `jvmtiHeapCallbacks` | Exact 16-slot modern callback table | Initialize with `jvmtiHeapCallbacks::default()` and set the required modern callbacks |
| `jvmtiObjectReferenceInfo*` used as modern heap metadata | `jvmtiHeapReferenceInfo` and its field/array/constant-pool/stack/JNI/reserved members | Port modern heap callbacks; do not reinterpret the old union |
| `jvmtiObjectCallback` | `jvmtiHeapObjectCallback` | Update deprecated legacy heap callback declarations |
| Incomplete heap callback signatures | Exact modern and legacy callback aliases | Recompile every callback; parameter order and arity are ABI-significant |
| Four-field `jvmtiTimerInfo` | Six-field structure including `reserved1` and `reserved2` | Prefer `Default`; update struct literals |
| Misordered `jvmtiStackInfo` | Header-order `thread`, `state`, `frame_buffer`, `frame_count` | Rebuild raw layout assumptions; deallocate only the returned top-level allocation |
| Two-argument `jvmtiStartFunction` | Three-argument callback including `JNIEnv*` | Update agent-thread functions |
| Zero-argument `jvmtiExtensionEventCallback` | `Option<unsafe extern "C" fn(jvmtiEnv*, ...)>` | Use the exact vendor signature and treat registration/invocation as raw FFI |
| `jvmtiExtensionFunctionInfo.func: *mut c_void` | `Option<jvmtiExtensionFunction>` | Handle the opaque variadic callable type explicitly |
| Fixed `JvmtiSetEventNotificationModeFn` | Exact variadic `extern "C"` function type | Normal wrapper calls remain fixed-argument; raw extension calls must obey the vendor ABI |
| `JvmtiSuspendAllVirtualThreadsFn` and `JvmtiResumeAllVirtualThreadsFn` with only `jvmtiEnv*` | Adds `except_count` and `except_list` | Pass a valid list or `(0, null)` |
| JNI-table APIs expressed through `JNIEnv` | APIs expressed through `JNINativeInterface_` | Remove the extra pointer indirection |
| `JVMTI_HEAP_OBJECT_EITHER == 0` | Correct header value `3` | Remove any workaround for the old value |
| JDK 27-sized `JNINativeInterface_` struct literals | JDK 28 tail including `HasIdentity` | Prefer `Default`/provided tables or initialize the new field explicitly |

### Open error values

Code that exhaustively matched the 2.3 enum must preserve unknown values:

```rust,ignore
match error {
    value if value == jvmti::jvmtiError::NONE => handle_success(),
    value if value == jvmti::jvmtiError::WRONG_PHASE => retry_later(),
    unknown => report_unknown(unknown.raw()),
}
```

This is a source break by design: a JVM or extension can return values unknown
when the crate was compiled, and representing those values as an invalid Rust
enum discriminant was unsound.

## Versioned Operations and JDK 28 Status

Use `JniEnv::supports_feature(JniFeature::...)` and
`Jvmti::supports_feature(JvmtiFeature::...)` when optional behavior is useful.
Safe wrappers also enforce gates before reading an appended table tail,
reclaimed slot, or newly consumed capability bit.

The 3.0 ABI is verified against pinned OpenJDK source/header snapshots for every
feature release from JDK 8 through the current JDK 28 main-line snapshot. Real
callback delivery has been exercised on installed JVMs through JDK 27. JDK 28
value-object behavior remains preview work and requires the specialized live
preview-runtime test before 3.0 publication; it is not described as final Java
SE 28 behavior.

One Rust layout is used across supported JVMs. Calls read only the function-table
prefix verified for the negotiated runtime. `release_profile(jdk)` exposes the
audited profile and `release_delta(jdk)` exposes structural, semantic, source,
and policy changes from the preceding release. A JVM TI interface milestone is
not always the exact Java release: JDK 10 reports JVM TI 9 and JDK 12 reports
JVM TI 11.

JDK 29 is not claimed until an official identifiable JDK 29 source line passes
the same ABI and runtime matrix.

## Verification After Migration

At minimum:

```bash
cargo test --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
scripts/check-jdk-abi.sh --all-releases
scripts/prove-event-callback-matrix.sh
```

The repository test `tests/migration_3.rs` compiles all 34 canonical callback
signatures and representative ownership, open-error, and unsafe-handle
migrations. It also checks that this guide retains the complete callback and
safe-to-unsafe method inventories.

The separate sanitizer-backed stack/timer/ownership checks and the live JDK 28
value-object preview check remain publication gates as described in
`ROADMAP_3_0_0_ABI_AND_CALLBACK_FIDELITY.md`.
