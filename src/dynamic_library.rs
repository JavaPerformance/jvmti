//! Minimal dependency-free dynamic-library support for JVM embedding.

use std::ffi::{CStr, c_void};
use std::path::Path;
use std::ptr::NonNull;

pub(crate) struct DynamicLibrary {
    handle: NonNull<c_void>,
}

impl DynamicLibrary {
    pub(crate) fn open(path: &Path) -> Result<Self, String> {
        platform::open(path).map(|handle| Self { handle })
    }

    /// Resolves a symbol and copies its pointer representation into `T`.
    ///
    /// # Safety
    ///
    /// `T` must be the exact function-pointer type exported under `name`, and
    /// the returned pointer must not outlive this library.
    pub(crate) unsafe fn symbol<T: Copy>(&self, name: &CStr) -> Result<T, String> {
        if std::mem::size_of::<T>() != std::mem::size_of::<*mut c_void>() {
            return Err(format!(
                "symbol {} has an unsupported pointer representation",
                name.to_string_lossy()
            ));
        }
        let symbol = platform::symbol(self.handle, name)?;
        // SAFETY: The size check above establishes representation size. The
        // caller establishes that `T` is the symbol's exact function type.
        Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&symbol.as_ptr()) })
    }
}

impl Drop for DynamicLibrary {
    fn drop(&mut self) {
        platform::close(self.handle);
    }
}

#[cfg(unix)]
mod platform {
    use super::*;
    use std::ffi::{CString, c_char, c_int};
    use std::os::unix::ffi::OsStrExt;

    const RTLD_NOW: c_int = 2;

    #[cfg_attr(not(target_os = "macos"), link(name = "dl"))]
    unsafe extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
        fn dlerror() -> *const c_char;
    }

    pub(super) fn open(path: &Path) -> Result<NonNull<c_void>, String> {
        let path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| format!("library path contains a NUL byte: {}", path.display()))?;
        // SAFETY: `path` is NUL-terminated and valid for the duration of the
        // call. `RTLD_NOW` is a supported POSIX loader flag.
        NonNull::new(unsafe { dlopen(path.as_ptr(), RTLD_NOW) }).ok_or_else(last_error)
    }

    pub(super) fn symbol(handle: NonNull<c_void>, name: &CStr) -> Result<NonNull<c_void>, String> {
        // POSIX requires clearing any prior loader error before `dlsym`.
        unsafe { dlerror() };
        // SAFETY: `handle` is live and `name` is NUL-terminated.
        let symbol = unsafe { dlsym(handle.as_ptr(), name.as_ptr()) };
        // A non-null `dlerror` result is authoritative even on platforms where
        // a symbol could theoretically resolve to address zero.
        let error = unsafe { dlerror() };
        if !error.is_null() {
            // SAFETY: `dlerror` returns a thread-local NUL-terminated string
            // valid until the next loader call on this thread.
            return Err(unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned());
        }
        NonNull::new(symbol)
            .ok_or_else(|| format!("symbol {} resolved to null", name.to_string_lossy()))
    }

    pub(super) fn close(handle: NonNull<c_void>) {
        // SAFETY: `handle` was returned by `dlopen` and is closed exactly once.
        let _ = unsafe { dlclose(handle.as_ptr()) };
    }

    fn last_error() -> String {
        // SAFETY: A failed loader operation makes `dlerror` return either null
        // or a thread-local NUL-terminated diagnostic string.
        let error = unsafe { dlerror() };
        if error.is_null() {
            "dynamic loader returned no diagnostic".to_owned()
        } else {
            unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned()
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryW(filename: *const u16) -> *mut c_void;
        fn GetProcAddress(handle: *mut c_void, symbol: *const u8) -> *mut c_void;
        fn FreeLibrary(handle: *mut c_void) -> i32;
        fn GetLastError() -> u32;
    }

    pub(super) fn open(path: &Path) -> Result<NonNull<c_void>, String> {
        let mut path: Vec<u16> = path.as_os_str().encode_wide().collect();
        path.push(0);
        // SAFETY: `path` is a valid NUL-terminated UTF-16 string.
        NonNull::new(unsafe { LoadLibraryW(path.as_ptr()) }).ok_or_else(last_error)
    }

    pub(super) fn symbol(handle: NonNull<c_void>, name: &CStr) -> Result<NonNull<c_void>, String> {
        // SAFETY: `handle` is live and `name` is NUL-terminated ASCII.
        NonNull::new(unsafe { GetProcAddress(handle.as_ptr(), name.as_ptr().cast()) })
            .ok_or_else(last_error)
    }

    pub(super) fn close(handle: NonNull<c_void>) {
        // SAFETY: `handle` was returned by `LoadLibraryW` and is closed once.
        let _ = unsafe { FreeLibrary(handle.as_ptr()) };
    }

    fn last_error() -> String {
        // SAFETY: `GetLastError` has no preconditions.
        let code = unsafe { GetLastError() };
        std::io::Error::from_raw_os_error(code as i32).to_string()
    }
}

#[cfg(not(any(unix, windows)))]
compile_error!("the `embed` feature currently supports Unix and Windows targets");
