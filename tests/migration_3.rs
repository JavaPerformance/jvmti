//! Compile-checked examples and documentation coverage for the 2.x to 3.0 migration.

use jvmti_bindings::prelude::*;

struct MigratedAgent;

impl Agent for MigratedAgent {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let _exact_options = context.option_bytes();
        match context.options_str() {
            Ok(_) => jni::JNI_OK,
            Err(_) => jni::JNI_ERR,
        }
    }

    fn on_attach(&self, context: AgentLoadContext<'_>) -> jni::jint {
        self.on_load(context)
    }

    fn on_unload(&self, context: AgentUnloadContext<'_>) {
        let _vm = context.vm().raw();
    }

    fn callback_panicked(&self, _event: &'static str) {}

    fn vm_init(&self, context: CallbackContext<'_>, event: ThreadEvent) {
        let _ = (context.jvmti(), context.jni(), event.thread());
    }
    fn vm_death(&self, context: CallbackContext<'_>) {
        let _ = (context.jvmti(), context.jni());
    }
    fn vm_start(&self, context: CallbackContext<'_>) {
        let _ = (context.jvmti(), context.jni());
    }
    fn thread_start(&self, _context: CallbackContext<'_>, event: ThreadEvent) {
        let _ = event.thread();
    }
    fn thread_end(&self, _context: CallbackContext<'_>, event: ThreadEvent) {
        let _ = event.thread();
    }
    fn virtual_thread_start(&self, _context: CallbackContext<'_>, event: ThreadEvent) {
        let _ = event.thread();
    }
    fn virtual_thread_end(&self, _context: CallbackContext<'_>, event: ThreadEvent) {
        let _ = event.thread();
    }
    fn class_load(&self, _context: CallbackContext<'_>, event: ClassEvent) {
        let _ = (event.thread(), event.class());
    }
    fn class_prepare(&self, _context: CallbackContext<'_>, event: ClassEvent) {
        let _ = (event.thread(), event.class());
    }

    fn class_file_load_hook<'callback>(
        &self,
        context: CallbackContext<'callback>,
        mut event: ClassFileLoadHookEvent<'callback>,
    ) {
        let _ = (
            event.class_being_redefined(),
            event.loader(),
            event.name(),
            event.protection_domain(),
        );
        let transformed = event.class_data().to_vec();
        let _ = event.set_transformed_class(&context, &transformed);
    }

    fn method_entry(&self, _context: CallbackContext<'_>, event: MethodEvent) {
        let _ = (event.thread(), event.method());
    }
    fn method_exit(&self, _context: CallbackContext<'_>, event: MethodExitEvent) {
        let _ = (
            event.thread(),
            event.method(),
            event.was_popped_by_exception(),
            event.return_value(),
        );
    }
    fn native_method_bind(&self, _context: CallbackContext<'_>, mut event: NativeMethodBindEvent) {
        let _ = (event.thread(), event.method());
        // SAFETY: retaining the JVM-selected address preserves its ABI and lifetime.
        unsafe { event.set_new_address(event.address()) };
    }

    fn compiled_method_load<'callback>(
        &self,
        _context: CallbackContext<'callback>,
        event: CompiledMethodLoadEvent<'callback>,
    ) {
        let _ = (
            event.method(),
            event.code_size(),
            event.code_address(),
            event.map(),
            event.compile_info(),
        );
    }

    fn compiled_method_unload(
        &self,
        _context: CallbackContext<'_>,
        event: CompiledMethodUnloadEvent,
    ) {
        let _ = (event.method(), event.code_address());
    }

    fn dynamic_code_generated<'callback>(
        &self,
        _context: CallbackContext<'callback>,
        event: DynamicCodeGeneratedEvent<'callback>,
    ) {
        let _ = (event.name(), event.address(), event.length());
    }

    fn data_dump_request(&self, _context: CallbackContext<'_>) {}
    fn exception(&self, _context: CallbackContext<'_>, event: ExceptionEvent) {
        let _ = (
            event.thread(),
            event.method(),
            event.location(),
            event.exception(),
            event.catch_method(),
            event.catch_location(),
        );
    }
    fn exception_catch(&self, _context: CallbackContext<'_>, event: ExceptionCatchEvent) {
        let _ = (
            event.thread(),
            event.method(),
            event.location(),
            event.exception(),
        );
    }

    fn single_step(&self, _context: CallbackContext<'_>, event: LocationEvent) {
        let _ = (event.thread(), event.method(), event.location());
    }
    fn breakpoint(&self, _context: CallbackContext<'_>, event: LocationEvent) {
        let _ = (event.thread(), event.method(), event.location());
    }
    fn frame_pop(&self, _context: CallbackContext<'_>, event: FramePopEvent) {
        let _ = (
            event.thread(),
            event.method(),
            event.was_popped_by_exception(),
        );
    }
    fn monitor_wait(&self, _context: CallbackContext<'_>, event: MonitorWaitEvent) {
        let _ = (event.thread(), event.object(), event.timeout());
    }
    fn monitor_waited(&self, _context: CallbackContext<'_>, event: MonitorWaitedEvent) {
        let _ = (event.thread(), event.object(), event.timed_out());
    }

    fn monitor_contended_enter(&self, _context: CallbackContext<'_>, event: MonitorEvent) {
        let _ = (event.thread(), event.object());
    }

    fn monitor_contended_entered(&self, _context: CallbackContext<'_>, event: MonitorEvent) {
        let _ = (event.thread(), event.object());
    }

    fn field_access(&self, _context: CallbackContext<'_>, event: FieldAccessEvent) {
        let _ = (
            event.thread(),
            event.method(),
            event.location(),
            event.field_class(),
            event.object(),
            event.field(),
        );
    }
    fn field_modification(&self, _context: CallbackContext<'_>, event: FieldModificationEvent) {
        let _ = (
            event.thread(),
            event.method(),
            event.location(),
            event.field_class(),
            event.object(),
            event.field(),
            event.signature_type(),
            event.new_value(),
        );
    }

    fn garbage_collection_start(&self, _context: CallbackContext<'_>) {}
    fn garbage_collection_finish(&self, _context: CallbackContext<'_>) {}

    fn resource_exhausted<'callback>(
        &self,
        _context: CallbackContext<'callback>,
        event: ResourceExhaustedEvent<'callback>,
    ) {
        let _ = (event.flags(), event.reserved(), event.description());
    }

    fn object_free(&self, _context: CallbackContext<'_>, event: ObjectFreeEvent) {
        let _ = event.tag();
    }
    fn vm_object_alloc(&self, _context: CallbackContext<'_>, event: ObjectAllocationEvent) {
        let _ = (
            event.thread(),
            event.object_opt(),
            event.class(),
            event.size(),
        );
    }

    fn sampled_object_alloc(&self, _context: CallbackContext<'_>, event: ObjectAllocationEvent) {
        let _ = (
            event.thread(),
            event.object_opt(),
            event.class(),
            event.size(),
        );
    }
}

#[allow(dead_code)]
fn allocation_and_reference_migration<'a>(
    jvmti: &'a Jvmti,
    jni_env: &'a JniEnv,
    raw_local: jni::jobject,
) -> Result<JvmtiAllocation<'a>, jvmti::jvmtiError> {
    let mut allocation = jvmti.allocate(16)?;
    allocation.as_mut_slice().fill(0);

    // SAFETY: callers of this example must provide a local reference owned by
    // the current JNI thread and frame, with no competing deletion owner.
    let local = unsafe { LocalRef::from_raw(jni_env, raw_local) };

    // SAFETY: the same caller guarantee makes the local reference valid for
    // promotion to a global reference on this JNI environment.
    let global = unsafe { GlobalRef::new(jni_env, local.get()) };
    drop(global);
    drop(local);
    Ok(allocation)
}

#[allow(dead_code)]
unsafe fn raw_handle_migration(
    jvmti: &Jvmti,
    jni_env: &JniEnv,
    class: jni::jclass,
    thread: jni::jthread,
) -> Result<(), jvmti::jvmtiError> {
    // SAFETY: the caller guarantees both handles belong to the live VM and are
    // valid for these operations on the current thread.
    let _superclass = unsafe { jni_env.get_superclass(class) };
    let _state = unsafe { jvmti.get_thread_state(thread) }?;
    Ok(())
}

#[allow(dead_code)]
fn classify_open_error(error: jvmti::jvmtiError) -> &'static str {
    match error {
        value if value == jvmti::jvmtiError::NONE => "success",
        value if value == jvmti::jvmtiError::WRONG_PHASE => "wrong phase",
        _ => "unknown or unhandled",
    }
}

#[test]
fn canonical_agent_surface_compiles() {
    fn assert_agent<T: Agent>() {}
    assert_agent::<MigratedAgent>();
}

#[test]
fn migration_guide_covers_the_complete_break_inventory() {
    const GUIDE: &str = include_str!("../docs/MIGRATING_2_TO_3.md");
    const CALLBACKS: &[&str] = &[
        "vm_init",
        "vm_death",
        "vm_start",
        "thread_start",
        "thread_end",
        "virtual_thread_start",
        "virtual_thread_end",
        "class_load",
        "class_prepare",
        "class_file_load_hook",
        "method_entry",
        "method_exit",
        "native_method_bind",
        "compiled_method_load",
        "compiled_method_unload",
        "dynamic_code_generated",
        "data_dump_request",
        "exception",
        "exception_catch",
        "single_step",
        "breakpoint",
        "frame_pop",
        "monitor_wait",
        "monitor_waited",
        "monitor_contended_enter",
        "monitor_contended_entered",
        "field_access",
        "field_modification",
        "garbage_collection_start",
        "garbage_collection_finish",
        "resource_exhausted",
        "object_free",
        "vm_object_alloc",
        "sampled_object_alloc",
    ];
    const JNI_SAFE_TO_UNSAFE: &[&str] = &[
        "alloc_object",
        "call_boolean_method",
        "call_int_method",
        "call_long_method",
        "call_object_method",
        "call_static_int_method",
        "call_static_object_method",
        "call_static_void_method",
        "call_void_method",
        "class_loader_parent",
        "define_class",
        "delete_global_ref",
        "delete_local_ref",
        "delete_weak_global_ref",
        "get_array_length",
        "get_byte_array_region",
        "get_field_id",
        "get_int_array_region",
        "get_int_field",
        "get_long_array_region",
        "get_long_field",
        "get_method_id",
        "get_object_array_element",
        "get_object_class",
        "get_object_field",
        "get_static_field_id",
        "get_static_int_field",
        "get_static_method_id",
        "get_static_object_field",
        "get_string",
        "get_string_length",
        "get_string_utf",
        "get_string_utf_length",
        "get_superclass",
        "is_assignable_from",
        "is_instance_of",
        "is_same_object",
        "module_can_read",
        "module_class_loader",
        "module_is_exported_to",
        "module_is_open_to",
        "module_name",
        "module_packages",
        "monitor_enter",
        "monitor_exit",
        "new_global_ref",
        "new_local_ref",
        "new_object",
        "new_object_array",
        "new_weak_global_ref",
        "pop_local_frame",
        "register_natives",
        "set_byte_array_region",
        "set_int_array_region",
        "set_int_field",
        "set_long_array_region",
        "set_long_field",
        "set_object_array_element",
        "set_object_field",
        "set_static_object_field",
        "throw",
        "throw_new",
        "unregister_natives",
    ];
    const JVMTI_SAFE_TO_UNSAFE: &[&str] = &[
        "add_module_exports",
        "add_module_opens",
        "add_module_provides",
        "add_module_reads",
        "add_module_uses",
        "clear_all_frame_pops",
        "clear_breakpoint",
        "clear_field_access_watch",
        "clear_field_modification_watch",
        "destroy_raw_monitor",
        "disable_event",
        "enable_event",
        "follow_references",
        "force_early_return_double",
        "force_early_return_float",
        "force_early_return_int",
        "force_early_return_long",
        "force_early_return_object",
        "force_early_return_void",
        "get_arguments_size",
        "get_bytecodes",
        "get_class_fields",
        "get_class_loader",
        "get_class_methods",
        "get_class_modifiers",
        "get_class_signature",
        "get_class_status",
        "get_class_version_numbers",
        "get_classloader_classes",
        "get_constant_pool",
        "get_current_contended_monitor",
        "get_field_declaring_class",
        "get_field_modifiers",
        "get_field_name",
        "get_frame_count",
        "get_frame_location",
        "get_implemented_interfaces",
        "get_line_number_table",
        "get_local_double",
        "get_local_float",
        "get_local_instance",
        "get_local_int",
        "get_local_long",
        "get_local_object",
        "get_local_variable_table",
        "get_max_locals",
        "get_method_declaring_class",
        "get_method_location",
        "get_method_modifiers",
        "get_method_name",
        "get_named_module",
        "get_object_hash_code",
        "get_object_monitor_usage",
        "get_object_size",
        "get_owned_monitor_info",
        "get_owned_monitor_stack_depth_info",
        "get_source_debug_extension",
        "get_source_file_name",
        "get_stack_trace",
        "get_tag",
        "get_thread_cpu_time",
        "get_thread_group_children",
        "get_thread_group_info",
        "get_thread_info",
        "get_thread_list_stack_traces",
        "get_thread_local_storage",
        "get_thread_state",
        "interrupt_thread",
        "is_array_class",
        "is_field_synthetic",
        "is_interface",
        "is_method_native",
        "is_method_obsolete",
        "is_method_synthetic",
        "is_modifiable_class",
        "is_modifiable_module",
        "iterate_over_heap",
        "iterate_over_instances_of_class",
        "iterate_over_objects_reachable_from_object",
        "iterate_over_reachable_objects",
        "iterate_through_heap",
        "notify_frame_pop",
        "pop_frame",
        "raw_monitor_enter",
        "raw_monitor_exit",
        "raw_monitor_notify",
        "raw_monitor_notify_all",
        "raw_monitor_wait",
        "redefine_classes",
        "resume_all_virtual_threads",
        "resume_thread",
        "resume_thread_list",
        "retransform_classes",
        "run_agent_thread",
        "set_breakpoint",
        "set_environment_local_storage",
        "set_event_notification_mode",
        "set_field_access_watch",
        "set_field_modification_watch",
        "set_jni_function_table",
        "set_local_double",
        "set_local_float",
        "set_local_int",
        "set_local_long",
        "set_local_object",
        "set_tag",
        "set_thread_local_storage",
        "stop_thread",
        "suspend_all_virtual_threads",
        "suspend_thread",
        "suspend_thread_list",
    ];
    const RAW_BREAKS: &[&str] = &[
        "jvmtiError",
        "jvmtiParamInfo",
        "jvmtiHeapCallbacks",
        "jvmtiHeapReferenceInfo",
        "jvmtiHeapObjectCallback",
        "jvmtiTimerInfo",
        "jvmtiStackInfo",
        "jvmtiStartFunction",
        "jvmtiExtensionFunction",
        "jvmtiExtensionEventCallback",
        "ExtensionFunctionInfo",
        "JvmtiSetEventNotificationModeFn",
        "JvmtiSuspendAllVirtualThreadsFn",
        "JvmtiResumeAllVirtualThreadsFn",
        "JNINativeInterface_",
        "JVMTI_HEAP_OBJECT_EITHER",
    ];

    assert_eq!(CALLBACKS.len(), 34);
    assert_eq!(JNI_SAFE_TO_UNSAFE.len(), 63);
    assert_eq!(JVMTI_SAFE_TO_UNSAFE.len(), 111);
    assert_unique(CALLBACKS);
    assert_unique(JNI_SAFE_TO_UNSAFE);
    assert_unique(JVMTI_SAFE_TO_UNSAFE);
    assert_unique(RAW_BREAKS);
    assert!(GUIDE.contains("`GlobalRef::new` also changed from safe to unsafe"));

    for symbol in CALLBACKS
        .iter()
        .chain(JNI_SAFE_TO_UNSAFE)
        .chain(JVMTI_SAFE_TO_UNSAFE)
        .chain(RAW_BREAKS)
    {
        assert!(
            contains_identifier(GUIDE, symbol),
            "migration guide omits {symbol}"
        );
    }
}

fn assert_unique(symbols: &[&str]) {
    let unique: std::collections::BTreeSet<_> = symbols.iter().copied().collect();
    assert_eq!(
        unique.len(),
        symbols.len(),
        "migration inventory has duplicates"
    );
}

fn contains_identifier(document: &str, expected: &str) -> bool {
    document
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .any(|token| token == expected)
}
