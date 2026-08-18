//! Callback-scoped agent lifecycle inputs.

use crate::env::Jvmti;
use crate::mutf8::{self, Mutf8Error};
use crate::sys::jni;
use std::borrow::Cow;
use std::ffi::{CStr, c_char, c_void};
use std::marker::PhantomData;
use std::rc::Rc;

/// A JVM-owned `JavaVM*` borrowed for one agent lifecycle callback.
///
/// The wrapper can only be constructed from raw state by trusted FFI code. It
/// makes operations such as [`Jvmti::new`] safe without claiming that an
/// arbitrary raw pointer supplied by safe Rust is valid.
pub struct JavaVmRef<'callback> {
    raw: *mut jni::JavaVM,
    _lifetime: PhantomData<&'callback mut jni::JavaVM>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl JavaVmRef<'_> {
    /// Construct a callback-scoped VM reference from the JVM entry-point
    /// argument.
    ///
    /// # Safety
    ///
    /// `raw` must be a valid `JavaVM*` supplied by the active JVM and remain
    /// valid for `'callback`.
    pub(crate) unsafe fn from_raw(raw: *mut jni::JavaVM) -> Option<Self> {
        if raw.is_null() {
            return None;
        }
        Some(Self {
            raw,
            _lifetime: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    /// Return the underlying VM pointer for APIs that explicitly require raw
    /// JNI interop.
    pub fn raw(&self) -> *mut jni::JavaVM {
        self.raw
    }

    /// Obtain the JVM TI environment associated with this VM.
    pub fn jvmti(&self) -> Result<Jvmti, jni::jint> {
        Jvmti::new(self)
    }
}

/// Inputs supplied to `Agent_OnLoad` or `Agent_OnAttach`.
#[non_exhaustive]
pub struct AgentLoadContext<'callback> {
    vm: JavaVmRef<'callback>,
    options: Option<&'callback CStr>,
    reserved: *mut c_void,
}

impl<'callback> AgentLoadContext<'callback> {
    pub(crate) unsafe fn from_raw(
        vm: *mut jni::JavaVM,
        options: *mut c_char,
        reserved: *mut c_void,
    ) -> Option<Self> {
        Some(Self {
            vm: unsafe { JavaVmRef::from_raw(vm)? },
            options: if options.is_null() {
                None
            } else {
                Some(unsafe { CStr::from_ptr(options) })
            },
            reserved,
        })
    }

    pub fn vm(&self) -> &JavaVmRef<'callback> {
        &self.vm
    }

    /// Return the exact NUL-terminated option string, or `None` when the JVM
    /// supplied a null pointer.
    pub fn options(&self) -> Option<&'callback CStr> {
        self.options
    }

    /// Return exact option bytes without the trailing NUL.
    pub fn option_bytes(&self) -> Option<&'callback [u8]> {
        self.options.map(CStr::to_bytes)
    }

    /// Decode the JVM's Modified UTF-8 options without replacement.
    ///
    /// Ordinary UTF-8-compatible options are borrowed. Options containing
    /// Java's special NUL or supplementary-character encodings are converted
    /// into an owned string.
    pub fn options_str(&self) -> Result<Option<Cow<'callback, str>>, Mutf8Error> {
        self.options.map(mutf8::decode_cstr_cow).transpose()
    }

    /// Explicitly request a lossy Modified UTF-8 view of the option bytes.
    pub fn options_lossy(&self) -> Option<Cow<'callback, str>> {
        self.options.map(|options| {
            mutf8::decode_cstr_cow(options)
                .unwrap_or_else(|_| Cow::Owned(mutf8::decode_cstr_lossy(options)))
        })
    }

    /// Opaque pointer reserved by the JVM specification for future use.
    pub fn reserved(&self) -> *mut c_void {
        self.reserved
    }
}

/// Inputs supplied to `Agent_OnUnload`.
#[non_exhaustive]
pub struct AgentUnloadContext<'callback> {
    vm: JavaVmRef<'callback>,
}

impl<'callback> AgentUnloadContext<'callback> {
    pub(crate) unsafe fn from_raw(vm: *mut jni::JavaVM) -> Option<Self> {
        Some(Self {
            vm: unsafe { JavaVmRef::from_raw(vm)? },
        })
    }

    pub fn vm(&self) -> &JavaVmRef<'callback> {
        &self.vm
    }
}
