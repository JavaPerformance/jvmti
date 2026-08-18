use std::mem::{align_of, size_of};
use std::os::raw::c_uint;

use jvmti_bindings::sys::{jni, jvmti};

#[test]
fn jobject_reference_kind_preserves_future_values() {
    let future = jni::jobjectRefType::from_raw(99);
    assert_eq!(future.raw(), 99);
    assert_eq!(format!("{future:?}"), "JNIUnknownRefType(99)");
    assert_eq!(
        jni::jobjectRefType::JNIGlobalRefType.raw(),
        2,
        "known JNI names remain available as associated constants"
    );
}

#[test]
fn open_native_domains_match_their_c_integer_abi() {
    assert_eq!(size_of::<jni::jobjectRefType>(), size_of::<c_uint>());
    assert_eq!(align_of::<jni::jobjectRefType>(), align_of::<c_uint>());
    assert_eq!(size_of::<jvmti::jvmtiEvent>(), size_of::<jni::jint>());
}
