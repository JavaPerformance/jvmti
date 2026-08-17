//! Callback-scoped contexts and complete JVMTI event payloads.
//!
//! Values in this module borrow JVM-owned state for one callback invocation.
//! Raw JNI/JVMTI handles may be copied, but using them after the callback is
//! valid only when the corresponding JVM specification explicitly permits it.

use crate::env::{JniEnv, Jvmti};
use crate::sys::{jni, jvmti};
use crate::version::{
    jni_version_feature, jvmti_interface_feature, runtime_support, RuntimeSupport,
};
use std::ffi::{c_char, c_void, CStr};
use std::marker::PhantomData;
use std::ops::Deref;
use std::rc::Rc;

/// A borrowed JVMTI environment valid for the current callback.
pub struct JvmtiRef<'callback> {
    inner: Jvmti,
    _lifetime: PhantomData<&'callback mut jvmti::jvmtiEnv>,
    _not_send_sync: PhantomData<Rc<()>>,
}

impl<'callback> JvmtiRef<'callback> {
    pub(crate) unsafe fn from_raw(raw: *mut jvmti::jvmtiEnv) -> Option<Self> {
        if raw.is_null() {
            return None;
        }
        Some(Self {
            inner: unsafe { Jvmti::from_raw(raw) },
            _lifetime: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    pub fn raw(&self) -> *mut jvmti::jvmtiEnv {
        self.inner.raw()
    }
}

impl Deref for JvmtiRef<'_> {
    type Target = Jvmti;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// A borrowed JNI environment valid on the callback's current JVM thread.
pub struct JniEnvRef<'callback> {
    inner: JniEnv,
    _lifetime: PhantomData<&'callback mut jni::JNIEnv>,
}

impl<'callback> JniEnvRef<'callback> {
    pub(crate) unsafe fn from_raw(raw: *mut jni::JNIEnv) -> Option<Self> {
        if raw.is_null() {
            return None;
        }
        Some(Self {
            inner: unsafe { JniEnv::from_raw(raw) },
            _lifetime: PhantomData,
        })
    }

    pub fn raw(&self) -> *mut jni::JNIEnv {
        self.inner.raw()
    }
}

impl Deref for JniEnvRef<'_> {
    type Target = JniEnv;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// The native environments supplied for one JVMTI callback invocation.
#[non_exhaustive]
pub struct CallbackContext<'callback> {
    jvmti: JvmtiRef<'callback>,
    jni: Option<JniEnvRef<'callback>>,
}

impl<'callback> CallbackContext<'callback> {
    pub(crate) unsafe fn from_raw(
        jvmti: *mut jvmti::jvmtiEnv,
        jni: *mut jni::JNIEnv,
    ) -> Option<Self> {
        Some(Self {
            jvmti: unsafe { JvmtiRef::from_raw(jvmti)? },
            jni: unsafe { JniEnvRef::from_raw(jni) },
        })
    }

    pub fn jvmti(&self) -> &JvmtiRef<'callback> {
        &self.jvmti
    }

    pub fn jni(&self) -> Option<&JniEnvRef<'callback>> {
        self.jni.as_ref()
    }

    /// Encoded JVM TI version reported by the environment that delivered this
    /// callback.
    pub fn jvmti_version(&self) -> Result<jni::jint, jvmti::jvmtiError> {
        self.jvmti.get_version_number()
    }

    /// Feature milestone encoded in [`Self::jvmti_version`].
    ///
    /// This is an interface milestone, not always the exact Java release:
    /// JDK 10 reports milestone 9 and JDK 12 reports milestone 11.
    pub fn jvmti_interface_feature(&self) -> Result<u16, jvmti::jvmtiError> {
        self.jvmti_version().map(jvmti_interface_feature)
    }

    /// JNI version supplied to this callback, when this callback has a JNI
    /// environment.
    pub fn jni_version(&self) -> Option<jni::jint> {
        self.jni.as_ref().map(|env| env.get_version())
    }

    /// Feature milestone represented by [`Self::jni_version`].
    pub fn jni_interface_feature(&self) -> Option<u16> {
        self.jni_version().map(jni_version_feature)
    }

    /// Verification status of the callback's JVM TI interface milestone.
    ///
    /// This cannot prove the exact Java feature release because JDK 10 and 12
    /// reuse the preceding JVM TI interface revisions.
    pub fn jvmti_interface_support(&self) -> Result<RuntimeSupport, jvmti::jvmtiError> {
        self.jvmti_interface_feature().map(runtime_support)
    }
}

macro_rules! simple_event {
    ($(#[$meta:meta])* $name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[non_exhaustive]
        #[derive(Copy, Clone)]
        pub struct $name {
            $($field: $ty),*
        }

        impl $name {
            // Constructors mirror the complete native callback payload; some
            // events legitimately exceed Clippy's generic argument threshold.
            #[allow(clippy::too_many_arguments)]
            pub(crate) fn new($($field: $ty),*) -> Self {
                Self { $($field),* }
            }

            $(pub fn $field(&self) -> $ty { self.$field })*
        }
    };
}

simple_event!(
    /// Thread supplied by a lifecycle event.
    ThreadEvent {
        thread: jni::jthread
    }
);

simple_event!(
    /// Thread and class supplied by class-load and class-prepare events.
    ClassEvent {
        thread: jni::jthread,
        class: jni::jclass
    }
);

simple_event!(
    /// Thread and method supplied by method-entry events.
    MethodEvent {
        thread: jni::jthread,
        method: jni::jmethodID
    }
);

simple_event!(
    /// A bytecode location supplied by step and breakpoint events.
    LocationEvent {
        thread: jni::jthread,
        method: jni::jmethodID,
        location: jvmti::jlocation
    }
);

simple_event!(
    /// A frame-pop notification.
    FramePopEvent {
        thread: jni::jthread,
        method: jni::jmethodID,
        was_popped_by_exception_raw: jni::jboolean
    }
);

impl FramePopEvent {
    pub fn was_popped_by_exception(&self) -> bool {
        self.was_popped_by_exception_raw != jni::JNI_FALSE
    }
}

impl ObjectAllocationEvent {
    /// Returns the allocated object when the JVM exposes an identity-bearing
    /// reference. Value-object allocation events may legitimately supply null.
    pub fn object_opt(&self) -> Option<jni::jobject> {
        (!self.object.is_null()).then_some(self.object)
    }
}

simple_event!(
    /// A thrown exception and its prospective catch location.
    ExceptionEvent {
        thread: jni::jthread,
        method: jni::jmethodID,
        location: jvmti::jlocation,
        exception: jni::jobject,
        catch_method: jni::jmethodID,
        catch_location: jvmti::jlocation
    }
);

simple_event!(
    /// An exception caught at a bytecode location.
    ExceptionCatchEvent {
        thread: jni::jthread,
        method: jni::jmethodID,
        location: jvmti::jlocation,
        exception: jni::jobject
    }
);

simple_event!(
    /// A method exit, including the value omitted by the 2.x API.
    MethodExitEvent {
        thread: jni::jthread,
        method: jni::jmethodID,
        was_popped_by_exception_raw: jni::jboolean,
        return_value_raw: jni::jvalue
    }
);

impl MethodExitEvent {
    pub fn was_popped_by_exception(&self) -> bool {
        self.was_popped_by_exception_raw != jni::JNI_FALSE
    }

    /// Return the raw JNI value only for a normal method exit.
    pub fn return_value(&self) -> Option<jni::jvalue> {
        (!self.was_popped_by_exception()).then_some(self.return_value_raw)
    }
}

simple_event!(
    /// A native method implementation binding.
    NativeMethodBindEvent {
        thread: jni::jthread,
        method: jni::jmethodID,
        address: *mut c_void,
        new_address_ptr: *mut *mut c_void
    }
);

impl NativeMethodBindEvent {
    /// Replace the implementation selected by the JVM.
    ///
    /// # Safety
    ///
    /// `address` must be a valid native implementation with the exact JNI ABI
    /// expected by `method` and must remain valid while the JVM can call it.
    pub unsafe fn set_new_address(&mut self, address: *mut c_void) {
        if !self.new_address_ptr.is_null() {
            unsafe { *self.new_address_ptr = address };
        }
    }
}

simple_event!(
    /// A field read watchpoint.
    FieldAccessEvent {
        thread: jni::jthread,
        method: jni::jmethodID,
        location: jvmti::jlocation,
        field_class: jni::jclass,
        object: jni::jobject,
        field: jni::jfieldID
    }
);

simple_event!(
    /// A field write watchpoint.
    FieldModificationEvent {
        thread: jni::jthread,
        method: jni::jmethodID,
        location: jvmti::jlocation,
        field_class: jni::jclass,
        object: jni::jobject,
        field: jni::jfieldID,
        signature_type: c_char,
        new_value: jni::jvalue
    }
);

simple_event!(
    /// A thread entering or leaving monitor contention.
    MonitorEvent {
        thread: jni::jthread,
        object: jni::jobject
    }
);

simple_event!(
    /// A thread preparing to wait on a monitor.
    MonitorWaitEvent {
        thread: jni::jthread,
        object: jni::jobject,
        timeout: jni::jlong
    }
);

simple_event!(
    /// Completion of a monitor wait.
    MonitorWaitedEvent {
        thread: jni::jthread,
        object: jni::jobject,
        timed_out_raw: jni::jboolean
    }
);

impl MonitorWaitedEvent {
    pub fn timed_out(&self) -> bool {
        self.timed_out_raw != jni::JNI_FALSE
    }
}

simple_event!(
    /// A tagged object reclaimed by the garbage collector.
    ObjectFreeEvent { tag: jni::jlong }
);

simple_event!(
    /// A VM or sampled object allocation.
    ObjectAllocationEvent {
        thread: jni::jthread,
        object: jni::jobject,
        class: jni::jclass,
        size: jni::jlong
    }
);

simple_event!(
    /// Unloading of a compiled method body.
    CompiledMethodUnloadEvent {
        method: jni::jmethodID,
        code_address: *const c_void
    }
);

/// Class bytes and writable transformation outputs supplied to a load hook.
#[non_exhaustive]
pub struct ClassFileLoadHookEvent<'callback> {
    class_being_redefined: jni::jclass,
    loader: jni::jobject,
    name: *const c_char,
    protection_domain: jni::jobject,
    class_data_length: jni::jint,
    class_data: *const u8,
    new_class_data_length: *mut jni::jint,
    new_class_data: *mut *mut u8,
    _lifetime: PhantomData<&'callback [u8]>,
}

impl<'callback> ClassFileLoadHookEvent<'callback> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        class_being_redefined: jni::jclass,
        loader: jni::jobject,
        name: *const c_char,
        protection_domain: jni::jobject,
        class_data_length: jni::jint,
        class_data: *const u8,
        new_class_data_length: *mut jni::jint,
        new_class_data: *mut *mut u8,
    ) -> Self {
        Self {
            class_being_redefined,
            loader,
            name,
            protection_domain,
            class_data_length,
            class_data,
            new_class_data_length,
            new_class_data,
            _lifetime: PhantomData,
        }
    }

    pub fn class_being_redefined(&self) -> jni::jclass {
        self.class_being_redefined
    }
    pub fn loader(&self) -> jni::jobject {
        self.loader
    }
    pub fn name(&self) -> Option<&'callback CStr> {
        (!self.name.is_null()).then(|| unsafe { CStr::from_ptr(self.name) })
    }
    pub fn protection_domain(&self) -> jni::jobject {
        self.protection_domain
    }
    pub fn class_data(&self) -> &'callback [u8] {
        if self.class_data.is_null() || self.class_data_length <= 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.class_data, self.class_data_length as usize) }
        }
    }

    /// Copies transformed class bytes into JVM TI memory and transfers that
    /// allocation to the JVM.
    pub fn set_transformed_class(
        &mut self,
        context: &CallbackContext<'callback>,
        bytes: &[u8],
    ) -> Result<(), jvmti::jvmtiError> {
        if self.new_class_data.is_null() || self.new_class_data_length.is_null() {
            return Err(jvmti::jvmtiError::NULL_POINTER);
        }
        let length =
            jni::jint::try_from(bytes.len()).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)?;
        let mut allocation = context.jvmti().allocate(bytes.len())?;
        allocation.as_mut_slice().copy_from_slice(bytes);
        let data = unsafe { allocation.into_raw() };
        unsafe {
            *self.new_class_data = data;
            *self.new_class_data_length = length;
        }
        Ok(())
    }
}

/// JIT code and address-to-bytecode map supplied during compilation.
#[non_exhaustive]
pub struct CompiledMethodLoadEvent<'callback> {
    method: jni::jmethodID,
    code_size: jni::jint,
    code_address: *const c_void,
    map_length: jni::jint,
    map: *const jvmti::jvmtiAddrLocationMap,
    compile_info: *const c_void,
    _lifetime: PhantomData<&'callback jvmti::jvmtiAddrLocationMap>,
}

impl<'callback> CompiledMethodLoadEvent<'callback> {
    pub(crate) fn new(
        method: jni::jmethodID,
        code_size: jni::jint,
        code_address: *const c_void,
        map_length: jni::jint,
        map: *const jvmti::jvmtiAddrLocationMap,
        compile_info: *const c_void,
    ) -> Self {
        Self {
            method,
            code_size,
            code_address,
            map_length,
            map,
            compile_info,
            _lifetime: PhantomData,
        }
    }

    pub fn method(&self) -> jni::jmethodID {
        self.method
    }
    pub fn code_size(&self) -> jni::jint {
        self.code_size
    }
    pub fn code_address(&self) -> *const c_void {
        self.code_address
    }
    pub fn map(&self) -> &'callback [jvmti::jvmtiAddrLocationMap] {
        if self.map.is_null() || self.map_length <= 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.map, self.map_length as usize) }
        }
    }
    pub fn compile_info(&self) -> *const c_void {
        self.compile_info
    }
}

/// Dynamically generated non-method machine code.
#[non_exhaustive]
pub struct DynamicCodeGeneratedEvent<'callback> {
    name: *const c_char,
    address: *const c_void,
    length: jni::jint,
    _lifetime: PhantomData<&'callback CStr>,
}

impl<'callback> DynamicCodeGeneratedEvent<'callback> {
    pub(crate) fn new(name: *const c_char, address: *const c_void, length: jni::jint) -> Self {
        Self {
            name,
            address,
            length,
            _lifetime: PhantomData,
        }
    }

    pub fn name(&self) -> Option<&'callback CStr> {
        (!self.name.is_null()).then(|| unsafe { CStr::from_ptr(self.name) })
    }
    pub fn address(&self) -> *const c_void {
        self.address
    }
    pub fn length(&self) -> jni::jint {
        self.length
    }
}

/// A critical JVM resource exhaustion notification.
#[non_exhaustive]
pub struct ResourceExhaustedEvent<'callback> {
    flags: jni::jint,
    reserved: *const c_void,
    description: *const c_char,
    _lifetime: PhantomData<&'callback CStr>,
}

impl<'callback> ResourceExhaustedEvent<'callback> {
    pub(crate) fn new(
        flags: jni::jint,
        reserved: *const c_void,
        description: *const c_char,
    ) -> Self {
        Self {
            flags,
            reserved,
            description,
            _lifetime: PhantomData,
        }
    }

    pub fn flags(&self) -> jni::jint {
        self.flags
    }
    pub fn reserved(&self) -> *const c_void {
        self.reserved
    }
    pub fn description(&self) -> Option<&'callback CStr> {
        (!self.description.is_null()).then(|| unsafe { CStr::from_ptr(self.description) })
    }
}
