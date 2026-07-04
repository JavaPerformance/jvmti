#![cfg(feature = "embed")]

use jvmti_bindings::embed::{EmbedError, JavaVmBuilder};
use jvmti_bindings::jni;

#[test]
fn jni_result_names_are_human_readable() {
    assert_eq!(jni::result_name(jni::JNI_OK), "JNI_OK");
    assert_eq!(jni::result_name(jni::JNI_EDETACHED), "JNI_EDETACHED");
    assert_eq!(jni::result_name(12345), "JNI_UNKNOWN");
}

#[test]
fn embed_error_displays_jni_result_name() {
    let rendered = EmbedError::Jni(jni::JNI_EVERSION).to_string();
    assert!(rendered.contains("JNI_EVERSION"));
    assert!(rendered.contains("-3"));
}

#[test]
fn default_builder_is_available_for_java8_baseline() {
    let _builder = JavaVmBuilder::default()
        .option("-Xms64m")
        .expect("valid option")
        .ignore_unrecognized(true);
}
