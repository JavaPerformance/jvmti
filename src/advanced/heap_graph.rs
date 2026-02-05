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
    let tagger = &mut *(user_data as *mut Tagger);
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
pub fn tag_all_objects(jvmti_env: &Jvmti, start_tag: jni::jlong) -> Result<TagRange, jvmti::jvmtiError> {
    let mut tagger = Tagger { next: start_tag, tagged: 0 };
    let user_data = &mut tagger as *mut Tagger as *mut c_void;
    jvmti_env.iterate_over_heap(jvmti::JVMTI_HEAP_OBJECT_EITHER, tag_all_objects_cb, user_data)?;
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
    _reference_info: jvmti::jvmtiObjectReferenceInfo,
    _class_tag: jni::jlong,
    referrer_tag: jni::jlong,
    target_tag: jni::jlong,
    _reference_index: jni::jint,
    user_data: *mut c_void,
    _index_ptr: *mut jni::jint,
) -> jni::jint {
    if user_data.is_null() {
        return jvmti::JVMTI_ITERATION_CONTINUE;
    }
    if referrer_tag != 0 && target_tag != 0 {
        let collector = &mut *(user_data as *mut EdgeCollector);
        collector.edges.push((referrer_tag, target_tag));
    }
    jvmti::JVMTI_ITERATION_CONTINUE
}

/// Builds a heap reference edge list using `FollowReferences`.
///
/// Note: this only records edges for objects with non-zero tags.
/// Call [`tag_all_objects`] first if you want full coverage.
pub fn build_heap_graph(
    jvmti_env: &Jvmti,
    heap_filter: jni::jint,
    initial_object: jni::jobject,
) -> Result<HeapGraph, jvmti::jvmtiError> {
    let mut collector = EdgeCollector { edges: Vec::new() };
    let callbacks = jvmti::jvmtiHeapCallbacks {
        heap_root_callback: None,
        stack_reference_callback: None,
        object_reference_callback: Some(edge_collector_cb),
        object_callback: None,
    };

    jvmti_env.follow_references(
        heap_filter,
        ptr::null_mut(),
        initial_object,
        &callbacks,
        &mut collector as *mut EdgeCollector as *const c_void,
    )?;

    Ok(HeapGraph { edges: collector.edges })
}
