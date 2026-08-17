//! Heap graph utilities (feature-gated).
//!
//! This module provides simple helpers for tagging objects and extracting
//! reference edges using JVMTI heap callbacks. It is intentionally conservative
//! and designed for tooling, not production hot paths.

use crate::env::Jvmti;
use crate::sys::{jni, jvmti};
use std::os::raw::c_void;
use std::ptr;

#[derive(Debug, Clone)]
pub struct HeapGraph {
    pub edges: Vec<(jni::jlong, jni::jlong)>,
}

#[derive(Debug, Clone)]
pub struct TagRange {
    pub start: jni::jlong,
    pub end: jni::jlong,
    pub tagged: jni::jlong,
}

struct Tagger {
    next: jni::jlong,
    tagged: jni::jlong,
}

unsafe extern "system" fn tag_all_objects_cb(
    _class_tag: jni::jlong,
    _size: jni::jlong,
    tag_ptr: *mut jni::jlong,
    user_data: *mut c_void,
) -> jni::jint {
    if tag_ptr.is_null() || user_data.is_null() {
        return jvmti::JVMTI_ITERATION_CONTINUE;
    }
    // SAFETY: JVM TI supplies both pointers for the duration of this
    // synchronous callback, and the caller passes `user_data` as a `Tagger`.
    let tagger = unsafe { &mut *user_data.cast::<Tagger>() };
    // SAFETY: The null check above and JVM TI callback contract make `tag_ptr`
    // readable and writable for this invocation.
    if unsafe { *tag_ptr } == 0 {
        unsafe { *tag_ptr = tagger.next };
        tagger.next += 1;
        tagger.tagged += 1;
    }
    jvmti::JVMTI_ITERATION_CONTINUE
}

/// Tags all objects in the heap with a unique tag (if currently 0).
///
/// This is expensive and should be used for offline analysis, not in hot paths.
pub fn tag_all_objects(
    jvmti_env: &Jvmti,
    start_tag: jni::jlong,
) -> Result<TagRange, jvmti::jvmtiError> {
    let mut tagger = Tagger {
        next: start_tag,
        tagged: 0,
    };
    let user_data = &mut tagger as *mut Tagger as *mut c_void;
    // The callback and user-data pointer remain valid for this synchronous call.
    #[allow(deprecated)]
    unsafe {
        jvmti_env.iterate_over_heap(
            jvmti::JVMTI_HEAP_OBJECT_EITHER,
            tag_all_objects_cb,
            user_data,
        )?
    };
    Ok(TagRange {
        start: start_tag,
        end: tagger.next,
        tagged: tagger.tagged,
    })
}

struct EdgeCollector {
    edges: Vec<(jni::jlong, jni::jlong)>,
}

unsafe extern "system" fn edge_collector_cb(
    _reference_kind: jni::jint,
    _reference_info: *const jvmti::jvmtiHeapReferenceInfo,
    _class_tag: jni::jlong,
    _referrer_class_tag: jni::jlong,
    _size: jni::jlong,
    tag_ptr: *mut jni::jlong,
    referrer_tag_ptr: *mut jni::jlong,
    _length: jni::jint,
    user_data: *mut c_void,
) -> jni::jint {
    if user_data.is_null() || tag_ptr.is_null() || referrer_tag_ptr.is_null() {
        return jvmti::JVMTI_ITERATION_CONTINUE;
    }
    let target_tag = unsafe { *tag_ptr };
    let referrer_tag = unsafe { *referrer_tag_ptr };
    if referrer_tag != 0 && target_tag != 0 {
        let collector = unsafe { &mut *(user_data as *mut EdgeCollector) };
        collector.edges.push((referrer_tag, target_tag));
    }
    jvmti::JVMTI_ITERATION_CONTINUE
}

/// Builds a heap reference edge list using `FollowReferences`.
///
/// Note: this only records edges for objects with non-zero tags.
/// Call [`tag_all_objects`] first if you want full coverage.
/// # Safety
///
/// `initial_object` must be null or a live reference belonging to the JVM TI
/// environment for the full synchronous traversal.
pub unsafe fn build_heap_graph(
    jvmti_env: &Jvmti,
    heap_filter: jni::jint,
    initial_object: jni::jobject,
) -> Result<HeapGraph, jvmti::jvmtiError> {
    let mut collector = EdgeCollector { edges: Vec::new() };
    let callbacks = jvmti::jvmtiHeapCallbacks {
        heap_reference_callback: Some(edge_collector_cb),
        ..Default::default()
    };

    // SAFETY: Forwarded from this function's contract. The callbacks and
    // collector remain alive for the complete synchronous traversal.
    unsafe {
        jvmti_env.follow_references(
            heap_filter,
            ptr::null_mut(),
            initial_object,
            &callbacks,
            &mut collector as *mut EdgeCollector as *const c_void,
        )?;
    }

    Ok(HeapGraph {
        edges: collector.edges,
    })
}
