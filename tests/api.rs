use std::ptr;

use jvmti_bindings::env::Jvmti;
use jvmti_bindings::jni;

#[test]
fn jvmti_new_rejects_null_vm_pointer() {
    let err = match Jvmti::new(ptr::null_mut()) {
        Ok(_) => panic!("null JavaVM must be rejected"),
        Err(err) => err,
    };
    assert_eq!(err, jni::JNI_ERR);
}
