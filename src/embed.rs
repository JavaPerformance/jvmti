//! Helpers for embedding a JVM inside a Rust process.
//!
//! This module is feature-gated behind `embed` to keep the core crate
//! dependency-free by default.

use std::ffi::{CString, NulError};
use std::path::{Path, PathBuf};
use std::ptr;

use crate::env::JniEnv;
use crate::sys::jni;

/// Errors returned by the embedding helpers.
#[derive(Debug)]
pub enum EmbedError {
    Nul(NulError),
    Load(String),
    Jni(jni::jint),
    Locate(String),
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbedError::Nul(e) => write!(f, "invalid option (NUL byte): {e}"),
            EmbedError::Load(e) => write!(f, "failed to load libjvm: {e}"),
            EmbedError::Jni(code) => write!(f, "JNI error: {code}"),
            EmbedError::Locate(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for EmbedError {}

impl From<NulError> for EmbedError {
    fn from(value: NulError) -> Self {
        EmbedError::Nul(value)
    }
}

fn libjvm_filename() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "jvm.dll"
    }
    #[cfg(target_os = "macos")]
    {
        "libjvm.dylib"
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "libjvm.so"
    }
}

fn candidates_from_java_home(java_home: &Path) -> Vec<PathBuf> {
    let filename = libjvm_filename();
    let arch = std::env::consts::ARCH;

    let mut rels = vec![
        format!("lib/server/{filename}"),
        format!("jre/lib/server/{filename}"),
        format!("lib/{arch}/server/{filename}"),
        format!("jre/lib/{arch}/server/{filename}"),
    ];

    if cfg!(target_os = "windows") {
        rels.push(format!("bin/server/{filename}"));
        rels.push(format!("jre/bin/server/{filename}"));
        rels.push(format!("bin/client/{filename}"));
        rels.push(format!("jre/bin/client/{filename}"));
    }

    rels.into_iter().map(|r| java_home.join(r)).collect()
}

/// Try to locate `libjvm` using `JVM_LIB_PATH` or `JAVA_HOME`.
pub fn find_libjvm() -> Result<PathBuf, EmbedError> {
    if let Some(path) = std::env::var_os("JVM_LIB_PATH") {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
        return Err(EmbedError::Locate(format!(
            "JVM_LIB_PATH is set but does not exist: {}",
            path.display()
        )));
    }

    if let Some(java_home) = std::env::var_os("JAVA_HOME") {
        let java_home = PathBuf::from(java_home);
        for candidate in candidates_from_java_home(&java_home) {
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        return Err(EmbedError::Locate(format!(
            "Could not find {} under JAVA_HOME={}. Set JVM_LIB_PATH explicitly.",
            libjvm_filename(),
            java_home.display()
        )));
    }

    Err(EmbedError::Locate(
        "JAVA_HOME is not set. Set JAVA_HOME or JVM_LIB_PATH to locate libjvm.".to_string(),
    ))
}

/// Builder for creating an embedded JVM.
pub struct JavaVmBuilder {
    version: jni::jint,
    options: Vec<CString>,
    ignore_unrecognized: bool,
}

impl JavaVmBuilder {
    /// Create a new builder for the given JNI version (e.g. `jni::JNI_VERSION_1_8`).
    pub fn new(version: jni::jint) -> Self {
        Self {
            version,
            options: Vec::new(),
            ignore_unrecognized: false,
        }
    }

    /// Add a JVM option like `-Xmx1g` or `-Dkey=value`.
    pub fn option(mut self, opt: &str) -> Result<Self, NulError> {
        self.options.push(CString::new(opt)?);
        Ok(self)
    }

    /// Add multiple JVM options.
    pub fn options<I, S>(mut self, opts: I) -> Result<Self, NulError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for opt in opts {
            self.options.push(CString::new(opt.as_ref())?);
        }
        Ok(self)
    }

    /// Set whether unrecognized options should be ignored.
    pub fn ignore_unrecognized(mut self, value: bool) -> Self {
        self.ignore_unrecognized = value;
        self
    }

    fn build_args(&mut self) -> (jni::JavaVMInitArgs, Vec<jni::JavaVMOption>) {
        let mut opt_structs: Vec<jni::JavaVMOption> = self
            .options
            .iter_mut()
            .map(|s| jni::JavaVMOption {
                optionString: s.as_ptr() as *mut std::os::raw::c_char,
                extraInfo: ptr::null_mut(),
            })
            .collect();

        let args = jni::JavaVMInitArgs {
            version: self.version,
            nOptions: opt_structs.len() as jni::jint,
            options: if opt_structs.is_empty() {
                ptr::null_mut()
            } else {
                opt_structs.as_mut_ptr()
            },
            ignoreUnrecognized: if self.ignore_unrecognized { 1 } else { 0 },
        };

        (args, opt_structs)
    }

    /// Create a JVM using a raw `JNI_CreateJavaVM` function pointer.
    ///
    /// # Safety
    /// The caller must ensure the function pointer is valid and the JVM
    /// shared library remains loaded for the lifetime of the returned `JavaVm`.
    pub unsafe fn create_with(self, create: jni::JNI_CreateJavaVM) -> Result<JavaVm, jni::jint> {
        let mut this = self;
        let (mut args, _opt_structs) = this.build_args();

        let mut vm: *mut jni::JavaVM = ptr::null_mut();
        let mut env: *mut jni::JNIEnv = ptr::null_mut();

        let res = create(&mut vm, &mut env, &mut args);
        if res != jni::JNI_OK {
            return Err(res);
        }
        if vm.is_null() || env.is_null() {
            return Err(jni::JNI_ERR);
        }

        Ok(JavaVm {
            vm,
            creator_env: env,
            destroyed: false,
            _lib: None,
        })
    }

    /// Create a JVM by dynamically loading `libjvm` from the given path.
    pub fn create_from_library<P: AsRef<Path>>(self, path: P) -> Result<JavaVm, EmbedError> {
        let lib = unsafe {
            libloading::Library::new(path.as_ref())
                .map_err(|e| EmbedError::Load(e.to_string()))?
        };

        let create: libloading::Symbol<jni::JNI_CreateJavaVM> = unsafe {
            lib.get(b"JNI_CreateJavaVM\0")
                .map_err(|e| EmbedError::Load(e.to_string()))?
        };

        let vm = unsafe { self.create_with(*create).map_err(EmbedError::Jni)? };
        Ok(JavaVm {
            _lib: Some(lib),
            ..vm
        })
    }

    /// Create a JVM by locating `libjvm` from `JVM_LIB_PATH` or `JAVA_HOME`.
    pub fn create(self) -> Result<JavaVm, EmbedError> {
        let path = find_libjvm()?;
        self.create_from_library(path)
    }

    /// Create a JVM using a specific `JAVA_HOME`.
    pub fn create_from_java_home<P: AsRef<Path>>(self, java_home: P) -> Result<JavaVm, EmbedError> {
        let java_home = java_home.as_ref();
        let candidate = candidates_from_java_home(java_home)
            .into_iter()
            .find(|p| p.exists())
            .ok_or_else(|| {
                EmbedError::Locate(format!(
                    "Could not find {} under JAVA_HOME={}.",
                    libjvm_filename(),
                    java_home.display()
                ))
            })?;
        self.create_from_library(candidate)
    }
}

/// Embedded JVM handle.
///
/// The `creator_env` is only valid on the thread that created the JVM.
pub struct JavaVm {
    vm: *mut jni::JavaVM,
    creator_env: *mut jni::JNIEnv,
    destroyed: bool,
    _lib: Option<libloading::Library>,
}

impl JavaVm {
    /// Return the raw `JavaVM*` pointer.
    pub fn java_vm_ptr(&self) -> *mut jni::JavaVM {
        self.vm
    }

    /// Return the raw `JNIEnv*` for the thread that created the JVM.
    pub fn creator_env_ptr(&self) -> *mut jni::JNIEnv {
        self.creator_env
    }

    /// Wrap the creator thread's `JNIEnv*` in a `JniEnv`.
    ///
    /// # Safety
    /// This is only valid on the thread that created the JVM.
    pub unsafe fn creator_env(&self) -> JniEnv {
        JniEnv::from_raw(self.creator_env)
    }

    /// Attach the current thread to the JVM and return a `JniEnv`.
    pub fn attach_current_thread(&self) -> Result<JniEnv, jni::jint> {
        let mut env_ptr: *mut std::os::raw::c_void = ptr::null_mut();
        let res = unsafe { crate::jvm_call!(self.vm, AttachCurrentThread, &mut env_ptr, ptr::null_mut()) };
        if res != jni::JNI_OK || env_ptr.is_null() {
            return Err(res);
        }
        Ok(unsafe { JniEnv::from_raw(env_ptr as *mut jni::JNIEnv) })
    }

    /// Detach the current thread from the JVM.
    pub fn detach_current_thread(&self) -> Result<(), jni::jint> {
        let res = unsafe { crate::jvm_call!(self.vm, DetachCurrentThread) };
        if res != jni::JNI_OK {
            return Err(res);
        }
        Ok(())
    }

    /// Destroy the JVM (explicit shutdown).
    pub fn destroy(mut self) -> Result<(), jni::jint> {
        let res = unsafe { crate::jvm_call!(self.vm, DestroyJavaVM) };
        if res != jni::JNI_OK {
            return Err(res);
        }
        self.destroyed = true;
        Ok(())
    }
}

impl Drop for JavaVm {
    fn drop(&mut self) {
        if self.destroyed {
            return;
        }
        if !self.vm.is_null() {
            unsafe {
                let _ = crate::jvm_call!(self.vm, DestroyJavaVM);
            }
        }
    }
}
