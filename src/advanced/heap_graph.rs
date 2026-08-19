//! Heap graph utilities (feature-gated).
//!
//! This module provides simple helpers for tagging objects and extracting
//! reference edges using JVMTI heap callbacks. It is intentionally conservative
//! and designed for tooling, not production hot paths.

use crate::env::Jvmti;
use crate::sys::{jni, jvmti};
use std::os::raw::c_void;
use std::ptr;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct HeapGraph {
    pub edges: Vec<(jni::jlong, jni::jlong)>,
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TagRange {
    pub start: jni::jlong,
    pub end: jni::jlong,
    pub tagged: jni::jlong,
}

struct Tagger {
    next: jni::jlong,
    tagged: jni::jlong,
    failure: Option<HeapCallbackFailure>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum HeapCallbackFailure {
    TagRangeExhausted,
    EdgeCapacityExhausted,
}

impl HeapCallbackFailure {
    const fn as_jvmti_error(self) -> jvmti::jvmtiError {
        match self {
            Self::TagRangeExhausted => jvmti::JVMTI_ERROR_ILLEGAL_ARGUMENT,
            Self::EdgeCapacityExhausted => jvmti::JVMTI_ERROR_OUT_OF_MEMORY,
        }
    }
}

fn next_nonzero_tag(tag: jni::jlong) -> Option<jni::jlong> {
    let next = tag.checked_add(1)?;
    if next == 0 {
        next.checked_add(1)
    } else {
        Some(next)
    }
}

unsafe extern "system" fn tag_all_objects_cb(
    _class_tag: jni::jlong,
    _size: jni::jlong,
    tag_ptr: *mut jni::jlong,
    user_data: *mut c_void,
) -> jvmti::jvmtiIterationControl {
    if tag_ptr.is_null() || user_data.is_null() {
        return jvmti::JVMTI_ITERATION_CONTINUE;
    }
    // SAFETY: JVM TI supplies both pointers for the duration of this
    // synchronous callback, and the caller passes `user_data` as a `Tagger`.
    let tagger = unsafe { &mut *user_data.cast::<Tagger>() };
    // SAFETY: The null check above and JVM TI callback contract make `tag_ptr`
    // readable and writable for this invocation.
    if unsafe { *tag_ptr } == 0 {
        let Some(next) = next_nonzero_tag(tagger.next) else {
            tagger.failure = Some(HeapCallbackFailure::TagRangeExhausted);
            return jvmti::JVMTI_ITERATION_ABORT;
        };
        let Some(tagged) = tagger.tagged.checked_add(1) else {
            tagger.failure = Some(HeapCallbackFailure::TagRangeExhausted);
            return jvmti::JVMTI_ITERATION_ABORT;
        };
        unsafe { *tag_ptr = tagger.next };
        tagger.next = next;
        tagger.tagged = tagged;
    }
    jvmti::JVMTI_ITERATION_CONTINUE
}

/// Tags all objects in the heap with a unique tag (if currently 0).
///
/// `start_tag` must be non-zero. Tag assignment increases monotonically and
/// skips zero when a negative range crosses into positive values. If the range
/// or tagged-object count cannot advance without overflow, traversal aborts and
/// returns [`jvmti::JVMTI_ERROR_ILLEGAL_ARGUMENT`]. Objects tagged before that
/// abort remain tagged, as required by the synchronous JVM TI traversal model.
///
/// This is expensive and should be used for offline analysis, not in hot paths.
pub fn tag_all_objects(
    jvmti_env: &Jvmti,
    start_tag: jni::jlong,
) -> Result<TagRange, jvmti::jvmtiError> {
    if start_tag == 0 {
        return Err(jvmti::JVMTI_ERROR_ILLEGAL_ARGUMENT);
    }
    let mut tagger = Tagger {
        next: start_tag,
        tagged: 0,
        failure: None,
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
    if let Some(failure) = tagger.failure {
        return Err(failure.as_jvmti_error());
    }
    Ok(TagRange {
        start: start_tag,
        end: tagger.next,
        tagged: tagger.tagged,
    })
}

struct EdgeCollector {
    edges: Vec<(jni::jlong, jni::jlong)>,
    failure: Option<HeapCallbackFailure>,
}

unsafe extern "system" fn edge_collector_cb(
    _reference_kind: jvmti::jvmtiHeapReferenceKind,
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
        return jvmti::JVMTI_VISIT_OBJECTS;
    }
    let target_tag = unsafe { *tag_ptr };
    let referrer_tag = unsafe { *referrer_tag_ptr };
    if referrer_tag != 0 && target_tag != 0 {
        let collector = unsafe { &mut *(user_data as *mut EdgeCollector) };
        if collector.edges.try_reserve(1).is_err() {
            collector.failure = Some(HeapCallbackFailure::EdgeCapacityExhausted);
            return jvmti::JVMTI_VISIT_ABORT;
        }
        collector.edges.push((referrer_tag, target_tag));
    }
    jvmti::JVMTI_VISIT_OBJECTS
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
    let mut collector = EdgeCollector {
        edges: Vec::new(),
        failure: None,
    };
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
    if let Some(failure) = collector.failure {
        return Err(failure.as_jvmti_error());
    }

    Ok(HeapGraph {
        edges: collector.edges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_reference_callback_requests_continued_object_visitation() {
        let mut collector = EdgeCollector {
            edges: Vec::new(),
            failure: None,
        };
        let mut target = 22;
        let mut referrer = 11;
        let result = unsafe {
            edge_collector_cb(
                jvmti::JVMTI_HEAP_REFERENCE_FIELD,
                ptr::null(),
                0,
                0,
                0,
                &mut target,
                &mut referrer,
                0,
                (&mut collector as *mut EdgeCollector).cast(),
            )
        };
        assert_eq!(result, jvmti::JVMTI_VISIT_OBJECTS);
        assert_eq!(collector.edges, [(11, 22)]);
    }

    #[test]
    fn deprecated_iterator_keeps_its_distinct_continue_contract() {
        let mut tag = 0;
        let mut tagger = Tagger {
            next: 7,
            tagged: 0,
            failure: None,
        };
        let result =
            unsafe { tag_all_objects_cb(0, 0, &mut tag, (&mut tagger as *mut Tagger).cast()) };
        assert_eq!(result, jvmti::JVMTI_ITERATION_CONTINUE);
        assert_eq!(tag, 7);
        assert_eq!(tagger.tagged, 1);
    }

    #[test]
    fn tag_progression_skips_zero() {
        let mut tagger = Tagger {
            next: -1,
            tagged: 0,
            failure: None,
        };
        let mut first = 0;
        let mut second = 0;
        let first_result =
            unsafe { tag_all_objects_cb(0, 0, &mut first, (&mut tagger as *mut Tagger).cast()) };
        let second_result =
            unsafe { tag_all_objects_cb(0, 0, &mut second, (&mut tagger as *mut Tagger).cast()) };
        assert_eq!(first_result, jvmti::JVMTI_ITERATION_CONTINUE);
        assert_eq!(second_result, jvmti::JVMTI_ITERATION_CONTINUE);
        assert_eq!((first, second), (-1, 1));
        assert_eq!(tagger.next, 2);
        assert_eq!(tagger.tagged, 2);
        assert_eq!(tagger.failure, None);
    }

    #[test]
    fn exhausted_tag_range_aborts_without_mutating_the_object() {
        let mut tagger = Tagger {
            next: jni::jlong::MAX,
            tagged: 0,
            failure: None,
        };
        let mut tag = 0;
        let result =
            unsafe { tag_all_objects_cb(0, 0, &mut tag, (&mut tagger as *mut Tagger).cast()) };
        assert_eq!(result, jvmti::JVMTI_ITERATION_ABORT);
        assert_eq!(tag, 0);
        assert_eq!(tagger.tagged, 0);
        assert_eq!(tagger.failure, Some(HeapCallbackFailure::TagRangeExhausted));
    }

    unsafe extern "system" fn iterate_one_untagged_object(
        _env: *mut jvmti::jvmtiEnv,
        _filter: jvmti::jvmtiHeapObjectFilter,
        callback: Option<jvmti::jvmtiHeapObjectCallback>,
        user_data: *const c_void,
    ) -> jvmti::jvmtiError {
        let mut tag = 0;
        let callback = callback.expect("heap callback should be installed");
        let result = unsafe { callback(0, 0, &mut tag, user_data.cast_mut()) };
        assert_eq!(result, jvmti::JVMTI_ITERATION_ABORT);
        jvmti::JVMTI_ERROR_NONE
    }

    #[test]
    fn public_helper_propagates_callback_range_exhaustion() {
        let table = jvmti::jvmtiInterface_1_ {
            IterateOverHeap: Some(iterate_one_untagged_object),
            ..Default::default()
        };
        let mut raw_env = jvmti::jvmtiEnv { functions: &table };
        let env = unsafe { Jvmti::from_raw(&mut raw_env) };
        assert_eq!(
            tag_all_objects(&env, jni::jlong::MAX).unwrap_err(),
            jvmti::JVMTI_ERROR_ILLEGAL_ARGUMENT
        );
    }
}
