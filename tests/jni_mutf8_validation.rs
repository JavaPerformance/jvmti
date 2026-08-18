use std::ffi::CStr;
use std::mem::MaybeUninit;
use std::ptr;

use jvmti_bindings::env::JniEnv;
use jvmti_bindings::sys::jni;

fn invalid_mutf8() -> &'static CStr {
    CStr::from_bytes_with_nul(&[0xf0, 0x90, 0x80, 0x80, 0]).unwrap()
}

#[test]
fn safe_cstr_helpers_reject_invalid_modified_utf8_before_native_dispatch() {
    // Every function slot is intentionally uninitialized. Reaching native
    // dispatch would make this test fail under Miri/sanitizers or crash.
    let table = MaybeUninit::<jni::JNINativeInterface_>::uninit();
    let mut raw_env = table.as_ptr();
    let env = unsafe { JniEnv::from_raw(&mut raw_env) };

    assert!(env.find_class_cstr(invalid_mutf8()).is_none());
    assert!(env.new_string_utf_cstr(invalid_mutf8()).is_none());

    let class = ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr();
    assert_eq!(
        unsafe { env.throw_new_cstr(class, invalid_mutf8()) },
        Err(jni::JNI_EINVAL)
    );
    assert!(unsafe { env.get_method_id_cstr(class, invalid_mutf8(), c"()V") }.is_none());
    assert!(unsafe { env.get_static_method_id_cstr(class, c"ok", invalid_mutf8()) }.is_none());
    assert!(unsafe { env.get_field_id_cstr(class, invalid_mutf8(), c"I") }.is_none());
    assert!(unsafe { env.get_static_field_id_cstr(class, c"ok", invalid_mutf8()) }.is_none());
}
