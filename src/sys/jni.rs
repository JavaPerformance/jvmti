// jvmti/src/sys/jni.rs
//
// Complete JNI (Java Native Interface) bindings for Rust.
// No external dependencies - suitable for standalone use.
//
// Verified against JDK 27 jni.h header. Compatible with JDK 8+.
//
// The JNI interface has been remarkably stable since JDK 1.6.
// Newer JDKs add functions at the END of the vtable, maintaining
// backwards compatibility:
//   - JDK 9:  GetModule (index 233)
//   - JDK 19: IsVirtualThread (index 234)
//   - JDK 24: GetStringUTFLengthAsLong (index 235)

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

use std::ffi::c_void;
use std::os::raw::c_char;

// Import JVMTI types that are used in JNI function signatures
use crate::sys::jvmti::{jvmtiEnv, jvmtiError};

// =============================================================================
// Primitive Types
// =============================================================================

pub type jint = i32;
pub type jlong = i64;
pub type jbyte = i8;
pub type jboolean = u8;
pub type jchar = u16;
pub type jshort = i16;
pub type jfloat = f32;
pub type jdouble = f64;
pub type jsize = jint;

// =============================================================================
// Reference Types (opaque pointers)
// =============================================================================

pub type jobject = *mut c_void;
pub type jclass = jobject;
pub type jstring = jobject;
pub type jarray = jobject;
pub type jthread = jobject;
pub type jthrowable = jobject;
pub type jweak = jobject;

// Typed arrays (all just aliases to jobject in FFI)
pub type jobjectArray = jarray;
pub type jbooleanArray = jarray;
pub type jbyteArray = jarray;
pub type jcharArray = jarray;
pub type jshortArray = jarray;
pub type jintArray = jarray;
pub type jlongArray = jarray;
pub type jfloatArray = jarray;
pub type jdoubleArray = jarray;

// =============================================================================
// ID Types (opaque identifiers)
// =============================================================================

pub type jmethodID = *mut c_void;
pub type jfieldID = *mut c_void;

// =============================================================================
// jvalue Union
// =============================================================================

#[repr(C)]
#[derive(Copy, Clone)]
pub union jvalue {
    pub z: jboolean,
    pub b: jbyte,
    pub c: jchar,
    pub s: jshort,
    pub i: jint,
    pub j: jlong,
    pub f: jfloat,
    pub d: jdouble,
    pub l: jobject,
}

// =============================================================================
// Constants
// =============================================================================

pub const JNI_OK: jint = 0;
pub const JNI_ERR: jint = -1;
pub const JNI_EDETACHED: jint = -2;
pub const JNI_EVERSION: jint = -3;
pub const JNI_ENOMEM: jint = -4;
pub const JNI_EEXIST: jint = -5;
pub const JNI_EINVAL: jint = -6;

pub const JNI_TRUE: jboolean = 1;
pub const JNI_FALSE: jboolean = 0;

pub const JNI_COMMIT: jint = 1;
pub const JNI_ABORT: jint = 2;

// JNI Version constants
pub const JNI_VERSION_1_1: jint = 0x00010001;
pub const JNI_VERSION_1_2: jint = 0x00010002;
pub const JNI_VERSION_1_4: jint = 0x00010004;
pub const JNI_VERSION_1_6: jint = 0x00010006;
pub const JNI_VERSION_1_8: jint = 0x00010008;
pub const JNI_VERSION_9: jint = 0x00090000;
pub const JNI_VERSION_10: jint = 0x000a0000;
pub const JNI_VERSION_19: jint = 0x00130000;
pub const JNI_VERSION_20: jint = 0x00140000;
pub const JNI_VERSION_21: jint = 0x00150000;
pub const JNI_VERSION_24: jint = 0x00180000;

// =============================================================================
// jobjectRefType enum (JNI 1.6+)
// =============================================================================

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum jobjectRefType {
    JNIInvalidRefType = 0,
    JNILocalRefType = 1,
    JNIGlobalRefType = 2,
    JNIWeakGlobalRefType = 3,
}

// =============================================================================
// JNINativeMethod for RegisterNatives
// =============================================================================

#[repr(C)]
pub struct JNINativeMethod {
    pub name: *const c_char,
    pub signature: *const c_char,
    pub fnPtr: *mut c_void,
}

// =============================================================================
// JVMTI Alloc/Dealloc function types (used in agent code)
// =============================================================================

pub type JvmtiAllocFn = unsafe extern "system" fn(
    env: *mut jvmtiEnv,
    size: jlong,
    mem_ptr: *mut *mut u8,
) -> jvmtiError;

pub type JvmtiDeallocFn = unsafe extern "system" fn(
    env: *mut jvmtiEnv,
    mem: *mut u8,
) -> jvmtiError;

// =============================================================================
// va_list placeholder
// =============================================================================

// va_list is platform-specific and rarely used from Rust.
// We use *mut c_void as a placeholder. In practice, use the "A" variants
// (e.g., CallObjectMethodA) which take jvalue arrays instead.
pub type va_list = *mut c_void;

// =============================================================================
// JNINativeInterface_ - The JNI function table (vtable)
// =============================================================================
//
// This is the heart of JNI. JNIEnv is a pointer to a pointer to this struct.
// 236 function pointers total (4 reserved + 232 functions).
// Order must exactly match the JDK header!

#[repr(C)]
pub struct JNINativeInterface_ {
    // Reserved slots (0-3)
    pub reserved0: *mut c_void,
    pub reserved1: *mut c_void,
    pub reserved2: *mut c_void,
    pub reserved3: *mut c_void,

    // 4: GetVersion
    pub GetVersion: unsafe extern "system" fn(env: *mut JNIEnv) -> jint,

    // 5-6: Class operations
    pub DefineClass: unsafe extern "system" fn(
        env: *mut JNIEnv,
        name: *const c_char,
        loader: jobject,
        buf: *const jbyte,
        len: jsize,
    ) -> jclass,
    pub FindClass: unsafe extern "system" fn(env: *mut JNIEnv, name: *const c_char) -> jclass,

    // 7-9: Reflection
    pub FromReflectedMethod:
        unsafe extern "system" fn(env: *mut JNIEnv, method: jobject) -> jmethodID,
    pub FromReflectedField:
        unsafe extern "system" fn(env: *mut JNIEnv, field: jobject) -> jfieldID,
    pub ToReflectedMethod: unsafe extern "system" fn(
        env: *mut JNIEnv,
        cls: jclass,
        methodID: jmethodID,
        isStatic: jboolean,
    ) -> jobject,

    // 10-11: Class hierarchy
    pub GetSuperclass: unsafe extern "system" fn(env: *mut JNIEnv, sub: jclass) -> jclass,
    pub IsAssignableFrom:
        unsafe extern "system" fn(env: *mut JNIEnv, sub: jclass, sup: jclass) -> jboolean,

    // 12: More reflection
    pub ToReflectedField: unsafe extern "system" fn(
        env: *mut JNIEnv,
        cls: jclass,
        fieldID: jfieldID,
        isStatic: jboolean,
    ) -> jobject,

    // 13-18: Exception handling
    pub Throw: unsafe extern "system" fn(env: *mut JNIEnv, obj: jthrowable) -> jint,
    pub ThrowNew:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, msg: *const c_char) -> jint,
    pub ExceptionOccurred: unsafe extern "system" fn(env: *mut JNIEnv) -> jthrowable,
    pub ExceptionDescribe: unsafe extern "system" fn(env: *mut JNIEnv),
    pub ExceptionClear: unsafe extern "system" fn(env: *mut JNIEnv),
    pub FatalError: unsafe extern "system" fn(env: *mut JNIEnv, msg: *const c_char),

    // 19-20: Local frame
    pub PushLocalFrame: unsafe extern "system" fn(env: *mut JNIEnv, capacity: jint) -> jint,
    pub PopLocalFrame: unsafe extern "system" fn(env: *mut JNIEnv, result: jobject) -> jobject,

    // 21-26: References
    pub NewGlobalRef: unsafe extern "system" fn(env: *mut JNIEnv, lobj: jobject) -> jobject,
    pub DeleteGlobalRef: unsafe extern "system" fn(env: *mut JNIEnv, gref: jobject),
    pub DeleteLocalRef: unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject),
    pub IsSameObject:
        unsafe extern "system" fn(env: *mut JNIEnv, obj1: jobject, obj2: jobject) -> jboolean,
    pub NewLocalRef: unsafe extern "system" fn(env: *mut JNIEnv, ref_: jobject) -> jobject,
    pub EnsureLocalCapacity: unsafe extern "system" fn(env: *mut JNIEnv, capacity: jint) -> jint,

    // 27-30: Object creation
    pub AllocObject: unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass) -> jobject,
    pub NewObject:
        *mut c_void /* variadic - use NewObjectA instead */,
    pub NewObjectV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jobject,
    pub NewObjectA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jobject,

    // 31-32: Object class operations
    pub GetObjectClass: unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject) -> jclass,
    pub IsInstanceOf:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, clazz: jclass) -> jboolean,

    // 33: GetMethodID
    pub GetMethodID: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        name: *const c_char,
        sig: *const c_char,
    ) -> jmethodID,

    // 34-63: Call<Type>Method variants (Object, Boolean, Byte, Char, Short, Int, Long, Float, Double, Void)
    // Each type has 3 variants: varargs, V (va_list), A (jvalue array)
    pub CallObjectMethod:
        *mut c_void /* variadic - use CallObjectMethodA instead */,
    pub CallObjectMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: va_list,
    ) -> jobject,
    pub CallObjectMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jobject,

    pub CallBooleanMethod:
        *mut c_void /* variadic - use CallBooleanMethodA instead */,
    pub CallBooleanMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: va_list,
    ) -> jboolean,
    pub CallBooleanMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jboolean,

    pub CallByteMethod:
        *mut c_void /* variadic - use CallByteMethodA instead */,
    pub CallByteMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: va_list,
    ) -> jbyte,
    pub CallByteMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jbyte,

    pub CallCharMethod:
        *mut c_void /* variadic - use CallCharMethodA instead */,
    pub CallCharMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: va_list,
    ) -> jchar,
    pub CallCharMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jchar,

    pub CallShortMethod:
        *mut c_void /* variadic - use CallShortMethodA instead */,
    pub CallShortMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: va_list,
    ) -> jshort,
    pub CallShortMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jshort,

    pub CallIntMethod:
        *mut c_void /* variadic - use CallIntMethodA instead */,
    pub CallIntMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: va_list,
    ) -> jint,
    pub CallIntMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jint,

    pub CallLongMethod:
        *mut c_void /* variadic - use CallLongMethodA instead */,
    pub CallLongMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: va_list,
    ) -> jlong,
    pub CallLongMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jlong,

    pub CallFloatMethod:
        *mut c_void /* variadic - use CallFloatMethodA instead */,
    pub CallFloatMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: va_list,
    ) -> jfloat,
    pub CallFloatMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jfloat,

    pub CallDoubleMethod:
        *mut c_void /* variadic - use CallDoubleMethodA instead */,
    pub CallDoubleMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: va_list,
    ) -> jdouble,
    pub CallDoubleMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jdouble,

    pub CallVoidMethod:
        *mut c_void /* variadic - use CallVoidMethodA instead */,
    pub CallVoidMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: va_list,
    ),
    pub CallVoidMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        methodID: jmethodID,
        args: *const jvalue,
    ),

    // 64-93: CallNonvirtual<Type>Method variants
    pub CallNonvirtualObjectMethod: *mut c_void, /* variadic - use CallNonvirtualObjectMethodA */
    pub CallNonvirtualObjectMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jobject,
    pub CallNonvirtualObjectMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jobject,

    pub CallNonvirtualBooleanMethod: *mut c_void, /* variadic - use CallNonvirtualBooleanMethodA */
    pub CallNonvirtualBooleanMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jboolean,
    pub CallNonvirtualBooleanMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jboolean,

    pub CallNonvirtualByteMethod: *mut c_void, /* variadic - use CallNonvirtualByteMethodA */
    pub CallNonvirtualByteMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jbyte,
    pub CallNonvirtualByteMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jbyte,

    pub CallNonvirtualCharMethod: *mut c_void, /* variadic - use CallNonvirtualCharMethodA */
    pub CallNonvirtualCharMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jchar,
    pub CallNonvirtualCharMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jchar,

    pub CallNonvirtualShortMethod: *mut c_void, /* variadic - use CallNonvirtualShortMethodA */
    pub CallNonvirtualShortMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jshort,
    pub CallNonvirtualShortMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jshort,

    pub CallNonvirtualIntMethod: *mut c_void, /* variadic - use CallNonvirtualIntMethodA */
    pub CallNonvirtualIntMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jint,
    pub CallNonvirtualIntMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jint,

    pub CallNonvirtualLongMethod: *mut c_void, /* variadic - use CallNonvirtualLongMethodA */
    pub CallNonvirtualLongMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jlong,
    pub CallNonvirtualLongMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jlong,

    pub CallNonvirtualFloatMethod: *mut c_void, /* variadic - use CallNonvirtualFloatMethodA */
    pub CallNonvirtualFloatMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jfloat,
    pub CallNonvirtualFloatMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jfloat,

    pub CallNonvirtualDoubleMethod: *mut c_void, /* variadic - use CallNonvirtualDoubleMethodA */
    pub CallNonvirtualDoubleMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jdouble,
    pub CallNonvirtualDoubleMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jdouble,

    pub CallNonvirtualVoidMethod: *mut c_void, /* variadic - use CallNonvirtualVoidMethodA */
    pub CallNonvirtualVoidMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ),
    pub CallNonvirtualVoidMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        obj: jobject,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ),

    // 94: GetFieldID
    pub GetFieldID: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        name: *const c_char,
        sig: *const c_char,
    ) -> jfieldID,

    // 95-103: Get<Type>Field
    pub GetObjectField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID) -> jobject,
    pub GetBooleanField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID) -> jboolean,
    pub GetByteField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID) -> jbyte,
    pub GetCharField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID) -> jchar,
    pub GetShortField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID) -> jshort,
    pub GetIntField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID) -> jint,
    pub GetLongField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID) -> jlong,
    pub GetFloatField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID) -> jfloat,
    pub GetDoubleField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID) -> jdouble,

    // 104-112: Set<Type>Field
    pub SetObjectField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID, val: jobject),
    pub SetBooleanField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID, val: jboolean),
    pub SetByteField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID, val: jbyte),
    pub SetCharField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID, val: jchar),
    pub SetShortField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID, val: jshort),
    pub SetIntField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID, val: jint),
    pub SetLongField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID, val: jlong),
    pub SetFloatField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID, val: jfloat),
    pub SetDoubleField:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject, fieldID: jfieldID, val: jdouble),

    // 113: GetStaticMethodID
    pub GetStaticMethodID: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        name: *const c_char,
        sig: *const c_char,
    ) -> jmethodID,

    // 114-143: CallStatic<Type>Method variants
    pub CallStaticObjectMethod:
        *mut c_void /* variadic - use NewObjectA instead */,
    pub CallStaticObjectMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jobject,
    pub CallStaticObjectMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jobject,

    pub CallStaticBooleanMethod:
        *mut c_void /* variadic - use CallStaticBooleanMethodA */,
    pub CallStaticBooleanMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jboolean,
    pub CallStaticBooleanMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jboolean,

    pub CallStaticByteMethod:
        *mut c_void /* variadic - use CallStaticByteMethodA */,
    pub CallStaticByteMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jbyte,
    pub CallStaticByteMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jbyte,

    pub CallStaticCharMethod:
        *mut c_void /* variadic - use CallStaticCharMethodA */,
    pub CallStaticCharMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jchar,
    pub CallStaticCharMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jchar,

    pub CallStaticShortMethod:
        *mut c_void /* variadic - use CallStaticShortMethodA */,
    pub CallStaticShortMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jshort,
    pub CallStaticShortMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jshort,

    pub CallStaticIntMethod:
        *mut c_void /* variadic - use CallStaticIntMethodA */,
    pub CallStaticIntMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jint,
    pub CallStaticIntMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jint,

    pub CallStaticLongMethod:
        *mut c_void /* variadic - use CallStaticLongMethodA */,
    pub CallStaticLongMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jlong,
    pub CallStaticLongMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jlong,

    pub CallStaticFloatMethod:
        *mut c_void /* variadic - use CallStaticFloatMethodA */,
    pub CallStaticFloatMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jfloat,
    pub CallStaticFloatMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jfloat,

    pub CallStaticDoubleMethod:
        *mut c_void /* variadic - use CallStaticDoubleMethodA */,
    pub CallStaticDoubleMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: va_list,
    ) -> jdouble,
    pub CallStaticDoubleMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ) -> jdouble,

    pub CallStaticVoidMethod:
        *mut c_void /* variadic - use CallStaticVoidMethodA */,
    pub CallStaticVoidMethodV: unsafe extern "system" fn(
        env: *mut JNIEnv,
        cls: jclass,
        methodID: jmethodID,
        args: va_list,
    ),
    pub CallStaticVoidMethodA: unsafe extern "system" fn(
        env: *mut JNIEnv,
        cls: jclass,
        methodID: jmethodID,
        args: *const jvalue,
    ),

    // 144: GetStaticFieldID
    pub GetStaticFieldID: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        name: *const c_char,
        sig: *const c_char,
    ) -> jfieldID,

    // 145-153: GetStatic<Type>Field
    pub GetStaticObjectField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID) -> jobject,
    pub GetStaticBooleanField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID) -> jboolean,
    pub GetStaticByteField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID) -> jbyte,
    pub GetStaticCharField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID) -> jchar,
    pub GetStaticShortField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID) -> jshort,
    pub GetStaticIntField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID) -> jint,
    pub GetStaticLongField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID) -> jlong,
    pub GetStaticFloatField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID) -> jfloat,
    pub GetStaticDoubleField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID) -> jdouble,

    // 154-162: SetStatic<Type>Field
    pub SetStaticObjectField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID, value: jobject),
    pub SetStaticBooleanField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID, value: jboolean),
    pub SetStaticByteField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID, value: jbyte),
    pub SetStaticCharField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID, value: jchar),
    pub SetStaticShortField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID, value: jshort),
    pub SetStaticIntField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID, value: jint),
    pub SetStaticLongField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID, value: jlong),
    pub SetStaticFloatField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID, value: jfloat),
    pub SetStaticDoubleField:
        unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass, fieldID: jfieldID, value: jdouble),

    // 163-166: String operations
    pub NewString:
        unsafe extern "system" fn(env: *mut JNIEnv, unicode: *const jchar, len: jsize) -> jstring,
    pub GetStringLength: unsafe extern "system" fn(env: *mut JNIEnv, str: jstring) -> jsize,
    pub GetStringChars: unsafe extern "system" fn(
        env: *mut JNIEnv,
        str: jstring,
        isCopy: *mut jboolean,
    ) -> *const jchar,
    pub ReleaseStringChars:
        unsafe extern "system" fn(env: *mut JNIEnv, str: jstring, chars: *const jchar),

    // 167-170: UTF String operations
    pub NewStringUTF: unsafe extern "system" fn(env: *mut JNIEnv, utf: *const c_char) -> jstring,
    pub GetStringUTFLength: unsafe extern "system" fn(env: *mut JNIEnv, str: jstring) -> jsize,
    pub GetStringUTFChars: unsafe extern "system" fn(
        env: *mut JNIEnv,
        str: jstring,
        isCopy: *mut jboolean,
    ) -> *const c_char,
    pub ReleaseStringUTFChars:
        unsafe extern "system" fn(env: *mut JNIEnv, str: jstring, chars: *const c_char),

    // 171: GetArrayLength
    pub GetArrayLength: unsafe extern "system" fn(env: *mut JNIEnv, array: jarray) -> jsize,

    // 172-174: Object array operations
    pub NewObjectArray: unsafe extern "system" fn(
        env: *mut JNIEnv,
        len: jsize,
        clazz: jclass,
        init: jobject,
    ) -> jobjectArray,
    pub GetObjectArrayElement:
        unsafe extern "system" fn(env: *mut JNIEnv, array: jobjectArray, index: jsize) -> jobject,
    pub SetObjectArrayElement: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jobjectArray,
        index: jsize,
        val: jobject,
    ),

    // 175-182: New<Type>Array
    pub NewBooleanArray:
        unsafe extern "system" fn(env: *mut JNIEnv, len: jsize) -> jbooleanArray,
    pub NewByteArray: unsafe extern "system" fn(env: *mut JNIEnv, len: jsize) -> jbyteArray,
    pub NewCharArray: unsafe extern "system" fn(env: *mut JNIEnv, len: jsize) -> jcharArray,
    pub NewShortArray: unsafe extern "system" fn(env: *mut JNIEnv, len: jsize) -> jshortArray,
    pub NewIntArray: unsafe extern "system" fn(env: *mut JNIEnv, len: jsize) -> jintArray,
    pub NewLongArray: unsafe extern "system" fn(env: *mut JNIEnv, len: jsize) -> jlongArray,
    pub NewFloatArray: unsafe extern "system" fn(env: *mut JNIEnv, len: jsize) -> jfloatArray,
    pub NewDoubleArray: unsafe extern "system" fn(env: *mut JNIEnv, len: jsize) -> jdoubleArray,

    // 183-190: Get<Type>ArrayElements
    pub GetBooleanArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jbooleanArray,
        isCopy: *mut jboolean,
    ) -> *mut jboolean,
    pub GetByteArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jbyteArray,
        isCopy: *mut jboolean,
    ) -> *mut jbyte,
    pub GetCharArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jcharArray,
        isCopy: *mut jboolean,
    ) -> *mut jchar,
    pub GetShortArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jshortArray,
        isCopy: *mut jboolean,
    ) -> *mut jshort,
    pub GetIntArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jintArray,
        isCopy: *mut jboolean,
    ) -> *mut jint,
    pub GetLongArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jlongArray,
        isCopy: *mut jboolean,
    ) -> *mut jlong,
    pub GetFloatArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jfloatArray,
        isCopy: *mut jboolean,
    ) -> *mut jfloat,
    pub GetDoubleArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jdoubleArray,
        isCopy: *mut jboolean,
    ) -> *mut jdouble,

    // 191-198: Release<Type>ArrayElements
    pub ReleaseBooleanArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jbooleanArray,
        elems: *mut jboolean,
        mode: jint,
    ),
    pub ReleaseByteArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jbyteArray,
        elems: *mut jbyte,
        mode: jint,
    ),
    pub ReleaseCharArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jcharArray,
        elems: *mut jchar,
        mode: jint,
    ),
    pub ReleaseShortArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jshortArray,
        elems: *mut jshort,
        mode: jint,
    ),
    pub ReleaseIntArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jintArray,
        elems: *mut jint,
        mode: jint,
    ),
    pub ReleaseLongArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jlongArray,
        elems: *mut jlong,
        mode: jint,
    ),
    pub ReleaseFloatArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jfloatArray,
        elems: *mut jfloat,
        mode: jint,
    ),
    pub ReleaseDoubleArrayElements: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jdoubleArray,
        elems: *mut jdouble,
        mode: jint,
    ),

    // 199-206: Get<Type>ArrayRegion
    pub GetBooleanArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jbooleanArray,
        start: jsize,
        len: jsize,
        buf: *mut jboolean,
    ),
    pub GetByteArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jbyteArray,
        start: jsize,
        len: jsize,
        buf: *mut jbyte,
    ),
    pub GetCharArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jcharArray,
        start: jsize,
        len: jsize,
        buf: *mut jchar,
    ),
    pub GetShortArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jshortArray,
        start: jsize,
        len: jsize,
        buf: *mut jshort,
    ),
    pub GetIntArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jintArray,
        start: jsize,
        len: jsize,
        buf: *mut jint,
    ),
    pub GetLongArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jlongArray,
        start: jsize,
        len: jsize,
        buf: *mut jlong,
    ),
    pub GetFloatArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jfloatArray,
        start: jsize,
        len: jsize,
        buf: *mut jfloat,
    ),
    pub GetDoubleArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jdoubleArray,
        start: jsize,
        len: jsize,
        buf: *mut jdouble,
    ),

    // 207-214: Set<Type>ArrayRegion
    pub SetBooleanArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jbooleanArray,
        start: jsize,
        len: jsize,
        buf: *const jboolean,
    ),
    pub SetByteArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jbyteArray,
        start: jsize,
        len: jsize,
        buf: *const jbyte,
    ),
    pub SetCharArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jcharArray,
        start: jsize,
        len: jsize,
        buf: *const jchar,
    ),
    pub SetShortArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jshortArray,
        start: jsize,
        len: jsize,
        buf: *const jshort,
    ),
    pub SetIntArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jintArray,
        start: jsize,
        len: jsize,
        buf: *const jint,
    ),
    pub SetLongArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jlongArray,
        start: jsize,
        len: jsize,
        buf: *const jlong,
    ),
    pub SetFloatArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jfloatArray,
        start: jsize,
        len: jsize,
        buf: *const jfloat,
    ),
    pub SetDoubleArrayRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jdoubleArray,
        start: jsize,
        len: jsize,
        buf: *const jdouble,
    ),

    // 215-216: Native method registration
    pub RegisterNatives: unsafe extern "system" fn(
        env: *mut JNIEnv,
        clazz: jclass,
        methods: *const JNINativeMethod,
        nMethods: jint,
    ) -> jint,
    pub UnregisterNatives: unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass) -> jint,

    // 217-218: Monitor operations
    pub MonitorEnter: unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject) -> jint,
    pub MonitorExit: unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject) -> jint,

    // 219: GetJavaVM
    pub GetJavaVM: unsafe extern "system" fn(env: *mut JNIEnv, vm: *mut *mut JavaVM) -> jint,

    // 220-221: String region operations (JNI 1.2)
    pub GetStringRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        str: jstring,
        start: jsize,
        len: jsize,
        buf: *mut jchar,
    ),
    pub GetStringUTFRegion: unsafe extern "system" fn(
        env: *mut JNIEnv,
        str: jstring,
        start: jsize,
        len: jsize,
        buf: *mut c_char,
    ),

    // 222-223: Critical array access (JNI 1.2)
    pub GetPrimitiveArrayCritical: unsafe extern "system" fn(
        env: *mut JNIEnv,
        array: jarray,
        isCopy: *mut jboolean,
    ) -> *mut c_void,
    pub ReleasePrimitiveArrayCritical:
        unsafe extern "system" fn(env: *mut JNIEnv, array: jarray, carray: *mut c_void, mode: jint),

    // 224-225: Critical string access (JNI 1.2)
    pub GetStringCritical: unsafe extern "system" fn(
        env: *mut JNIEnv,
        string: jstring,
        isCopy: *mut jboolean,
    ) -> *const jchar,
    pub ReleaseStringCritical:
        unsafe extern "system" fn(env: *mut JNIEnv, string: jstring, cstring: *const jchar),

    // 226-227: Weak global references (JNI 1.2)
    pub NewWeakGlobalRef: unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject) -> jweak,
    pub DeleteWeakGlobalRef: unsafe extern "system" fn(env: *mut JNIEnv, ref_: jweak),

    // 228: Exception check (JNI 1.2)
    pub ExceptionCheck: unsafe extern "system" fn(env: *mut JNIEnv) -> jboolean,

    // 229-231: Direct buffer support (JNI 1.4)
    pub NewDirectByteBuffer:
        unsafe extern "system" fn(env: *mut JNIEnv, address: *mut c_void, capacity: jlong) -> jobject,
    pub GetDirectBufferAddress:
        unsafe extern "system" fn(env: *mut JNIEnv, buf: jobject) -> *mut c_void,
    pub GetDirectBufferCapacity: unsafe extern "system" fn(env: *mut JNIEnv, buf: jobject) -> jlong,

    // 232: Object reference type (JNI 1.6)
    pub GetObjectRefType:
        unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject) -> jobjectRefType,

    // 233: Module support (JNI 9)
    pub GetModule: unsafe extern "system" fn(env: *mut JNIEnv, clazz: jclass) -> jobject,

    // 234: Virtual thread support (JNI 19/21)
    pub IsVirtualThread: unsafe extern "system" fn(env: *mut JNIEnv, obj: jobject) -> jboolean,

    // 235: String UTF length as long (JNI 24/25)
    pub GetStringUTFLengthAsLong: unsafe extern "system" fn(env: *mut JNIEnv, str: jstring) -> jlong,
}

// =============================================================================
// JNIEnv - Pointer to the JNI function table
// =============================================================================
//
// IMPORTANT: In C JNI, JNIEnv is directly a pointer to the vtable:
//   typedef const struct JNINativeInterface_ *JNIEnv;
//
// The JNIEnv_ wrapper struct only exists in C++ for convenience methods.
// Since Rust uses C ABI (extern "system"), we use the C definition.
// =============================================================================

/// JNIEnv is directly the vtable pointer (C ABI definition)
pub type JNIEnv = *const JNINativeInterface_;

// =============================================================================
// JNIInvokeInterface_ - The JavaVM function table
// =============================================================================

#[repr(C)]
pub struct JNIInvokeInterface_ {
    pub reserved0: *mut c_void,
    pub reserved1: *mut c_void,
    pub reserved2: *mut c_void,

    pub DestroyJavaVM: unsafe extern "system" fn(vm: *mut JavaVM) -> jint,
    pub AttachCurrentThread:
        unsafe extern "system" fn(vm: *mut JavaVM, penv: *mut *mut c_void, args: *mut c_void) -> jint,
    pub DetachCurrentThread: unsafe extern "system" fn(vm: *mut JavaVM) -> jint,
    pub GetEnv:
        unsafe extern "system" fn(vm: *mut JavaVM, penv: *mut *mut c_void, version: jint) -> jint,
    pub AttachCurrentThreadAsDaemon:
        unsafe extern "system" fn(vm: *mut JavaVM, penv: *mut *mut c_void, args: *mut c_void) -> jint,
}

// =============================================================================
// JavaVM - Pointer to the JavaVM function table
// =============================================================================
//
// IMPORTANT: In C JNI, JavaVM is directly a pointer to the vtable:
//   typedef const struct JNIInvokeInterface_ *JavaVM;
//
// The JavaVM_ wrapper struct only exists in C++ for convenience methods.
// Since Rust uses C ABI (extern "system"), we use the C definition.
// =============================================================================

/// JavaVM is directly the vtable pointer (C ABI definition)
pub type JavaVM = *const JNIInvokeInterface_;

// =============================================================================
// JavaVMInitArgs and JavaVMOption for JNI_CreateJavaVM
// =============================================================================

#[repr(C)]
pub struct JavaVMOption {
    pub optionString: *mut c_char,
    pub extraInfo: *mut c_void,
}

#[repr(C)]
pub struct JavaVMInitArgs {
    pub version: jint,
    pub nOptions: jint,
    pub options: *mut JavaVMOption,
    pub ignoreUnrecognized: jboolean,
}

#[repr(C)]
pub struct JavaVMAttachArgs {
    pub version: jint,
    pub name: *mut c_char,
    pub group: jobject,
}

// =============================================================================
// Helper macros and functions
// =============================================================================

/// Helper to call JNI functions through the vtable.
/// env_ptr: *mut JNIEnv = *mut *const JNINativeInterface_
/// *env_ptr: *const JNINativeInterface_ (vtable pointer)
/// **env_ptr: JNINativeInterface_ (vtable itself)
/// Usage: jni_call!(env, FindClass, b"java/lang/String\0".as_ptr() as *const c_char)
#[macro_export]
macro_rules! jni_call {
    ($env:expr, $func:ident $(, $args:expr)*) => {{
        let env_ptr = $env;
        ((**env_ptr).$func)(env_ptr $(, $args)*)
    }};
}

/// Helper to call JavaVM functions through the vtable.
/// vm_ptr: *mut JavaVM = *mut *const JNIInvokeInterface_
/// *vm_ptr: *const JNIInvokeInterface_ (vtable pointer)
/// **vm_ptr: JNIInvokeInterface_ (vtable itself)
#[macro_export]
macro_rules! jvm_call {
    ($vm:expr, $func:ident $(, $args:expr)*) => {{
        let vm_ptr = $vm;
        ((**vm_ptr).$func)(vm_ptr $(, $args)*)
    }};
}
