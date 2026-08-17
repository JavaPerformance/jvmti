#include <jni.h>
#include <jvmti.h>
#include <string.h>

static void JNICALL method_entry(
    jvmtiEnv *jvmti,
    JNIEnv *jni,
    jthread thread,
    jmethodID method
) {
    (void)jvmti;
    (void)jni;
    (void)thread;
    (void)method;
}

JNIEXPORT jint JNICALL Agent_OnLoad(JavaVM *vm, char *options, void *reserved) {
    (void)options;
    (void)reserved;

    jvmtiEnv *jvmti = NULL;
    if ((*vm)->GetEnv(vm, (void **)&jvmti, JVMTI_VERSION_1_2) != JNI_OK || jvmti == NULL) {
        return JNI_ERR;
    }

    jvmtiCapabilities capabilities;
    memset(&capabilities, 0, sizeof(capabilities));
    capabilities.can_generate_method_entry_events = 1;
    if ((*jvmti)->AddCapabilities(jvmti, &capabilities) != JVMTI_ERROR_NONE) {
        return JNI_ERR;
    }

    jvmtiEventCallbacks callbacks;
    memset(&callbacks, 0, sizeof(callbacks));
    callbacks.MethodEntry = &method_entry;
    if ((*jvmti)->SetEventCallbacks(jvmti, &callbacks, sizeof(callbacks)) != JVMTI_ERROR_NONE) {
        return JNI_ERR;
    }

    if ((*jvmti)->SetEventNotificationMode(
            jvmti,
            JVMTI_ENABLE,
            JVMTI_EVENT_METHOD_ENTRY,
            NULL
        ) != JVMTI_ERROR_NONE) {
        return JNI_ERR;
    }

    return JNI_OK;
}
