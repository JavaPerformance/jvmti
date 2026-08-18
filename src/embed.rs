//! Helpers for embedding a JVM inside a Rust process.
//!
//! This module is feature-gated behind `embed`; its platform loader is
//! implemented in-tree so enabling it does not add dependencies.

use std::ffi::{CString, NulError};
use std::path::{Path, PathBuf};
use std::ptr;

use crate::dynamic_library::DynamicLibrary;
use crate::env::JniEnv;
use crate::sys::jni;

/// Errors returned by the embedding helpers.
#[non_exhaustive]
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
            EmbedError::Jni(code) => write!(f, "JNI error {} ({code})", jni::result_name(*code)),
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

fn platform_hint() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Typical path: %JAVA_HOME%\\bin\\server\\jvm.dll"
    }
    #[cfg(target_os = "macos")]
    {
        "Typical path: $JAVA_HOME/lib/server/libjvm.dylib"
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "Typical path: $JAVA_HOME/lib/server/libjvm.so"
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
        let candidates = candidates_from_java_home(&java_home);
        for candidate in candidates.iter() {
            if candidate.exists() {
                return Ok(candidate.clone());
            }
        }
        return Err(EmbedError::Locate(format!(
            "Could not find {} under JAVA_HOME={}. {} Set JVM_LIB_PATH explicitly.",
            libjvm_filename(),
            java_home.display(),
            platform_hint()
        )));
    }

    Err(EmbedError::Locate(format!(
        "JAVA_HOME is not set. Set JAVA_HOME or JVM_LIB_PATH to locate libjvm. {}",
        platform_hint()
    )))
}

/// Like `find_libjvm`, but prints the discovered path to stderr.
pub fn find_libjvm_verbose() -> Result<PathBuf, EmbedError> {
    let path = find_libjvm()?;
    eprintln!("libjvm={}", path.display());
    Ok(path)
}

/// Builder for creating an embedded JVM.
pub struct JavaVmBuilder {
    version: jni::jint,
    options: Vec<CString>,
    ignore_unrecognized: bool,
}

impl Default for JavaVmBuilder {
    /// Create a builder using the Java 8 JNI baseline version.
    fn default() -> Self {
        Self::new(jni::JNI_VERSION_1_8)
    }
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

    fn build_args(&mut self) -> Result<(jni::JavaVMInitArgs, Vec<jni::JavaVMOption>), jni::jint> {
        let mut opt_structs: Vec<jni::JavaVMOption> = self
            .options
            .iter_mut()
            .map(|s| jni::JavaVMOption {
                optionString: s.as_ptr() as *mut std::os::raw::c_char,
                extraInfo: ptr::null_mut(),
            })
            .collect();

        // Zero the complete C structure, including padding, before assigning
        // fields. This keeps native diagnostics and tools from observing
        // indeterminate bytes when a JVM copies the invocation arguments.
        // SAFETY: Every field in `JavaVMInitArgs` is an integer or pointer, for
        // which the all-zero representation is valid.
        let mut args: jni::JavaVMInitArgs = unsafe { std::mem::zeroed() };
        args.version = self.version;
        args.nOptions = jni::jint::try_from(opt_structs.len()).map_err(|_| jni::JNI_EINVAL)?;
        args.options = if opt_structs.is_empty() {
            ptr::null_mut()
        } else {
            opt_structs.as_mut_ptr()
        };
        args.ignoreUnrecognized = if self.ignore_unrecognized { 1 } else { 0 };

        Ok((args, opt_structs))
    }

    /// Create a JVM using a raw `JNI_CreateJavaVM` function pointer.
    ///
    /// # Safety
    /// The caller must ensure the function pointer is valid and the JVM
    /// shared library remains loaded for the lifetime of the returned `JavaVm`.
    pub unsafe fn create_with(self, create: jni::JNI_CreateJavaVM) -> Result<JavaVm, jni::jint> {
        let mut this = self;
        let (mut args, option_structs) = this.build_args()?;

        let mut vm: *mut jni::JavaVM = ptr::null_mut();
        let mut env: *mut jni::JNIEnv = ptr::null_mut();

        // SAFETY: Forwarded from this function's contract. All output pointers
        // and the initialization arguments remain valid for the call.
        let res = unsafe { create(&mut vm, &mut env, &mut args) };
        if res != jni::JNI_OK {
            return Err(res);
        }
        if vm.is_null() || env.is_null() {
            return Err(jni::JNI_ERR);
        }

        Ok(JavaVm {
            vm,
            creator_env: env,
            destroy_on_drop: true,
            _options: this.options,
            _option_structs: option_structs,
            _lib: None,
        })
    }

    /// Create a JVM by dynamically loading `libjvm` from the given path.
    pub fn create_from_library<P: AsRef<Path>>(self, path: P) -> Result<JavaVm, EmbedError> {
        let lib = DynamicLibrary::open(path.as_ref()).map_err(EmbedError::Load)?;

        // SAFETY: `JNI_CreateJavaVM` has the JNI invocation API signature, and
        // `lib` remains owned by the returned `JavaVm` until after destruction.
        let create: jni::JNI_CreateJavaVM =
            unsafe { lib.symbol(c"JNI_CreateJavaVM").map_err(EmbedError::Load)? };

        // SAFETY: The symbol came from the still-live JVM library above.
        let mut vm = unsafe { self.create_with(create).map_err(EmbedError::Jni)? };
        vm._lib = Some(lib);
        Ok(vm)
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

/// RAII guard for a JNI environment on the current native thread.
///
/// If the guard had to attach the thread, it detaches the thread on drop. If
/// the thread was already attached, drop leaves the thread attached.
pub struct AttachedThread<'vm> {
    vm: &'vm JavaVm,
    env: JniEnv,
    detach_on_drop: bool,
}

impl AttachedThread<'_> {
    /// Borrow the current thread's JNI environment.
    pub fn env(&self) -> &JniEnv {
        &self.env
    }

    /// Return the raw `JNIEnv*` pointer for the current thread.
    pub fn env_ptr(&self) -> *mut jni::JNIEnv {
        self.env.raw()
    }
}

impl Drop for AttachedThread<'_> {
    fn drop(&mut self) {
        if self.detach_on_drop {
            // SAFETY: this guard owns the only `JniEnv` produced by the attach
            // operation and drops it immediately after detaching.
            let _ = unsafe { self.vm.detach_current_thread() };
        }
    }
}

/// Embedded JVM handle.
///
/// The `creator_env` is only valid on the thread that created the JVM.
pub struct JavaVm {
    vm: *mut jni::JavaVM,
    creator_env: *mut jni::JNIEnv,
    destroy_on_drop: bool,
    // Some JVM implementations continue to observe invocation-option storage
    // during startup after `JNI_CreateJavaVM` returns. Keep both the strings
    // and the pointer-bearing C array alive until after VM destruction.
    _options: Vec<CString>,
    _option_structs: Vec<jni::JavaVMOption>,
    _lib: Option<DynamicLibrary>,
}

// JavaVM is the process-wide JNI invocation interface. It is valid to share
// for GetEnv/AttachCurrentThread/DetachCurrentThread calls; JNIEnv remains
// thread-local and is not Send/Sync through the JniEnv wrapper.
unsafe impl Send for JavaVm {}
unsafe impl Sync for JavaVm {}

impl JavaVm {
    fn preserve_support_for_live_vm(&mut self) {
        // A failed DestroyJavaVM means native code may still execute from the
        // loaded library and a JVM implementation may still retain option
        // pointers. Leaking this small support state is the only sound
        // fallback; unloading or freeing it could invalidate a live VM.
        std::mem::forget(std::mem::take(&mut self._options));
        std::mem::forget(std::mem::take(&mut self._option_structs));
        if let Some(library) = self._lib.take() {
            std::mem::forget(library);
        }
    }

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
        unsafe { JniEnv::from_raw(self.creator_env) }
    }

    /// Return the current thread's `JNIEnv*` if this thread is already attached.
    ///
    /// # Safety
    ///
    /// The returned wrapper must not outlive this VM, cross threads, or remain
    /// usable after the current thread is detached. Prefer
    /// [`Self::attach_current_thread_guard`] for a lifetime-bound wrapper.
    pub unsafe fn get_env(&self, version: jni::jint) -> Result<JniEnv, jni::jint> {
        let mut env_ptr: *mut std::os::raw::c_void = ptr::null_mut();
        let res = unsafe { crate::jvm_call!(self.vm, GetEnv, &mut env_ptr, version) };
        if res != jni::JNI_OK {
            return Err(res);
        }
        if env_ptr.is_null() {
            return Err(jni::JNI_ERR);
        }
        Ok(unsafe { JniEnv::from_raw(env_ptr as *mut jni::JNIEnv) })
    }

    fn attach_current_thread_inner(&self, daemon: bool) -> Result<JniEnv, jni::jint> {
        let mut env_ptr: *mut std::os::raw::c_void = ptr::null_mut();
        let res = unsafe {
            if daemon {
                crate::jvm_call!(
                    self.vm,
                    AttachCurrentThreadAsDaemon,
                    &mut env_ptr,
                    ptr::null_mut()
                )
            } else {
                crate::jvm_call!(self.vm, AttachCurrentThread, &mut env_ptr, ptr::null_mut())
            }
        };
        if res != jni::JNI_OK {
            return Err(res);
        }
        if env_ptr.is_null() {
            return Err(jni::JNI_ERR);
        }
        Ok(unsafe { JniEnv::from_raw(env_ptr as *mut jni::JNIEnv) })
    }

    /// Attach the current thread to the JVM and return a `JniEnv`.
    ///
    /// If this native thread was not already attached, the caller is
    /// responsible for later calling [`JavaVm::detach_current_thread`].
    /// Prefer [`JavaVm::attach_current_thread_guard`] when possible.
    ///
    /// # Safety
    ///
    /// The returned wrapper must not outlive this VM or the attachment, and no
    /// JNI operation may use it after [`Self::detach_current_thread`].
    pub unsafe fn attach_current_thread(&self) -> Result<JniEnv, jni::jint> {
        self.attach_current_thread_inner(false)
    }

    /// Attach the current thread as a daemon thread.
    ///
    /// If this native thread was not already attached, the caller is
    /// responsible for later calling [`JavaVm::detach_current_thread`].
    /// Prefer [`JavaVm::attach_current_thread_as_daemon_guard`] when possible.
    ///
    /// # Safety
    ///
    /// The returned wrapper must not outlive this VM or the attachment, and no
    /// JNI operation may use it after [`Self::detach_current_thread`].
    pub unsafe fn attach_current_thread_as_daemon(&self) -> Result<JniEnv, jni::jint> {
        self.attach_current_thread_inner(true)
    }

    fn attach_current_thread_guard_inner(
        &self,
        daemon: bool,
    ) -> Result<AttachedThread<'_>, jni::jint> {
        // SAFETY: the result is immediately placed in a guard borrowing this
        // VM, which prevents it from outliving the VM or an owned attachment.
        match unsafe { self.get_env(jni::JNI_VERSION_1_8) } {
            Ok(env) => Ok(AttachedThread {
                vm: self,
                env,
                detach_on_drop: false,
            }),
            Err(jni::JNI_EDETACHED) => {
                let env = self.attach_current_thread_inner(daemon)?;
                Ok(AttachedThread {
                    vm: self,
                    env,
                    detach_on_drop: true,
                })
            }
            Err(code) => Err(code),
        }
    }

    /// Ensure the current thread is attached and detach it automatically on drop.
    ///
    /// If the thread was already attached, the guard will not detach it.
    pub fn attach_current_thread_guard(&self) -> Result<AttachedThread<'_>, jni::jint> {
        self.attach_current_thread_guard_inner(false)
    }

    /// Ensure the current thread is daemon-attached and detach it automatically on drop.
    ///
    /// If the thread was already attached, the guard will not detach it.
    pub fn attach_current_thread_as_daemon_guard(&self) -> Result<AttachedThread<'_>, jni::jint> {
        self.attach_current_thread_guard_inner(true)
    }

    /// Run a closure with a valid `JNIEnv` for the current thread.
    ///
    /// Threads attached by this helper are detached when the closure returns.
    pub fn with_attached_current_thread<R, F>(&self, f: F) -> Result<R, jni::jint>
    where
        F: FnOnce(&JniEnv) -> R,
    {
        let guard = self.attach_current_thread_guard()?;
        Ok(f(guard.env()))
    }

    /// Run a closure with a valid daemon-attached `JNIEnv` for the current thread.
    ///
    /// Threads attached by this helper are detached when the closure returns.
    pub fn with_attached_current_thread_as_daemon<R, F>(&self, f: F) -> Result<R, jni::jint>
    where
        F: FnOnce(&JniEnv) -> R,
    {
        let guard = self.attach_current_thread_as_daemon_guard()?;
        Ok(f(guard.env()))
    }

    /// Detach the current thread from the JVM.
    ///
    /// # Safety
    ///
    /// No `JniEnv`, local reference, or JNI operation tied to the current
    /// attachment may be used after this call. Do not call this while an
    /// [`AttachedThread`] guard is live.
    pub unsafe fn detach_current_thread(&self) -> Result<(), jni::jint> {
        let res = unsafe { crate::jvm_call!(self.vm, DetachCurrentThread) };
        if res != jni::JNI_OK {
            return Err(res);
        }
        Ok(())
    }

    /// Destroy the JVM (explicit shutdown).
    ///
    /// This may block until all non-daemon threads have terminated. If the VM
    /// rejects shutdown, support storage is intentionally leaked so a live VM
    /// is never left referring to freed options or an unloaded `libjvm`.
    pub fn destroy(mut self) -> Result<(), jni::jint> {
        let res = unsafe { crate::jvm_call!(self.vm, DestroyJavaVM) };
        if res != jni::JNI_OK {
            self.preserve_support_for_live_vm();
            self.destroy_on_drop = false;
            return Err(res);
        }
        self.destroy_on_drop = false;
        Ok(())
    }

    /// Transfer the live JVM pointer without attempting shutdown.
    ///
    /// This deliberately leaks the owned dynamic-library handle and retained
    /// startup options. It is appropriate only when another subsystem assumes
    /// responsibility for the process-lifetime JVM. The returned pointer must
    /// be used according to the JNI Invocation API.
    pub fn into_raw(self) -> *mut jni::JavaVM {
        let vm = self.vm;
        std::mem::forget(self);
        vm
    }
}

impl Drop for JavaVm {
    fn drop(&mut self) {
        if !self.destroy_on_drop {
            return;
        }
        if !self.vm.is_null() {
            let result = unsafe { crate::jvm_call!(self.vm, DestroyJavaVM) };
            if result != jni::JNI_OK {
                self.preserve_support_for_live_vm();
            }
        }
        self.destroy_on_drop = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr::NonNull;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

    static DESTROY_CALLS: AtomicUsize = AtomicUsize::new(0);
    static CREATE_VM: AtomicPtr<jni::JavaVM> = AtomicPtr::new(ptr::null_mut());
    static DESTROY_TEST_LOCK: Mutex<()> = Mutex::new(());

    unsafe extern "system" fn succeed_destroy(_vm: *mut jni::JavaVM) -> jni::jint {
        DESTROY_CALLS.fetch_add(1, Ordering::SeqCst);
        jni::JNI_OK
    }

    unsafe extern "system" fn fail_destroy(_vm: *mut jni::JavaVM) -> jni::jint {
        DESTROY_CALLS.fetch_add(1, Ordering::SeqCst);
        jni::JNI_ERR
    }

    unsafe extern "system" fn fail_attach(
        _vm: *mut jni::JavaVM,
        _env: *mut *mut std::ffi::c_void,
        _args: *mut std::ffi::c_void,
    ) -> jni::jint {
        jni::JNI_ERR
    }

    unsafe extern "system" fn fail_detach(_vm: *mut jni::JavaVM) -> jni::jint {
        jni::JNI_ERR
    }

    unsafe extern "system" fn fail_get_env(
        _vm: *mut jni::JavaVM,
        _env: *mut *mut std::ffi::c_void,
        _version: jni::jint,
    ) -> jni::jint {
        jni::JNI_ERR
    }

    fn failing_destroy_vm() -> (JavaVm, *mut jni::JavaVM, Box<jni::JNIInvokeInterface_>) {
        let table = Box::new(jni::JNIInvokeInterface_ {
            reserved0: ptr::null_mut(),
            reserved1: ptr::null_mut(),
            reserved2: ptr::null_mut(),
            DestroyJavaVM: fail_destroy,
            AttachCurrentThread: fail_attach,
            DetachCurrentThread: fail_detach,
            GetEnv: fail_get_env,
            AttachCurrentThreadAsDaemon: fail_attach,
        });
        let vm_slot = Box::into_raw(Box::new((&*table) as *const jni::JNIInvokeInterface_));
        (
            JavaVm {
                vm: vm_slot,
                creator_env: ptr::null_mut(),
                destroy_on_drop: true,
                _options: Vec::new(),
                _option_structs: Vec::new(),
                _lib: None,
            },
            vm_slot,
            table,
        )
    }

    unsafe extern "system" fn fake_create(
        vm: *mut *mut jni::JavaVM,
        env: *mut *mut jni::JNIEnv,
        _args: *mut jni::JavaVMInitArgs,
    ) -> jni::jint {
        let configured_vm = CREATE_VM.load(Ordering::SeqCst);
        if configured_vm.is_null() {
            return jni::JNI_ERR;
        }
        unsafe {
            *vm = configured_vm;
            *env = NonNull::<jni::JNIEnv>::dangling().as_ptr();
        }
        jni::JNI_OK
    }

    #[test]
    fn invocation_options_live_with_the_vm_handle() {
        let _guard = DESTROY_TEST_LOCK.lock().expect("destroy test lock");
        DESTROY_CALLS.store(0, Ordering::SeqCst);

        let table = Box::new(jni::JNIInvokeInterface_ {
            reserved0: ptr::null_mut(),
            reserved1: ptr::null_mut(),
            reserved2: ptr::null_mut(),
            DestroyJavaVM: succeed_destroy,
            AttachCurrentThread: fail_attach,
            DetachCurrentThread: fail_detach,
            GetEnv: fail_get_env,
            AttachCurrentThreadAsDaemon: fail_attach,
        });
        let vm_slot = Box::into_raw(Box::new((&*table) as *const jni::JNIInvokeInterface_));
        CREATE_VM.store(vm_slot, Ordering::SeqCst);

        let builder = JavaVmBuilder::default()
            .option("-Djvmti.bindings.option-lifetime=sentinel")
            .expect("valid option");
        // SAFETY: `fake_create` returns the valid test invocation table above,
        // which remains alive through the successful destroy operation.
        let vm = unsafe { builder.create_with(fake_create) }.expect("fake JVM creation");

        assert_eq!(vm._options.len(), 1);
        assert_eq!(vm._option_structs.len(), 1);
        assert_eq!(
            vm._options[0].as_c_str(),
            c"-Djvmti.bindings.option-lifetime=sentinel"
        );
        assert_eq!(
            vm._option_structs[0].optionString.cast_const(),
            vm._options[0].as_ptr()
        );

        drop(vm);
        assert_eq!(DESTROY_CALLS.load(Ordering::SeqCst), 1);
        CREATE_VM.store(ptr::null_mut(), Ordering::SeqCst);

        // SAFETY: `vm_slot` came from Box::into_raw and the JavaVm has already
        // called the test destroy function, so neither allocation is borrowed.
        drop(unsafe { Box::from_raw(vm_slot) });
        drop(table);
    }

    #[test]
    fn failed_explicit_destroy_is_not_retried_by_drop() {
        let _guard = DESTROY_TEST_LOCK.lock().expect("destroy test lock");
        DESTROY_CALLS.store(0, Ordering::SeqCst);
        let (vm, vm_slot, table) = failing_destroy_vm();
        assert_eq!(vm.destroy(), Err(jni::JNI_ERR));
        assert_eq!(DESTROY_CALLS.load(Ordering::SeqCst), 1);

        // SAFETY: `vm_slot` came from Box::into_raw above, and the disarmed
        // JavaVm no longer refers to it after the consuming destroy call.
        drop(unsafe { Box::from_raw(vm_slot) });
        drop(table);
    }

    #[test]
    fn failed_drop_attempts_destroy_once() {
        let _guard = DESTROY_TEST_LOCK.lock().expect("destroy test lock");
        DESTROY_CALLS.store(0, Ordering::SeqCst);
        let (vm, vm_slot, table) = failing_destroy_vm();
        drop(vm);
        assert_eq!(DESTROY_CALLS.load(Ordering::SeqCst), 1);

        // SAFETY: `vm_slot` came from Box::into_raw above and no JavaVm remains.
        drop(unsafe { Box::from_raw(vm_slot) });
        drop(table);
    }
}
