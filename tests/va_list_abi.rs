use jvmti_bindings::sys::jni::va_list;
use std::mem::{align_of, size_of};

#[test]
fn va_list_argument_layout_matches_the_target_abi() {
    #[cfg(all(
        target_arch = "aarch64",
        not(any(target_vendor = "apple", target_os = "windows"))
    ))]
    {
        assert_eq!(size_of::<va_list>(), 32);
        assert_eq!(align_of::<va_list>(), 8);
    }

    #[cfg(all(
        target_arch = "arm",
        not(any(target_vendor = "apple", target_os = "windows"))
    ))]
    {
        assert_eq!(size_of::<va_list>(), 4);
        assert_eq!(align_of::<va_list>(), 4);
    }

    #[cfg(not(any(
        all(
            target_arch = "aarch64",
            not(any(target_vendor = "apple", target_os = "windows"))
        ),
        all(
            target_arch = "arm",
            not(any(target_vendor = "apple", target_os = "windows"))
        )
    )))]
    {
        assert_eq!(size_of::<va_list>(), size_of::<*mut std::ffi::c_void>());
        assert_eq!(align_of::<va_list>(), align_of::<*mut std::ffi::c_void>());
    }
}
