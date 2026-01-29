//! Safe wrapper around the JNI environment.
//!
//! This module provides ergonomic Rust wrappers for common JNI operations.
//!
//! # Example
//!
//! ```rust,ignore
//! use jvmti::jni_wrapper::JniEnv;
//!
//! fn vm_init(jni: *mut jni::JNIEnv, thread: jni::jthread) {
//!     let env = unsafe { JniEnv::from_raw(jni) };
//!
//!     // Find a class
//!     let string_class = env.find_class("java/lang/String").unwrap();
//!
//!     // Create a string
//!     let greeting = env.new_string_utf("Hello from Rust!").unwrap();
//!
//!     // Check for exceptions
//!     if env.exception_check() {
//!         env.exception_describe();
//!         env.exception_clear();
//!     }
//! }
//! ```

use crate::sys::jni;
use std::ffi::{CStr, CString};
use std::ptr;

/// Safe wrapper around a JNI environment pointer.
///
/// This struct provides ergonomic access to JNI functions with proper
/// error handling and Rust-friendly types.
///
/// # Thread Safety
///
/// A `JniEnv` is tied to a specific thread and cannot be sent across threads.
/// Each JVM thread has its own JNI environment.
pub struct JniEnv {
    env: *mut jni::JNIEnv,
}

impl JniEnv {
    /// Creates a JniEnv wrapper from a raw pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure the pointer is valid and comes from the current thread.
    pub unsafe fn from_raw(env: *mut jni::JNIEnv) -> Self {
        JniEnv { env }
    }

    /// Returns the raw JNI environment pointer.
    pub fn raw(&self) -> *mut jni::JNIEnv {
        self.env
    }

    // =========================================================================
    // Version
    // =========================================================================

    /// Returns the JNI version.
    pub fn get_version(&self) -> jni::jint {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetVersion)(self.env)
        }
    }

    // =========================================================================
    // Class Operations
    // =========================================================================

    /// Finds a class by its fully qualified name.
    ///
    /// The name should use '/' as package separator (e.g., "java/lang/String").
    pub fn find_class(&self, name: &str) -> Option<jni::jclass> {
        let c_name = CString::new(name).ok()?;
        unsafe {
            let vtable = *self.env;
            let cls = ((*vtable).FindClass)(self.env, c_name.as_ptr());
            if cls.is_null() { None } else { Some(cls) }
        }
    }

    /// Gets the superclass of a class.
    pub fn get_superclass(&self, cls: jni::jclass) -> Option<jni::jclass> {
        unsafe {
            let vtable = *self.env;
            let super_cls = ((*vtable).GetSuperclass)(self.env, cls);
            if super_cls.is_null() { None } else { Some(super_cls) }
        }
    }

    /// Checks if `cls1` can be assigned to `cls2`.
    pub fn is_assignable_from(&self, cls1: jni::jclass, cls2: jni::jclass) -> bool {
        unsafe {
            let vtable = *self.env;
            ((*vtable).IsAssignableFrom)(self.env, cls1, cls2) != 0
        }
    }

    /// Gets the class of an object.
    pub fn get_object_class(&self, obj: jni::jobject) -> jni::jclass {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetObjectClass)(self.env, obj)
        }
    }

    /// Checks if an object is an instance of a class.
    pub fn is_instance_of(&self, obj: jni::jobject, cls: jni::jclass) -> bool {
        unsafe {
            let vtable = *self.env;
            ((*vtable).IsInstanceOf)(self.env, obj, cls) != 0
        }
    }

    // =========================================================================
    // Exception Handling
    // =========================================================================

    /// Checks if an exception is pending.
    pub fn exception_check(&self) -> bool {
        unsafe {
            let vtable = *self.env;
            ((*vtable).ExceptionCheck)(self.env) != 0
        }
    }

    /// Clears any pending exception.
    pub fn exception_clear(&self) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).ExceptionClear)(self.env);
        }
    }

    /// Prints the pending exception and stack trace to stderr.
    pub fn exception_describe(&self) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).ExceptionDescribe)(self.env);
        }
    }

    /// Gets the pending exception (if any).
    pub fn exception_occurred(&self) -> Option<jni::jthrowable> {
        unsafe {
            let vtable = *self.env;
            let exc = ((*vtable).ExceptionOccurred)(self.env);
            if exc.is_null() { None } else { Some(exc) }
        }
    }

    /// Throws an exception.
    pub fn throw(&self, obj: jni::jthrowable) -> Result<(), jni::jint> {
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).Throw)(self.env, obj);
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }

    /// Throws a new exception of the specified class with the given message.
    pub fn throw_new(&self, cls: jni::jclass, msg: &str) -> Result<(), jni::jint> {
        let c_msg = CString::new(msg).map_err(|_| -1)?;
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).ThrowNew)(self.env, cls, c_msg.as_ptr());
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }

    // =========================================================================
    // String Operations
    // =========================================================================

    /// Creates a new Java string from a Rust string.
    pub fn new_string_utf(&self, s: &str) -> Option<jni::jstring> {
        let c_str = CString::new(s).ok()?;
        unsafe {
            let vtable = *self.env;
            let jstr = ((*vtable).NewStringUTF)(self.env, c_str.as_ptr());
            if jstr.is_null() { None } else { Some(jstr) }
        }
    }

    /// Gets a Rust string from a Java string.
    ///
    /// Returns `None` if the string is null or contains invalid UTF-8.
    pub fn get_string_utf(&self, s: jni::jstring) -> Option<String> {
        if s.is_null() {
            return None;
        }
        unsafe {
            let vtable = *self.env;
            let chars = ((*vtable).GetStringUTFChars)(self.env, s, ptr::null_mut());
            if chars.is_null() {
                return None;
            }
            let result = CStr::from_ptr(chars).to_str().ok().map(|s| s.to_string());
            ((*vtable).ReleaseStringUTFChars)(self.env, s, chars);
            result
        }
    }

    /// Gets the UTF-8 length of a Java string.
    pub fn get_string_utf_length(&self, s: jni::jstring) -> jni::jsize {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetStringUTFLength)(self.env, s)
        }
    }

    /// Gets the length of a Java string (in UTF-16 code units).
    pub fn get_string_length(&self, s: jni::jstring) -> jni::jsize {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetStringLength)(self.env, s)
        }
    }

    // =========================================================================
    // Method IDs
    // =========================================================================

    /// Gets the method ID for an instance method.
    pub fn get_method_id(&self, cls: jni::jclass, name: &str, sig: &str) -> Option<jni::jmethodID> {
        let c_name = CString::new(name).ok()?;
        let c_sig = CString::new(sig).ok()?;
        unsafe {
            let vtable = *self.env;
            let mid = ((*vtable).GetMethodID)(self.env, cls, c_name.as_ptr(), c_sig.as_ptr());
            if mid.is_null() { None } else { Some(mid) }
        }
    }

    /// Gets the method ID for a static method.
    pub fn get_static_method_id(&self, cls: jni::jclass, name: &str, sig: &str) -> Option<jni::jmethodID> {
        let c_name = CString::new(name).ok()?;
        let c_sig = CString::new(sig).ok()?;
        unsafe {
            let vtable = *self.env;
            let mid = ((*vtable).GetStaticMethodID)(self.env, cls, c_name.as_ptr(), c_sig.as_ptr());
            if mid.is_null() { None } else { Some(mid) }
        }
    }

    // =========================================================================
    // Field IDs
    // =========================================================================

    /// Gets the field ID for an instance field.
    pub fn get_field_id(&self, cls: jni::jclass, name: &str, sig: &str) -> Option<jni::jfieldID> {
        let c_name = CString::new(name).ok()?;
        let c_sig = CString::new(sig).ok()?;
        unsafe {
            let vtable = *self.env;
            let fid = ((*vtable).GetFieldID)(self.env, cls, c_name.as_ptr(), c_sig.as_ptr());
            if fid.is_null() { None } else { Some(fid) }
        }
    }

    /// Gets the field ID for a static field.
    pub fn get_static_field_id(&self, cls: jni::jclass, name: &str, sig: &str) -> Option<jni::jfieldID> {
        let c_name = CString::new(name).ok()?;
        let c_sig = CString::new(sig).ok()?;
        unsafe {
            let vtable = *self.env;
            let fid = ((*vtable).GetStaticFieldID)(self.env, cls, c_name.as_ptr(), c_sig.as_ptr());
            if fid.is_null() { None } else { Some(fid) }
        }
    }

    // =========================================================================
    // Object Operations
    // =========================================================================

    /// Allocates a new object without calling any constructor.
    pub fn alloc_object(&self, cls: jni::jclass) -> Option<jni::jobject> {
        unsafe {
            let vtable = *self.env;
            let obj = ((*vtable).AllocObject)(self.env, cls);
            if obj.is_null() { None } else { Some(obj) }
        }
    }

    /// Creates a new object by calling the specified constructor.
    pub fn new_object(&self, cls: jni::jclass, method_id: jni::jmethodID, args: &[jni::jvalue]) -> Option<jni::jobject> {
        unsafe {
            let vtable = *self.env;
            let obj = ((*vtable).NewObjectA)(self.env, cls, method_id, args.as_ptr());
            if obj.is_null() { None } else { Some(obj) }
        }
    }

    /// Checks if two references refer to the same object.
    pub fn is_same_object(&self, ref1: jni::jobject, ref2: jni::jobject) -> bool {
        unsafe {
            let vtable = *self.env;
            ((*vtable).IsSameObject)(self.env, ref1, ref2) != 0
        }
    }

    // =========================================================================
    // Reference Management
    // =========================================================================

    /// Creates a new global reference to an object.
    ///
    /// Global references must be explicitly deleted with `delete_global_ref`.
    pub fn new_global_ref(&self, obj: jni::jobject) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).NewGlobalRef)(self.env, obj)
        }
    }

    /// Deletes a global reference.
    pub fn delete_global_ref(&self, obj: jni::jobject) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).DeleteGlobalRef)(self.env, obj);
        }
    }

    /// Creates a new local reference to an object.
    pub fn new_local_ref(&self, obj: jni::jobject) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).NewLocalRef)(self.env, obj)
        }
    }

    /// Deletes a local reference.
    pub fn delete_local_ref(&self, obj: jni::jobject) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).DeleteLocalRef)(self.env, obj);
        }
    }

    /// Creates a new weak global reference.
    pub fn new_weak_global_ref(&self, obj: jni::jobject) -> jni::jweak {
        unsafe {
            let vtable = *self.env;
            ((*vtable).NewWeakGlobalRef)(self.env, obj)
        }
    }

    /// Deletes a weak global reference.
    pub fn delete_weak_global_ref(&self, obj: jni::jweak) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).DeleteWeakGlobalRef)(self.env, obj);
        }
    }

    /// Ensures capacity for the given number of local references.
    pub fn ensure_local_capacity(&self, capacity: jni::jint) -> Result<(), jni::jint> {
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).EnsureLocalCapacity)(self.env, capacity);
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }

    /// Pushes a new local reference frame.
    pub fn push_local_frame(&self, capacity: jni::jint) -> Result<(), jni::jint> {
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).PushLocalFrame)(self.env, capacity);
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }

    /// Pops the current local reference frame, returning a reference in the previous frame.
    pub fn pop_local_frame(&self, result: jni::jobject) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).PopLocalFrame)(self.env, result)
        }
    }

    // =========================================================================
    // Array Operations
    // =========================================================================

    /// Gets the length of an array.
    pub fn get_array_length(&self, array: jni::jarray) -> jni::jsize {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetArrayLength)(self.env, array)
        }
    }

    /// Creates a new object array.
    pub fn new_object_array(&self, length: jni::jsize, cls: jni::jclass, init: jni::jobject) -> Option<jni::jobjectArray> {
        unsafe {
            let vtable = *self.env;
            let arr = ((*vtable).NewObjectArray)(self.env, length, cls, init);
            if arr.is_null() { None } else { Some(arr) }
        }
    }

    /// Gets an element from an object array.
    pub fn get_object_array_element(&self, array: jni::jobjectArray, index: jni::jsize) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetObjectArrayElement)(self.env, array, index)
        }
    }

    /// Sets an element in an object array.
    pub fn set_object_array_element(&self, array: jni::jobjectArray, index: jni::jsize, value: jni::jobject) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetObjectArrayElement)(self.env, array, index, value);
        }
    }

    /// Creates a new byte array.
    pub fn new_byte_array(&self, length: jni::jsize) -> Option<jni::jbyteArray> {
        unsafe {
            let vtable = *self.env;
            let arr = ((*vtable).NewByteArray)(self.env, length);
            if arr.is_null() { None } else { Some(arr) }
        }
    }

    /// Gets a region of a byte array.
    pub fn get_byte_array_region(&self, array: jni::jbyteArray, start: jni::jsize, len: jni::jsize, buf: &mut [jni::jbyte]) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetByteArrayRegion)(self.env, array, start, len, buf.as_mut_ptr());
        }
    }

    /// Sets a region of a byte array.
    pub fn set_byte_array_region(&self, array: jni::jbyteArray, start: jni::jsize, len: jni::jsize, buf: &[jni::jbyte]) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetByteArrayRegion)(self.env, array, start, len, buf.as_ptr());
        }
    }

    /// Creates a new int array.
    pub fn new_int_array(&self, length: jni::jsize) -> Option<jni::jintArray> {
        unsafe {
            let vtable = *self.env;
            let arr = ((*vtable).NewIntArray)(self.env, length);
            if arr.is_null() { None } else { Some(arr) }
        }
    }

    /// Gets a region of an int array.
    pub fn get_int_array_region(&self, array: jni::jintArray, start: jni::jsize, len: jni::jsize, buf: &mut [jni::jint]) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetIntArrayRegion)(self.env, array, start, len, buf.as_mut_ptr());
        }
    }

    /// Sets a region of an int array.
    pub fn set_int_array_region(&self, array: jni::jintArray, start: jni::jsize, len: jni::jsize, buf: &[jni::jint]) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetIntArrayRegion)(self.env, array, start, len, buf.as_ptr());
        }
    }

    /// Creates a new long array.
    pub fn new_long_array(&self, length: jni::jsize) -> Option<jni::jlongArray> {
        unsafe {
            let vtable = *self.env;
            let arr = ((*vtable).NewLongArray)(self.env, length);
            if arr.is_null() { None } else { Some(arr) }
        }
    }

    /// Gets a region of a long array.
    pub fn get_long_array_region(&self, array: jni::jlongArray, start: jni::jsize, len: jni::jsize, buf: &mut [jni::jlong]) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetLongArrayRegion)(self.env, array, start, len, buf.as_mut_ptr());
        }
    }

    /// Sets a region of a long array.
    pub fn set_long_array_region(&self, array: jni::jlongArray, start: jni::jsize, len: jni::jsize, buf: &[jni::jlong]) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetLongArrayRegion)(self.env, array, start, len, buf.as_ptr());
        }
    }

    // =========================================================================
    // Method Calls
    // =========================================================================

    /// Calls a void instance method.
    pub fn call_void_method(&self, obj: jni::jobject, method_id: jni::jmethodID, args: &[jni::jvalue]) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallVoidMethodA)(self.env, obj, method_id, args.as_ptr());
        }
    }

    /// Calls an int instance method.
    pub fn call_int_method(&self, obj: jni::jobject, method_id: jni::jmethodID, args: &[jni::jvalue]) -> jni::jint {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallIntMethodA)(self.env, obj, method_id, args.as_ptr())
        }
    }

    /// Calls a long instance method.
    pub fn call_long_method(&self, obj: jni::jobject, method_id: jni::jmethodID, args: &[jni::jvalue]) -> jni::jlong {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallLongMethodA)(self.env, obj, method_id, args.as_ptr())
        }
    }

    /// Calls a boolean instance method.
    pub fn call_boolean_method(&self, obj: jni::jobject, method_id: jni::jmethodID, args: &[jni::jvalue]) -> bool {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallBooleanMethodA)(self.env, obj, method_id, args.as_ptr()) != 0
        }
    }

    /// Calls an object instance method.
    pub fn call_object_method(&self, obj: jni::jobject, method_id: jni::jmethodID, args: &[jni::jvalue]) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallObjectMethodA)(self.env, obj, method_id, args.as_ptr())
        }
    }

    /// Calls a void static method.
    pub fn call_static_void_method(&self, cls: jni::jclass, method_id: jni::jmethodID, args: &[jni::jvalue]) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallStaticVoidMethodA)(self.env, cls, method_id, args.as_ptr());
        }
    }

    /// Calls an int static method.
    pub fn call_static_int_method(&self, cls: jni::jclass, method_id: jni::jmethodID, args: &[jni::jvalue]) -> jni::jint {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallStaticIntMethodA)(self.env, cls, method_id, args.as_ptr())
        }
    }

    /// Calls an object static method.
    pub fn call_static_object_method(&self, cls: jni::jclass, method_id: jni::jmethodID, args: &[jni::jvalue]) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).CallStaticObjectMethodA)(self.env, cls, method_id, args.as_ptr())
        }
    }

    // =========================================================================
    // Field Access
    // =========================================================================

    /// Gets an object instance field.
    pub fn get_object_field(&self, obj: jni::jobject, field_id: jni::jfieldID) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetObjectField)(self.env, obj, field_id)
        }
    }

    /// Gets an int instance field.
    pub fn get_int_field(&self, obj: jni::jobject, field_id: jni::jfieldID) -> jni::jint {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetIntField)(self.env, obj, field_id)
        }
    }

    /// Gets a long instance field.
    pub fn get_long_field(&self, obj: jni::jobject, field_id: jni::jfieldID) -> jni::jlong {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetLongField)(self.env, obj, field_id)
        }
    }

    /// Sets an object instance field.
    pub fn set_object_field(&self, obj: jni::jobject, field_id: jni::jfieldID, value: jni::jobject) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetObjectField)(self.env, obj, field_id, value);
        }
    }

    /// Sets an int instance field.
    pub fn set_int_field(&self, obj: jni::jobject, field_id: jni::jfieldID, value: jni::jint) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetIntField)(self.env, obj, field_id, value);
        }
    }

    /// Sets a long instance field.
    pub fn set_long_field(&self, obj: jni::jobject, field_id: jni::jfieldID, value: jni::jlong) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetLongField)(self.env, obj, field_id, value);
        }
    }

    /// Gets a static object field.
    pub fn get_static_object_field(&self, cls: jni::jclass, field_id: jni::jfieldID) -> jni::jobject {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetStaticObjectField)(self.env, cls, field_id)
        }
    }

    /// Gets a static int field.
    pub fn get_static_int_field(&self, cls: jni::jclass, field_id: jni::jfieldID) -> jni::jint {
        unsafe {
            let vtable = *self.env;
            ((*vtable).GetStaticIntField)(self.env, cls, field_id)
        }
    }

    /// Sets a static object field.
    pub fn set_static_object_field(&self, cls: jni::jclass, field_id: jni::jfieldID, value: jni::jobject) {
        unsafe {
            let vtable = *self.env;
            ((*vtable).SetStaticObjectField)(self.env, cls, field_id, value);
        }
    }

    // =========================================================================
    // Monitors
    // =========================================================================

    /// Enters the monitor associated with an object.
    pub fn monitor_enter(&self, obj: jni::jobject) -> Result<(), jni::jint> {
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).MonitorEnter)(self.env, obj);
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }

    /// Exits the monitor associated with an object.
    pub fn monitor_exit(&self, obj: jni::jobject) -> Result<(), jni::jint> {
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).MonitorExit)(self.env, obj);
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }

    // =========================================================================
    // Native Method Registration
    // =========================================================================

    /// Registers native methods for a class.
    pub fn register_natives(&self, cls: jni::jclass, methods: &[jni::JNINativeMethod]) -> Result<(), jni::jint> {
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).RegisterNatives)(self.env, cls, methods.as_ptr(), methods.len() as jni::jint);
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }

    /// Unregisters all native methods for a class.
    pub fn unregister_natives(&self, cls: jni::jclass) -> Result<(), jni::jint> {
        unsafe {
            let vtable = *self.env;
            let result = ((*vtable).UnregisterNatives)(self.env, cls);
            if result == 0 { Ok(()) } else { Err(result) }
        }
    }
}

// =========================================================================
// Reference Guards (RAII wrappers)
// =========================================================================

/// A guard that automatically deletes a local reference when dropped.
///
/// # Example
///
/// ```rust,ignore
/// let class = LocalRef::new(&env, env.find_class("java/lang/String").unwrap());
/// // class is automatically deleted when it goes out of scope
/// ```
pub struct LocalRef<'a> {
    env: &'a JniEnv,
    obj: jni::jobject,
}

impl<'a> LocalRef<'a> {
    /// Creates a new LocalRef guard.
    pub fn new(env: &'a JniEnv, obj: jni::jobject) -> Self {
        LocalRef { env, obj }
    }

    /// Returns the underlying jobject.
    pub fn get(&self) -> jni::jobject {
        self.obj
    }

    /// Releases the reference without deleting it.
    pub fn into_inner(self) -> jni::jobject {
        let obj = self.obj;
        std::mem::forget(self);
        obj
    }
}

impl<'a> Drop for LocalRef<'a> {
    fn drop(&mut self) {
        if !self.obj.is_null() {
            self.env.delete_local_ref(self.obj);
        }
    }
}

/// A guard that automatically deletes a global reference when dropped.
///
/// # Example
///
/// ```rust,ignore
/// let global_class = GlobalRef::new(&env, env.find_class("java/lang/String").unwrap());
/// // global_class can be used across JNI calls
/// // it's automatically deleted when dropped
/// ```
pub struct GlobalRef {
    env_for_cleanup: *mut jni::JNIEnv,
    obj: jni::jobject,
}

impl GlobalRef {
    /// Creates a new GlobalRef by creating a global reference from a local reference.
    ///
    /// # Safety
    ///
    /// The caller must ensure the env pointer remains valid for the lifetime of this GlobalRef,
    /// or that cleanup is handled manually.
    pub unsafe fn new(env: &JniEnv, local_obj: jni::jobject) -> Self {
        let global = env.new_global_ref(local_obj);
        GlobalRef {
            env_for_cleanup: env.raw(),
            obj: global,
        }
    }

    /// Returns the underlying global reference.
    pub fn get(&self) -> jni::jobject {
        self.obj
    }
}

impl Drop for GlobalRef {
    fn drop(&mut self) {
        if !self.obj.is_null() && !self.env_for_cleanup.is_null() {
            unsafe {
                let env = JniEnv::from_raw(self.env_for_cleanup);
                env.delete_global_ref(self.obj);
            }
        }
    }
}

// Note: GlobalRef is NOT Send or Sync by default because JNI environments
// are thread-local. If you need to share references across threads, you
// need to obtain a new JNIEnv via AttachCurrentThread.
