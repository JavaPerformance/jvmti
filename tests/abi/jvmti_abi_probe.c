#include <stddef.h>
#include <stdio.h>
#include <string.h>

#include "jni.h"
#include "jvmti.h"

#ifndef PROBE_JDK_FEATURE
#error "PROBE_JDK_FEATURE must identify the header's JDK feature release"
#endif

#define PRINT_SIZE(type) printf("size." #type "=%zu\n", sizeof(type))
#define PRINT_ALIGN(type) printf("align." #type "=%zu\n", _Alignof(type))
#define PRINT_OFFSET(type, field) \
    printf("offset." #type "." #field "=%zu\n", offsetof(type, field))

static void print_capability(const char *name, const jvmtiCapabilities *caps) {
    const unsigned char *bytes = (const unsigned char *)caps;
    printf("capability.%s=", name);
    for (size_t i = 0; i < sizeof(*caps); i++) {
        printf("%02x", bytes[i]);
    }
    putchar('\n');
}

#define PRINT_CAPABILITY(field) do { \
    jvmtiCapabilities caps; \
    memset(&caps, 0, sizeof(caps)); \
    caps.field = 1; \
    print_capability(#field, &caps); \
} while (0)

int main(void) {
    printf("version.JVMTI=%d\n", JVMTI_VERSION);
#if PROBE_JDK_FEATURE >= 28
    printf("version.JNI=%d\n", JNI_VERSION_28);
#elif PROBE_JDK_FEATURE >= 24
    printf("version.JNI=%d\n", JNI_VERSION_24);
#elif PROBE_JDK_FEATURE >= 21
    printf("version.JNI=%d\n", JNI_VERSION_21);
#elif PROBE_JDK_FEATURE >= 20
    printf("version.JNI=%d\n", JNI_VERSION_20);
#elif PROBE_JDK_FEATURE >= 19
    printf("version.JNI=%d\n", JNI_VERSION_19);
#elif PROBE_JDK_FEATURE >= 10
    printf("version.JNI=%d\n", JNI_VERSION_10);
#elif PROBE_JDK_FEATURE >= 9
    printf("version.JNI=%d\n", JNI_VERSION_9);
#else
    printf("version.JNI=%d\n", JNI_VERSION_1_8);
#endif

    PRINT_SIZE(jvmtiError);
    PRINT_ALIGN(jvmtiError);
    PRINT_SIZE(jobjectRefType);
    PRINT_ALIGN(jobjectRefType);
    PRINT_SIZE(jvmtiTimerInfo);
    PRINT_ALIGN(jvmtiTimerInfo);
    PRINT_OFFSET(jvmtiTimerInfo, kind);
    PRINT_OFFSET(jvmtiTimerInfo, reserved1);
    PRINT_OFFSET(jvmtiTimerInfo, reserved2);

    PRINT_SIZE(jvmtiStackInfo);
    PRINT_ALIGN(jvmtiStackInfo);
    PRINT_OFFSET(jvmtiStackInfo, thread);
    PRINT_OFFSET(jvmtiStackInfo, state);
    PRINT_OFFSET(jvmtiStackInfo, frame_buffer);
    PRINT_OFFSET(jvmtiStackInfo, frame_count);

    PRINT_SIZE(jvmtiHeapReferenceInfoField);
    PRINT_SIZE(jvmtiHeapReferenceInfoArray);
    PRINT_SIZE(jvmtiHeapReferenceInfoConstantPool);
    PRINT_SIZE(jvmtiHeapReferenceInfoStackLocal);
    PRINT_SIZE(jvmtiHeapReferenceInfoJniLocal);
    PRINT_SIZE(jvmtiHeapReferenceInfoReserved);
    PRINT_SIZE(jvmtiHeapReferenceInfo);
    PRINT_ALIGN(jvmtiHeapReferenceInfo);

    PRINT_SIZE(jvmtiHeapCallbacks);
    PRINT_ALIGN(jvmtiHeapCallbacks);
    PRINT_OFFSET(jvmtiHeapCallbacks, heap_iteration_callback);
    PRINT_OFFSET(jvmtiHeapCallbacks, heap_reference_callback);
    PRINT_OFFSET(jvmtiHeapCallbacks, primitive_field_callback);
    PRINT_OFFSET(jvmtiHeapCallbacks, array_primitive_value_callback);
    PRINT_OFFSET(jvmtiHeapCallbacks, string_primitive_value_callback);
    PRINT_OFFSET(jvmtiHeapCallbacks, reserved15);

    PRINT_SIZE(jvmtiParamInfo);
    PRINT_ALIGN(jvmtiParamInfo);
    PRINT_SIZE(jvmtiExtensionFunctionInfo);
    PRINT_ALIGN(jvmtiExtensionFunctionInfo);
    PRINT_OFFSET(jvmtiExtensionFunctionInfo, func);
    PRINT_OFFSET(jvmtiExtensionFunctionInfo, id);
    PRINT_OFFSET(jvmtiExtensionFunctionInfo, params);
    PRINT_SIZE(jvmtiExtensionEventInfo);
    PRINT_ALIGN(jvmtiExtensionEventInfo);
    PRINT_OFFSET(jvmtiExtensionEventInfo, extension_event_index);
    PRINT_OFFSET(jvmtiExtensionEventInfo, id);

    PRINT_SIZE(jvmtiCapabilities);
    PRINT_ALIGN(jvmtiCapabilities);
    PRINT_CAPABILITY(can_tag_objects);
    PRINT_CAPABILITY(can_generate_field_modification_events);
    PRINT_CAPABILITY(can_generate_field_access_events);
    PRINT_CAPABILITY(can_get_bytecodes);
    PRINT_CAPABILITY(can_get_synthetic_attribute);
    PRINT_CAPABILITY(can_get_owned_monitor_info);
    PRINT_CAPABILITY(can_get_current_contended_monitor);
    PRINT_CAPABILITY(can_get_monitor_info);
    PRINT_CAPABILITY(can_pop_frame);
    PRINT_CAPABILITY(can_redefine_classes);
    PRINT_CAPABILITY(can_signal_thread);
    PRINT_CAPABILITY(can_get_source_file_name);
    PRINT_CAPABILITY(can_get_line_numbers);
    PRINT_CAPABILITY(can_get_source_debug_extension);
    PRINT_CAPABILITY(can_access_local_variables);
    PRINT_CAPABILITY(can_maintain_original_method_order);
    PRINT_CAPABILITY(can_generate_single_step_events);
    PRINT_CAPABILITY(can_generate_exception_events);
    PRINT_CAPABILITY(can_generate_frame_pop_events);
    PRINT_CAPABILITY(can_generate_breakpoint_events);
    PRINT_CAPABILITY(can_suspend);
    PRINT_CAPABILITY(can_redefine_any_class);
    PRINT_CAPABILITY(can_get_current_thread_cpu_time);
    PRINT_CAPABILITY(can_get_thread_cpu_time);
    PRINT_CAPABILITY(can_generate_method_entry_events);
    PRINT_CAPABILITY(can_generate_method_exit_events);
    PRINT_CAPABILITY(can_generate_all_class_hook_events);
    PRINT_CAPABILITY(can_generate_compiled_method_load_events);
    PRINT_CAPABILITY(can_generate_monitor_events);
    PRINT_CAPABILITY(can_generate_vm_object_alloc_events);
    PRINT_CAPABILITY(can_generate_native_method_bind_events);
    PRINT_CAPABILITY(can_generate_garbage_collection_events);
    PRINT_CAPABILITY(can_generate_object_free_events);
    PRINT_CAPABILITY(can_force_early_return);
    PRINT_CAPABILITY(can_get_owned_monitor_stack_depth_info);
    PRINT_CAPABILITY(can_get_constant_pool);
    PRINT_CAPABILITY(can_set_native_method_prefix);
    PRINT_CAPABILITY(can_retransform_classes);
    PRINT_CAPABILITY(can_retransform_any_class);
    PRINT_CAPABILITY(can_generate_resource_exhaustion_heap_events);
    PRINT_CAPABILITY(can_generate_resource_exhaustion_threads_events);
#if PROBE_JDK_FEATURE >= 9
    PRINT_CAPABILITY(can_generate_early_vmstart);
    PRINT_CAPABILITY(can_generate_early_class_hook_events);
#endif
#if PROBE_JDK_FEATURE >= 11
    PRINT_CAPABILITY(can_generate_sampled_object_alloc_events);
#endif
#if PROBE_JDK_FEATURE >= 19
    PRINT_CAPABILITY(can_support_virtual_threads);
#endif
#if PROBE_JDK_FEATURE >= 28
    PRINT_CAPABILITY(can_support_value_objects);
#endif

    PRINT_SIZE(jvmtiEventCallbacks);
    PRINT_ALIGN(jvmtiEventCallbacks);
    PRINT_OFFSET(jvmtiEventCallbacks, VMInit);
    PRINT_OFFSET(jvmtiEventCallbacks, MethodEntry);
    PRINT_OFFSET(jvmtiEventCallbacks, VMObjectAlloc);
#if PROBE_JDK_FEATURE >= 11
    PRINT_OFFSET(jvmtiEventCallbacks, SampledObjectAlloc);
#endif
#if PROBE_JDK_FEATURE >= 19
    PRINT_OFFSET(jvmtiEventCallbacks, VirtualThreadStart);
    PRINT_OFFSET(jvmtiEventCallbacks, VirtualThreadEnd);
#endif

    PRINT_SIZE(struct JNINativeInterface_);
    PRINT_ALIGN(struct JNINativeInterface_);
    PRINT_OFFSET(struct JNINativeInterface_, GetVersion);
    PRINT_OFFSET(struct JNINativeInterface_, GetObjectRefType);
#if PROBE_JDK_FEATURE >= 9
    PRINT_OFFSET(struct JNINativeInterface_, GetModule);
#endif
#if PROBE_JDK_FEATURE >= 19
    PRINT_OFFSET(struct JNINativeInterface_, IsVirtualThread);
#endif
#if PROBE_JDK_FEATURE >= 24
    PRINT_OFFSET(struct JNINativeInterface_, GetStringUTFLengthAsLong);
#endif
#if PROBE_JDK_FEATURE >= 28
    PRINT_OFFSET(struct JNINativeInterface_, HasIdentity);
#endif

    PRINT_SIZE(struct jvmtiInterface_1_);
    PRINT_ALIGN(struct jvmtiInterface_1_);
    PRINT_OFFSET(struct jvmtiInterface_1_, SetEventNotificationMode);
    PRINT_OFFSET(struct jvmtiInterface_1_, GetVersionNumber);
    PRINT_OFFSET(struct jvmtiInterface_1_, SetEventCallbacks);
#if PROBE_JDK_FEATURE >= 9
    PRINT_OFFSET(struct jvmtiInterface_1_, GetAllModules);
#endif
#if PROBE_JDK_FEATURE >= 19
    PRINT_OFFSET(struct jvmtiInterface_1_, SuspendAllVirtualThreads);
    PRINT_OFFSET(struct jvmtiInterface_1_, ResumeAllVirtualThreads);
#endif
#if PROBE_JDK_FEATURE >= 25
    PRINT_OFFSET(struct jvmtiInterface_1_, ClearAllFramePops);
#endif
#if PROBE_JDK_FEATURE >= 11
    PRINT_OFFSET(struct jvmtiInterface_1_, SetHeapSamplingInterval);
#endif

    return 0;
}
