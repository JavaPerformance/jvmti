// vliss/jvmti/src/wrapper.rs
use crate::sys::jvmti;
use crate::sys::jni;
use std::ffi::{CStr, CString};
use std::ptr;

#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub name: Option<String>,
    pub priority: jni::jint,
    pub is_daemon: bool,
    pub thread_group: jni::jobject,
    pub context_class_loader: jni::jobject,
}

#[derive(Debug, Clone)]
pub struct ThreadGroupInfo {
    pub parent: jni::jobject,
    pub name: Option<String>,
    pub max_priority: jni::jint,
    pub is_daemon: bool,
}

#[derive(Debug, Clone)]
pub struct MonitorUsage {
    pub owner: jni::jthread,
    pub entry_count: jni::jint,
    pub waiters: Vec<jni::jthread>,
    pub notify_waiters: Vec<jni::jthread>,
}

#[derive(Debug, Clone)]
pub struct StackInfo {
    pub thread: jni::jthread,
    pub state: jni::jint,
    pub frames: Vec<jvmti::jvmtiFrameInfo>,
}

#[derive(Debug, Clone)]
pub struct ExtensionParamInfo {
    pub name: Option<String>,
    pub kind: jni::jint,
    pub base_type: jni::jint,
    pub null_ok: bool,
}

#[derive(Debug, Clone)]
pub struct ExtensionFunctionInfo {
    pub func: *mut std::ffi::c_void,
    pub id: Option<String>,
    pub short_description: Option<String>,
    pub params: Vec<ExtensionParamInfo>,
    pub errors: Vec<jvmti::jvmtiError>,
}

#[derive(Debug, Clone)]
pub struct ExtensionEventInfo {
    pub extension_event_index: jni::jint,
    pub id: Option<String>,
    pub short_description: Option<String>,
    pub params: Vec<ExtensionParamInfo>,
}

#[derive(Debug, Clone)]
pub struct LocalVariableEntry {
    pub start_location: jvmti::jlocation,
    pub length: jni::jint,
    pub name: Option<String>,
    pub signature: Option<String>,
    pub generic_signature: Option<String>,
    pub slot: jni::jint,
}

fn ptr_in_range(ptr: *const u8, base: *const u8, len: usize) -> bool {
    if ptr.is_null() || base.is_null() || len == 0 {
        return false;
    }
    let p = ptr as usize;
    let b = base as usize;
    p >= b && p < b + len
}

fn cstr_to_string(ptr: *const std::os::raw::c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string()) }
}

/// A safe wrapper around the raw JVMTI Environment pointer.
pub struct Jvmti {
    // We keep this private so the user can't mess with raw pointers directly.
    env: *mut jvmti::jvmtiEnv,
}

impl Jvmti {
    /// Connects to the JVM and retrieves the JVMTI environment.
    pub fn new(vm: *mut jni::JavaVM) -> Result<Self, jni::jint> {
        let mut env_ptr: *mut std::ffi::c_void = ptr::null_mut();

        unsafe {
            // Access GetEnv directly from the vtable
            // vm: *mut JavaVM = *mut *const JNIInvokeInterface_
            // *vm: *const JNIInvokeInterface_ (vtable pointer)
            // **vm: JNIInvokeInterface_ (vtable itself)
            let get_env_fn = (**vm).GetEnv;

            let res = get_env_fn(vm, &mut env_ptr, jvmti::JVMTI_VERSION_1_2);

            if res != jni::JNI_OK {
                return Err(res);
            }
        }

        Ok(Jvmti {
            env: env_ptr as *mut jvmti::jvmtiEnv,
        })
    }

    /// Create a Jvmti wrapper from a raw jvmtiEnv pointer
    ///
    /// # Safety
    /// The caller must ensure the pointer is valid for the duration of use.
    pub unsafe fn from_raw(env: *mut jvmti::jvmtiEnv) -> Self {
        Jvmti { env }
    }

    /// Get the raw jvmtiEnv pointer
    pub fn raw(&self) -> *mut jvmti::jvmtiEnv {
        self.env
    }

    pub fn get_capabilities(&self) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError> {
        let mut caps = jvmti::jvmtiCapabilities::default();

        unsafe {
            let get_caps_fn = (*(*self.env).functions).GetCapabilities.unwrap();
            let err = get_caps_fn(self.env, &mut caps);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        Ok(caps)
    }

    pub fn add_capabilities(&self, new_caps: &jvmti::jvmtiCapabilities) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            // 1. Retrieve the function pointer from the VTable (Index 142)
            // If this panics, it means AddCapabilities is null (unlikely on a valid JVM)
            // or jvmti.rs has the wrong type definition (missing Option).
            let add_caps_fn = (*(*self.env).functions).AddCapabilities.unwrap();

            // 2. Call the C function
            let err = add_caps_fn(self.env, new_caps);

            // 3. Check for success
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Convenience helper to build and add capabilities in one step.
    pub fn add_capabilities_with<F>(&self, f: F) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError>
    where
        F: FnOnce(&mut jvmti::jvmtiCapabilities),
    {
        let mut caps = jvmti::jvmtiCapabilities::default();
        f(&mut caps);
        self.add_capabilities(&caps)?;
        Ok(caps)
    }
    
    pub fn set_event_callbacks(&self, callbacks: jvmti::jvmtiEventCallbacks) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_callbacks_fn = (*(*self.env).functions).SetEventCallbacks.unwrap();
            let size = std::mem::size_of::<jvmti::jvmtiEventCallbacks>() as i32;

            let err = set_callbacks_fn(self.env, &callbacks, size);
            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }


    pub fn set_event_notification_mode(&self, enable: bool, event_type: u32, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_mode_fn = (*(*self.env).functions).SetEventNotificationMode.unwrap(); // Index 1
            let mode = if enable { 1 } else { 0 }; // JVMTI_ENABLE = 1, DISABLE = 0

            // thread can be null (all threads)
            let err = set_mode_fn(self.env, mode, event_type, thread);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Enable a single JVMTI event for a specific thread (or all threads with null).
    pub fn enable_event(&self, event_type: u32, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        self.set_event_notification_mode(true, event_type, thread)
    }

    /// Disable a single JVMTI event for a specific thread (or all threads with null).
    pub fn disable_event(&self, event_type: u32, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        self.set_event_notification_mode(false, event_type, thread)
    }

    /// Enable multiple JVMTI events for all threads.
    pub fn enable_events_global(&self, events: &[u32]) -> Result<(), jvmti::jvmtiError> {
        for &event_type in events {
            self.enable_event(event_type, ptr::null_mut())?;
        }
        Ok(())
    }

    /// Disable multiple JVMTI events for all threads.
    pub fn disable_events_global(&self, events: &[u32]) -> Result<(), jvmti::jvmtiError> {
        for &event_type in events {
            self.disable_event(event_type, ptr::null_mut())?;
        }
        Ok(())
    }

    pub fn get_all_modules(&self) -> Result<Vec<jni::jobject>, jvmti::jvmtiError> {
        let mut module_count: jni::jint = 0;
        let mut modules_ptr: *mut jni::jobject = ptr::null_mut();

        unsafe {
            let get_all_modules_fn = (*(*self.env).functions).GetAllModules.unwrap();
            let err = get_all_modules_fn(self.env, &mut module_count, &mut modules_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }

            let modules = std::slice::from_raw_parts(modules_ptr, module_count as usize).to_vec();
            self.deallocate(modules_ptr as *mut u8)?;

            Ok(modules)
        }
    }

    pub fn get_all_threads(&self) -> Result<Vec<jni::jthread>, jvmti::jvmtiError> {
        let mut threads_count: jni::jint = 0;
        let mut threads_ptr: *mut jni::jthread = ptr::null_mut();

        unsafe {
            let get_all_threads_fn = (*(*self.env).functions).GetAllThreads.unwrap();
            let err = get_all_threads_fn(self.env, &mut threads_count, &mut threads_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }

            let threads = std::slice::from_raw_parts(threads_ptr, threads_count as usize).to_vec();
            self.deallocate(threads_ptr as *mut u8)?;

            Ok(threads)
        }
    }

    pub fn get_thread_info(&self, thread: jni::jthread) -> Result<ThreadInfo, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiThreadInfo::default();

        unsafe {
            let get_thread_info_fn = (*(*self.env).functions).GetThreadInfo.unwrap();
            let err = get_thread_info_fn(self.env, thread, &mut info);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        let name = cstr_to_string(info.name);
        if !info.name.is_null() {
            self.deallocate(info.name as *mut u8)?;
        }

        Ok(ThreadInfo {
            name,
            priority: info.priority,
            is_daemon: info.is_daemon != 0,
            thread_group: info.thread_group,
            context_class_loader: info.context_class_loader,
        })
    }

    pub fn allocate(&self, size: jni::jlong) -> Result<*mut u8, jvmti::jvmtiError> {
        let mut mem_ptr: *mut u8 = ptr::null_mut();

        unsafe {
            let allocate_fn = (*(*self.env).functions).Allocate.unwrap();
            let err = allocate_fn(self.env, size, &mut mem_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        Ok(mem_ptr)
    }

    pub fn deallocate(&self, mem: *mut u8) -> Result<(), jvmti::jvmtiError> {
        if mem.is_null() {
            return Ok(());
        }
        unsafe {
            let deallocate_fn = (*(*self.env).functions).Deallocate.unwrap();
            let err = deallocate_fn(self.env, mem);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn get_thread_state(&self, thread: jni::jthread) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut thread_state: jni::jint = 0;

        unsafe {
            let get_thread_state_fn = (*(*self.env).functions).GetThreadState.unwrap();
            let err = get_thread_state_fn(self.env, thread, &mut thread_state);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        Ok(thread_state)
    }

    pub fn get_current_thread(&self) -> Result<jni::jthread, jvmti::jvmtiError> {
        let mut thread: jni::jthread = ptr::null_mut();

        unsafe {
            let get_current_thread_fn = (*(*self.env).functions).GetCurrentThread.unwrap();
            let err = get_current_thread_fn(self.env, &mut thread);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        Ok(thread)
    }

    pub fn get_class_signature(&self, klass: jni::jclass) -> Result<(String, Option<String>), jvmti::jvmtiError> {
        let mut sig_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        let mut gen_ptr: *mut std::os::raw::c_char = ptr::null_mut();

        unsafe {
            let get_class_sig_fn = (*(*self.env).functions).GetClassSignature.unwrap();
            let err = get_class_sig_fn(self.env, klass, &mut sig_ptr, &mut gen_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }

            let signature = std::ffi::CStr::from_ptr(sig_ptr).to_string_lossy().into_owned();
            let generic = if !gen_ptr.is_null() {
                Some(std::ffi::CStr::from_ptr(gen_ptr).to_string_lossy().into_owned())
            } else {
                None
            };

            self.deallocate(sig_ptr as *mut u8)?;
            if !gen_ptr.is_null() {
                self.deallocate(gen_ptr as *mut u8)?;
            }

            Ok((signature, generic))
        }
    }

    pub fn get_method_name(&self, method: jni::jmethodID) -> Result<(String, String, Option<String>), jvmti::jvmtiError> {
        let mut name_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        let mut sig_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        let mut gen_ptr: *mut std::os::raw::c_char = ptr::null_mut();

        unsafe {
            let get_method_name_fn = (*(*self.env).functions).GetMethodName.unwrap();
            let err = get_method_name_fn(self.env, method, &mut name_ptr, &mut sig_ptr, &mut gen_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }

            let name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().into_owned();
            let signature = std::ffi::CStr::from_ptr(sig_ptr).to_string_lossy().into_owned();
            let generic = if !gen_ptr.is_null() {
                Some(std::ffi::CStr::from_ptr(gen_ptr).to_string_lossy().into_owned())
            } else {
                None
            };

            self.deallocate(name_ptr as *mut u8)?;
            self.deallocate(sig_ptr as *mut u8)?;
            if !gen_ptr.is_null() {
                self.deallocate(gen_ptr as *mut u8)?;
            }

            Ok((name, signature, generic))
        }
    }

    pub fn get_potential_capabilities(&self) -> Result<jvmti::jvmtiCapabilities, jvmti::jvmtiError> {
        let mut caps = jvmti::jvmtiCapabilities::default();

        unsafe {
            let get_pot_caps_fn = (*(*self.env).functions).GetPotentialCapabilities.unwrap();
            let err = get_pot_caps_fn(self.env, &mut caps);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }

        Ok(caps)
    }

    pub fn dispose_environment(&self) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let dispose_env_fn = (*(*self.env).functions).DisposeEnvironment.unwrap();
            let err = dispose_env_fn(self.env);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn get_loaded_classes(&self) -> Result<Vec<jni::jclass>, jvmti::jvmtiError> {
        let mut class_count: jni::jint = 0;
        let mut classes_ptr: *mut jni::jclass = ptr::null_mut();

        unsafe {
            let get_loaded_classes_fn = (*(*self.env).functions).GetLoadedClasses.unwrap();
            let err = get_loaded_classes_fn(self.env, &mut class_count, &mut classes_ptr);

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }

            let classes = std::slice::from_raw_parts(classes_ptr, class_count as usize).to_vec();
            self.deallocate(classes_ptr as *mut u8)?;

            Ok(classes)
        }
    }

    pub fn redefine_classes(&self, class_definitions: &[jvmti::jvmtiClassDefinition]) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let redefine_classes_fn = (*(*self.env).functions).RedefineClasses.unwrap();
            let err = redefine_classes_fn(self.env, class_definitions.len() as jni::jint, class_definitions.as_ptr());

            if err != jvmti::jvmtiError::NONE {
                return Err(err);
            }
        }
        Ok(())
    }

    pub fn suspend_thread(&self, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let suspend_fn = (*(*self.env).functions).SuspendThread.unwrap();
            let err = suspend_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn resume_thread(&self, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let resume_fn = (*(*self.env).functions).ResumeThread.unwrap();
            let err = resume_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn stop_thread(&self, thread: jni::jthread, exception: jni::jobject) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let stop_fn = (*(*self.env).functions).StopThread.unwrap();
            let err = stop_fn(self.env, thread, exception);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn interrupt_thread(&self, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let interrupt_fn = (*(*self.env).functions).InterruptThread.unwrap();
            let err = interrupt_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn run_agent_thread(&self, thread: jni::jthread, proc: jvmti::jvmtiStartFunction, arg: *const std::os::raw::c_void, priority: jni::jint) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let run_fn = (*(*self.env).functions).RunAgentThread.unwrap();
            let err = run_fn(self.env, thread, proc, arg, priority);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn suspend_thread_list(&self, request_list: &[jni::jthread]) -> Result<Vec<jvmti::jvmtiError>, jvmti::jvmtiError> {
        let mut results = vec![jvmti::jvmtiError::NONE; request_list.len()];
        unsafe {
            let suspend_list_fn = (*(*self.env).functions).SuspendThreadList.unwrap();
            let err = suspend_list_fn(self.env, request_list.len() as jni::jint, request_list.as_ptr(), results.as_mut_ptr());
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(results)
    }

    pub fn resume_thread_list(&self, request_list: &[jni::jthread]) -> Result<Vec<jvmti::jvmtiError>, jvmti::jvmtiError> {
        let mut results = vec![jvmti::jvmtiError::NONE; request_list.len()];
        unsafe {
            let resume_list_fn = (*(*self.env).functions).ResumeThreadList.unwrap();
            let err = resume_list_fn(self.env, request_list.len() as jni::jint, request_list.as_ptr(), results.as_mut_ptr());
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(results)
    }

    pub fn get_top_thread_groups(&self) -> Result<Vec<jni::jobject>, jvmti::jvmtiError> {
        let mut group_count: jni::jint = 0;
        let mut groups_ptr: *mut jni::jobject = ptr::null_mut();
        unsafe {
            let get_groups_fn = (*(*self.env).functions).GetTopThreadGroups.unwrap();
            let err = get_groups_fn(self.env, &mut group_count, &mut groups_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let groups = std::slice::from_raw_parts(groups_ptr, group_count as usize).to_vec();
            self.deallocate(groups_ptr as *mut u8)?;
            Ok(groups)
        }
    }

    pub fn get_thread_group_info(&self, group: jni::jobject) -> Result<ThreadGroupInfo, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiThreadGroupInfo::default();
        unsafe {
            let get_info_fn = (*(*self.env).functions).GetThreadGroupInfo.unwrap();
            let err = get_info_fn(self.env, group, &mut info);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        let name = cstr_to_string(info.name);
        if !info.name.is_null() {
            self.deallocate(info.name as *mut u8)?;
        }
        Ok(ThreadGroupInfo {
            parent: info.parent,
            name,
            max_priority: info.max_priority,
            is_daemon: info.is_daemon != 0,
        })
    }

    pub fn get_thread_group_children(&self, group: jni::jobject) -> Result<(Vec<jni::jthread>, Vec<jni::jobject>), jvmti::jvmtiError> {
        let mut thread_count: jni::jint = 0;
        let mut threads_ptr: *mut jni::jthread = ptr::null_mut();
        let mut group_count: jni::jint = 0;
        let mut groups_ptr: *mut jni::jobject = ptr::null_mut();
        unsafe {
            let get_children_fn = (*(*self.env).functions).GetThreadGroupChildren.unwrap();
            let err = get_children_fn(self.env, group, &mut thread_count, &mut threads_ptr, &mut group_count, &mut groups_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let threads = std::slice::from_raw_parts(threads_ptr, thread_count as usize).to_vec();
            let groups = std::slice::from_raw_parts(groups_ptr, group_count as usize).to_vec();
            self.deallocate(threads_ptr as *mut u8)?;
            self.deallocate(groups_ptr as *mut u8)?;
            Ok((threads, groups))
        }
    }

    pub fn get_owned_monitor_info(&self, thread: jni::jthread) -> Result<Vec<jni::jobject>, jvmti::jvmtiError> {
        let mut monitor_count: jni::jint = 0;
        let mut monitors_ptr: *mut jni::jobject = ptr::null_mut();
        unsafe {
            let get_monitors_fn = (*(*self.env).functions).GetOwnedMonitorInfo.unwrap();
            let err = get_monitors_fn(self.env, thread, &mut monitor_count, &mut monitors_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let monitors = std::slice::from_raw_parts(monitors_ptr, monitor_count as usize).to_vec();
            self.deallocate(monitors_ptr as *mut u8)?;
            Ok(monitors)
        }
    }

    pub fn get_current_contended_monitor(&self, thread: jni::jthread) -> Result<jni::jobject, jvmti::jvmtiError> {
        let mut monitor: jni::jobject = ptr::null_mut();
        unsafe {
            let get_monitor_fn = (*(*self.env).functions).GetCurrentContendedMonitor.unwrap();
            let err = get_monitor_fn(self.env, thread, &mut monitor);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(monitor)
        }
    }

    pub fn create_raw_monitor(&self, name: &str) -> Result<jvmti::jrawMonitorID, jvmti::jvmtiError> {
        let c_name = CString::new(name).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)?;
        let mut monitor: jvmti::jrawMonitorID = ptr::null_mut();
        unsafe {
            let create_fn = (*(*self.env).functions).CreateRawMonitor.unwrap();
            let err = create_fn(self.env, c_name.as_ptr(), &mut monitor);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(monitor)
        }
    }

    pub fn destroy_raw_monitor(&self, monitor: jvmti::jrawMonitorID) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let destroy_fn = (*(*self.env).functions).DestroyRawMonitor.unwrap();
            let err = destroy_fn(self.env, monitor);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn raw_monitor_enter(&self, monitor: jvmti::jrawMonitorID) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let enter_fn = (*(*self.env).functions).RawMonitorEnter.unwrap();
            let err = enter_fn(self.env, monitor);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn raw_monitor_exit(&self, monitor: jvmti::jrawMonitorID) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let exit_fn = (*(*self.env).functions).RawMonitorExit.unwrap();
            let err = exit_fn(self.env, monitor);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn raw_monitor_wait(&self, monitor: jvmti::jrawMonitorID, millis: jni::jlong) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let wait_fn = (*(*self.env).functions).RawMonitorWait.unwrap();
            let err = wait_fn(self.env, monitor, millis);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn raw_monitor_notify(&self, monitor: jvmti::jrawMonitorID) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let notify_fn = (*(*self.env).functions).RawMonitorNotify.unwrap();
            let err = notify_fn(self.env, monitor);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn raw_monitor_notify_all(&self, monitor: jvmti::jrawMonitorID) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let notify_all_fn = (*(*self.env).functions).RawMonitorNotifyAll.unwrap();
            let err = notify_all_fn(self.env, monitor);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_frame_count(&self, thread: jni::jthread) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        unsafe {
            let get_count_fn = (*(*self.env).functions).GetFrameCount.unwrap();
            let err = get_count_fn(self.env, thread, &mut count);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(count)
        }
    }

    pub fn get_frame_location(&self, thread: jni::jthread, depth: jni::jint) -> Result<(jni::jmethodID, jvmti::jlocation), jvmti::jvmtiError> {
        let mut method: jni::jmethodID = ptr::null_mut();
        let mut location: jvmti::jlocation = 0;
        unsafe {
            let get_loc_fn = (*(*self.env).functions).GetFrameLocation.unwrap();
            let err = get_loc_fn(self.env, thread, depth, &mut method, &mut location);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok((method, location))
        }
    }

    pub fn notify_frame_pop(&self, thread: jni::jthread, depth: jni::jint) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let notify_fn = (*(*self.env).functions).NotifyFramePop.unwrap();
            let err = notify_fn(self.env, thread, depth);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_local_object(&self, thread: jni::jthread, depth: jni::jint, slot: jni::jint) -> Result<jni::jobject, jvmti::jvmtiError> {
        let mut value: jni::jobject = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetLocalObject.unwrap();
            let err = get_fn(self.env, thread, depth, slot, &mut value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(value)
        }
    }

    pub fn get_local_int(&self, thread: jni::jthread, depth: jni::jint, slot: jni::jint) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut value: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetLocalInt.unwrap();
            let err = get_fn(self.env, thread, depth, slot, &mut value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(value)
        }
    }

    pub fn get_local_long(&self, thread: jni::jthread, depth: jni::jint, slot: jni::jint) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut value: jni::jlong = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetLocalLong.unwrap();
            let err = get_fn(self.env, thread, depth, slot, &mut value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(value)
        }
    }

    pub fn get_local_float(&self, thread: jni::jthread, depth: jni::jint, slot: jni::jint) -> Result<jni::jfloat, jvmti::jvmtiError> {
        let mut value: jni::jfloat = 0.0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetLocalFloat.unwrap();
            let err = get_fn(self.env, thread, depth, slot, &mut value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(value)
        }
    }

    pub fn get_local_double(&self, thread: jni::jthread, depth: jni::jint, slot: jni::jint) -> Result<jni::jdouble, jvmti::jvmtiError> {
        let mut value: jni::jdouble = 0.0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetLocalDouble.unwrap();
            let err = get_fn(self.env, thread, depth, slot, &mut value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(value)
        }
    }

    pub fn set_local_object(&self, thread: jni::jthread, depth: jni::jint, slot: jni::jint, value: jni::jobject) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetLocalObject.unwrap();
            let err = set_fn(self.env, thread, depth, slot, value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn set_local_int(&self, thread: jni::jthread, depth: jni::jint, slot: jni::jint, value: jni::jint) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetLocalInt.unwrap();
            let err = set_fn(self.env, thread, depth, slot, value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn set_local_long(&self, thread: jni::jthread, depth: jni::jint, slot: jni::jint, value: jni::jlong) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetLocalLong.unwrap();
            let err = set_fn(self.env, thread, depth, slot, value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn set_local_float(&self, thread: jni::jthread, depth: jni::jint, slot: jni::jint, value: jni::jfloat) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetLocalFloat.unwrap();
            let err = set_fn(self.env, thread, depth, slot, value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn set_local_double(&self, thread: jni::jthread, depth: jni::jint, slot: jni::jint, value: jni::jdouble) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetLocalDouble.unwrap();
            let err = set_fn(self.env, thread, depth, slot, value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_local_instance(&self, thread: jni::jthread, depth: jni::jint) -> Result<jni::jobject, jvmti::jvmtiError> {
        let mut value: jni::jobject = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetLocalInstance.unwrap();
            let err = get_fn(self.env, thread, depth, &mut value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(value)
        }
    }

    pub fn pop_frame(&self, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let pop_fn = (*(*self.env).functions).PopFrame.unwrap();
            let err = pop_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn force_early_return_object(&self, thread: jni::jthread, value: jni::jobject) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = (*(*self.env).functions).ForceEarlyReturnObject.unwrap();
            let err = force_fn(self.env, thread, value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn force_early_return_int(&self, thread: jni::jthread, value: jni::jint) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = (*(*self.env).functions).ForceEarlyReturnInt.unwrap();
            let err = force_fn(self.env, thread, value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn force_early_return_long(&self, thread: jni::jthread, value: jni::jlong) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = (*(*self.env).functions).ForceEarlyReturnLong.unwrap();
            let err = force_fn(self.env, thread, value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn force_early_return_float(&self, thread: jni::jthread, value: jni::jfloat) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = (*(*self.env).functions).ForceEarlyReturnFloat.unwrap();
            let err = force_fn(self.env, thread, value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn force_early_return_double(&self, thread: jni::jthread, value: jni::jdouble) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = (*(*self.env).functions).ForceEarlyReturnDouble.unwrap();
            let err = force_fn(self.env, thread, value);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn force_early_return_void(&self, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = (*(*self.env).functions).ForceEarlyReturnVoid.unwrap();
            let err = force_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_stack_trace(&self, thread: jni::jthread, start_depth: jni::jint, max_frame_count: jni::jint) -> Result<Vec<jvmti::jvmtiFrameInfo>, jvmti::jvmtiError> {
        let mut frame_buffer = vec![jvmti::jvmtiFrameInfo::default(); max_frame_count as usize];
        let mut count: jni::jint = 0;
        unsafe {
            let get_stack_fn = (*(*self.env).functions).GetStackTrace.unwrap();
            let err = get_stack_fn(self.env, thread, start_depth, max_frame_count, frame_buffer.as_mut_ptr(), &mut count);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            frame_buffer.truncate(count as usize);
            Ok(frame_buffer)
        }
    }

    pub fn get_all_stack_traces(&self, max_frame_count: jni::jint) -> Result<Vec<StackInfo>, jvmti::jvmtiError> {
        let mut stack_info_ptr: *mut jvmti::jvmtiStackInfo = ptr::null_mut();
        let mut thread_count: jni::jint = 0;
        unsafe {
            let get_all_fn = (*(*self.env).functions).GetAllStackTraces.unwrap();
            let err = get_all_fn(self.env, max_frame_count, &mut stack_info_ptr, &mut thread_count);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        let info_slice = unsafe { std::slice::from_raw_parts(stack_info_ptr, thread_count as usize) };
        let base = stack_info_ptr as *const u8;
        let len = (thread_count as usize) * std::mem::size_of::<jvmti::jvmtiStackInfo>();

        let mut out = Vec::with_capacity(thread_count as usize);
        for info in info_slice {
            let frames = if info.frame_count > 0 && !info.frame_buffer.is_null() {
                unsafe { std::slice::from_raw_parts(info.frame_buffer, info.frame_count as usize).to_vec() }
            } else {
                Vec::new()
            };
            out.push(StackInfo {
                thread: info.thread,
                state: info.state,
                frames,
            });

            if !info.frame_buffer.is_null() && !ptr_in_range(info.frame_buffer as *const u8, base, len) {
                self.deallocate(info.frame_buffer as *mut u8)?;
            }
        }

        if !stack_info_ptr.is_null() {
            self.deallocate(stack_info_ptr as *mut u8)?;
        }
        Ok(out)
    }

    pub fn get_thread_list_stack_traces(&self, thread_list: &[jni::jthread], max_frame_count: jni::jint) -> Result<Vec<StackInfo>, jvmti::jvmtiError> {
        let mut stack_info_ptr: *mut jvmti::jvmtiStackInfo = ptr::null_mut();
        unsafe {
            let get_list_fn = (*(*self.env).functions).GetThreadListStackTraces.unwrap();
            let err = get_list_fn(self.env, thread_list.len() as jni::jint, thread_list.as_ptr(), max_frame_count, &mut stack_info_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        let info_slice = unsafe { std::slice::from_raw_parts(stack_info_ptr, thread_list.len()) };
        let base = stack_info_ptr as *const u8;
        let len = thread_list.len() * std::mem::size_of::<jvmti::jvmtiStackInfo>();

        let mut out = Vec::with_capacity(thread_list.len());
        for info in info_slice {
            let frames = if info.frame_count > 0 && !info.frame_buffer.is_null() {
                unsafe { std::slice::from_raw_parts(info.frame_buffer, info.frame_count as usize).to_vec() }
            } else {
                Vec::new()
            };
            out.push(StackInfo {
                thread: info.thread,
                state: info.state,
                frames,
            });

            if !info.frame_buffer.is_null() && !ptr_in_range(info.frame_buffer as *const u8, base, len) {
                self.deallocate(info.frame_buffer as *mut u8)?;
            }
        }

        if !stack_info_ptr.is_null() {
            self.deallocate(stack_info_ptr as *mut u8)?;
        }
        Ok(out)
    }

    pub fn get_named_module(&self, class_loader: jni::jobject, package_name: &str) -> Result<jni::jobject, jvmti::jvmtiError> {
        let c_package = CString::new(package_name).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)?;
        let mut module: jni::jobject = ptr::null_mut();
        unsafe {
            let get_module_fn = (*(*self.env).functions).GetNamedModule.unwrap();
            let err = get_module_fn(self.env, class_loader, c_package.as_ptr(), &mut module);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(module)
        }
    }

    pub fn get_class_status(&self, klass: jni::jclass) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut status: jni::jint = 0;
        unsafe {
            let get_status_fn = (*(*self.env).functions).GetClassStatus.unwrap();
            let err = get_status_fn(self.env, klass, &mut status);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(status)
        }
    }

    pub fn get_source_file_name(&self, klass: jni::jclass) -> Result<String, jvmti::jvmtiError> {
        let mut name_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetSourceFileName.unwrap();
            let err = get_fn(self.env, klass, &mut name_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().into_owned();
            self.deallocate(name_ptr as *mut u8)?;
            Ok(name)
        }
    }

    pub fn get_class_modifiers(&self, klass: jni::jclass) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut modifiers: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetClassModifiers.unwrap();
            let err = get_fn(self.env, klass, &mut modifiers);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(modifiers)
        }
    }

    pub fn get_class_methods(&self, klass: jni::jclass) -> Result<Vec<jni::jmethodID>, jvmti::jvmtiError> {
        let mut method_count: jni::jint = 0;
        let mut methods_ptr: *mut jni::jmethodID = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetClassMethods.unwrap();
            let err = get_fn(self.env, klass, &mut method_count, &mut methods_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let methods = std::slice::from_raw_parts(methods_ptr, method_count as usize).to_vec();
            self.deallocate(methods_ptr as *mut u8)?;
            Ok(methods)
        }
    }

    pub fn get_class_fields(&self, klass: jni::jclass) -> Result<Vec<jni::jfieldID>, jvmti::jvmtiError> {
        let mut field_count: jni::jint = 0;
        let mut fields_ptr: *mut jni::jfieldID = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetClassFields.unwrap();
            let err = get_fn(self.env, klass, &mut field_count, &mut fields_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let fields = std::slice::from_raw_parts(fields_ptr, field_count as usize).to_vec();
            self.deallocate(fields_ptr as *mut u8)?;
            Ok(fields)
        }
    }

    pub fn get_implemented_interfaces(&self, klass: jni::jclass) -> Result<Vec<jni::jclass>, jvmti::jvmtiError> {
        let mut interface_count: jni::jint = 0;
        let mut interfaces_ptr: *mut jni::jclass = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetImplementedInterfaces.unwrap();
            let err = get_fn(self.env, klass, &mut interface_count, &mut interfaces_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let interfaces = std::slice::from_raw_parts(interfaces_ptr, interface_count as usize).to_vec();
            self.deallocate(interfaces_ptr as *mut u8)?;
            Ok(interfaces)
        }
    }

    pub fn is_interface(&self, klass: jni::jclass) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).IsInterface.unwrap();
            let err = get_fn(self.env, klass, &mut res);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(res != 0)
        }
    }

    pub fn is_array_class(&self, klass: jni::jclass) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).IsArrayClass.unwrap();
            let err = get_fn(self.env, klass, &mut res);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(res != 0)
        }
    }

    pub fn get_class_loader(&self, klass: jni::jclass) -> Result<jni::jobject, jvmti::jvmtiError> {
        let mut loader: jni::jobject = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetClassLoader.unwrap();
            let err = get_fn(self.env, klass, &mut loader);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(loader)
        }
    }

    pub fn get_field_name(&self, klass: jni::jclass, field: jni::jfieldID) -> Result<(String, String, Option<String>), jvmti::jvmtiError> {
        let mut name_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        let mut sig_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        let mut gen_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetFieldName.unwrap();
            let err = get_fn(self.env, klass, field, &mut name_ptr, &mut sig_ptr, &mut gen_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().into_owned();
            let sig = std::ffi::CStr::from_ptr(sig_ptr).to_string_lossy().into_owned();
            let gen = if gen_ptr.is_null() { None } else { Some(std::ffi::CStr::from_ptr(gen_ptr).to_string_lossy().into_owned()) };
            self.deallocate(name_ptr as *mut u8)?;
            self.deallocate(sig_ptr as *mut u8)?;
            if !gen_ptr.is_null() { self.deallocate(gen_ptr as *mut u8)?; }
            Ok((name, sig, gen))
        }
    }

    pub fn get_field_declaring_class(&self, klass: jni::jclass, field: jni::jfieldID) -> Result<jni::jclass, jvmti::jvmtiError> {
        let mut declaring_class: jni::jclass = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetFieldDeclaringClass.unwrap();
            let err = get_fn(self.env, klass, field, &mut declaring_class);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(declaring_class)
        }
    }

    pub fn get_field_modifiers(&self, klass: jni::jclass, field: jni::jfieldID) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut modifiers: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetFieldModifiers.unwrap();
            let err = get_fn(self.env, klass, field, &mut modifiers);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(modifiers)
        }
    }

    pub fn is_field_synthetic(&self, klass: jni::jclass, field: jni::jfieldID) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).IsFieldSynthetic.unwrap();
            let err = get_fn(self.env, klass, field, &mut res);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(res != 0)
        }
    }

    pub fn get_method_declaring_class(&self, method: jni::jmethodID) -> Result<jni::jclass, jvmti::jvmtiError> {
        let mut declaring_class: jni::jclass = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetMethodDeclaringClass.unwrap();
            let err = get_fn(self.env, method, &mut declaring_class);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(declaring_class)
        }
    }

    pub fn get_method_modifiers(&self, method: jni::jmethodID) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut modifiers: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetMethodModifiers.unwrap();
            let err = get_fn(self.env, method, &mut modifiers);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(modifiers)
        }
    }

    pub fn get_max_locals(&self, method: jni::jmethodID) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut max: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetMaxLocals.unwrap();
            let err = get_fn(self.env, method, &mut max);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(max)
        }
    }

    pub fn get_arguments_size(&self, method: jni::jmethodID) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut size: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetArgumentsSize.unwrap();
            let err = get_fn(self.env, method, &mut size);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(size)
        }
    }

    pub fn get_line_number_table(&self, method: jni::jmethodID) -> Result<Vec<jvmti::jvmtiLineNumberEntry>, jvmti::jvmtiError> {
        let mut entry_count: jni::jint = 0;
        let mut table_ptr: *mut jvmti::jvmtiLineNumberEntry = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetLineNumberTable.unwrap();
            let err = get_fn(self.env, method, &mut entry_count, &mut table_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let table = std::slice::from_raw_parts(table_ptr, entry_count as usize).to_vec();
            self.deallocate(table_ptr as *mut u8)?;
            Ok(table)
        }
    }

    pub fn get_method_location(&self, method: jni::jmethodID) -> Result<(jvmti::jlocation, jvmti::jlocation), jvmti::jvmtiError> {
        let mut start: jvmti::jlocation = 0;
        let mut end: jvmti::jlocation = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetMethodLocation.unwrap();
            let err = get_fn(self.env, method, &mut start, &mut end);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok((start, end))
        }
    }

    pub fn get_local_variable_table(&self, method: jni::jmethodID) -> Result<Vec<LocalVariableEntry>, jvmti::jvmtiError> {
        let mut entry_count: jni::jint = 0;
        let mut table_ptr: *mut jvmti::jvmtiLocalVariableEntry = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetLocalVariableTable.unwrap();
            let err = get_fn(self.env, method, &mut entry_count, &mut table_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        let table = unsafe { std::slice::from_raw_parts(table_ptr, entry_count as usize) };
        let base = table_ptr as *const u8;
        let len = (entry_count as usize) * std::mem::size_of::<jvmti::jvmtiLocalVariableEntry>();

        let mut out = Vec::with_capacity(entry_count as usize);
        for entry in table {
            let name = cstr_to_string(entry.name);
            let signature = cstr_to_string(entry.signature);
            let generic_signature = cstr_to_string(entry.generic_signature);
            out.push(LocalVariableEntry {
                start_location: entry.start_location,
                length: entry.length,
                name,
                signature,
                generic_signature,
                slot: entry.slot,
            });

            if !entry.name.is_null() && !ptr_in_range(entry.name as *const u8, base, len) {
                self.deallocate(entry.name as *mut u8)?;
            }
            if !entry.signature.is_null() && !ptr_in_range(entry.signature as *const u8, base, len) {
                self.deallocate(entry.signature as *mut u8)?;
            }
            if !entry.generic_signature.is_null() && !ptr_in_range(entry.generic_signature as *const u8, base, len) {
                self.deallocate(entry.generic_signature as *mut u8)?;
            }
        }
        if !table_ptr.is_null() {
            self.deallocate(table_ptr as *mut u8)?;
        }
        Ok(out)
    }

    pub fn get_bytecodes(&self, method: jni::jmethodID) -> Result<Vec<u8>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut bytecodes_ptr: *mut u8 = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetBytecodes.unwrap();
            let err = get_fn(self.env, method, &mut count, &mut bytecodes_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let bytecodes = std::slice::from_raw_parts(bytecodes_ptr, count as usize).to_vec();
            self.deallocate(bytecodes_ptr)?;
            Ok(bytecodes)
        }
    }

    pub fn is_method_native(&self, method: jni::jmethodID) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).IsMethodNative.unwrap();
            let err = get_fn(self.env, method, &mut res);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(res != 0)
        }
    }

    pub fn is_method_synthetic(&self, method: jni::jmethodID) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).IsMethodSynthetic.unwrap();
            let err = get_fn(self.env, method, &mut res);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(res != 0)
        }
    }

    pub fn is_method_obsolete(&self, method: jni::jmethodID) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).IsMethodObsolete.unwrap();
            let err = get_fn(self.env, method, &mut res);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(res != 0)
        }
    }

    pub fn get_classloader_classes(&self, initiating_loader: jni::jobject) -> Result<Vec<jni::jclass>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut classes_ptr: *mut jni::jclass = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetClassLoaderClasses.unwrap();
            let err = get_fn(self.env, initiating_loader, &mut count, &mut classes_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let classes = std::slice::from_raw_parts(classes_ptr, count as usize).to_vec();
            self.deallocate(classes_ptr as *mut u8)?;
            Ok(classes)
        }
    }

    pub fn get_object_hash_code(&self, object: jni::jobject) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut hash: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetObjectHashCode.unwrap();
            let err = get_fn(self.env, object, &mut hash);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(hash)
        }
    }

    pub fn get_object_monitor_usage(&self, object: jni::jobject) -> Result<MonitorUsage, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiMonitorUsage {
            owner: ptr::null_mut(),
            entry_count: 0,
            waiter_count: 0,
            waiters: ptr::null_mut(),
            notify_waiter_count: 0,
            notify_waiters: ptr::null_mut(),
        };
        unsafe {
            let get_fn = (*(*self.env).functions).GetObjectMonitorUsage.unwrap();
            let err = get_fn(self.env, object, &mut info);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        let waiters = if info.waiter_count > 0 && !info.waiters.is_null() {
            unsafe { std::slice::from_raw_parts(info.waiters, info.waiter_count as usize).to_vec() }
        } else {
            Vec::new()
        };
        let notify_waiters = if info.notify_waiter_count > 0 && !info.notify_waiters.is_null() {
            unsafe { std::slice::from_raw_parts(info.notify_waiters, info.notify_waiter_count as usize).to_vec() }
        } else {
            Vec::new()
        };

        if !info.waiters.is_null() {
            self.deallocate(info.waiters as *mut u8)?;
        }
        if !info.notify_waiters.is_null() {
            self.deallocate(info.notify_waiters as *mut u8)?;
        }

        Ok(MonitorUsage {
            owner: info.owner,
            entry_count: info.entry_count,
            waiters,
            notify_waiters,
        })
    }

    pub fn get_tag(&self, object: jni::jobject) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut tag: jni::jlong = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetTag.unwrap();
            let err = get_fn(self.env, object, &mut tag);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(tag)
        }
    }

    pub fn set_tag(&self, object: jni::jobject, tag: jni::jlong) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetTag.unwrap();
            let err = set_fn(self.env, object, tag);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn force_garbage_collection(&self) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let force_fn = (*(*self.env).functions).ForceGarbageCollection.unwrap();
            let err = force_fn(self.env);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn iterate_over_objects_reachable_from_object(&self, object: jni::jobject, cb: jvmti::jvmtiObjectReferenceCallback, user_data: *const std::os::raw::c_void) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let iter_fn = (*(*self.env).functions).IterateOverObjectsReachableFromObject.unwrap();
            let err = iter_fn(self.env, object, cb, user_data);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn iterate_over_reachable_objects(&self, root_cb: jvmti::jvmtiHeapRootCallback, stack_cb: jvmti::jvmtiStackReferenceCallback, obj_cb: jvmti::jvmtiObjectReferenceCallback, user_data: *const std::os::raw::c_void) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let iter_fn = (*(*self.env).functions).IterateOverReachableObjects.unwrap();
            let err = iter_fn(self.env, root_cb, stack_cb, obj_cb, user_data);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn iterate_over_heap(&self, filter: jni::jint, cb: jvmti::jvmtiObjectCallback, user_data: *const std::os::raw::c_void) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let iter_fn = (*(*self.env).functions).IterateOverHeap.unwrap();
            let err = iter_fn(self.env, filter, cb, user_data);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn iterate_over_instances_of_class(&self, klass: jni::jclass, filter: jni::jint, cb: jvmti::jvmtiObjectCallback, user_data: *const std::os::raw::c_void) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let iter_fn = (*(*self.env).functions).IterateOverInstancesOfClass.unwrap();
            let err = iter_fn(self.env, klass, filter, cb, user_data);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_objects_with_tags(&self, tags: &[jni::jlong]) -> Result<(Vec<jni::jobject>, Vec<jni::jlong>), jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut objects_ptr: *mut jni::jobject = ptr::null_mut();
        let mut tags_ptr: *mut jni::jlong = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetObjectsWithTags.unwrap();
            let err = get_fn(self.env, tags.len() as jni::jint, tags.as_ptr(), &mut count, &mut objects_ptr, &mut tags_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let objects = std::slice::from_raw_parts(objects_ptr, count as usize).to_vec();
            let res_tags = std::slice::from_raw_parts(tags_ptr, count as usize).to_vec();
            self.deallocate(objects_ptr as *mut u8)?;
            self.deallocate(tags_ptr as *mut u8)?;
            Ok((objects, res_tags))
        }
    }

    pub fn follow_references(&self, heap_filter: jni::jint, klass: jni::jclass, initial_object: jni::jobject, callbacks: &jvmti::jvmtiHeapCallbacks, user_data: *const std::os::raw::c_void) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let follow_fn = (*(*self.env).functions).FollowReferences.unwrap();
            let err = follow_fn(self.env, heap_filter, klass, initial_object, callbacks, user_data);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn iterate_through_heap(&self, heap_filter: jni::jint, klass: jni::jclass, callbacks: &jvmti::jvmtiHeapCallbacks, user_data: *const std::os::raw::c_void) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let iter_fn = (*(*self.env).functions).IterateThroughHeap.unwrap();
            let err = iter_fn(self.env, heap_filter, klass, callbacks, user_data);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_object_size(&self, object: jni::jobject) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut size: jni::jlong = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetObjectSize.unwrap();
            let err = get_fn(self.env, object, &mut size);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(size)
        }
    }

    pub fn set_heap_sampling_interval(&self, interval: jni::jint) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetHeapSamplingInterval.unwrap();
            let err = set_fn(self.env, interval);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn set_breakpoint(&self, method: jni::jmethodID, location: jvmti::jlocation) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetBreakpoint.unwrap();
            let err = set_fn(self.env, method, location);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn clear_breakpoint(&self, method: jni::jmethodID, location: jvmti::jlocation) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let clear_fn = (*(*self.env).functions).ClearBreakpoint.unwrap();
            let err = clear_fn(self.env, method, location);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn set_field_access_watch(&self, klass: jni::jclass, field: jni::jfieldID) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetFieldAccessWatch.unwrap();
            let err = set_fn(self.env, klass, field);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn clear_field_access_watch(&self, klass: jni::jclass, field: jni::jfieldID) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let clear_fn = (*(*self.env).functions).ClearFieldAccessWatch.unwrap();
            let err = clear_fn(self.env, klass, field);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn set_field_modification_watch(&self, klass: jni::jclass, field: jni::jfieldID) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetFieldModificationWatch.unwrap();
            let err = set_fn(self.env, klass, field);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn clear_field_modification_watch(&self, klass: jni::jclass, field: jni::jfieldID) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let clear_fn = (*(*self.env).functions).ClearFieldModificationWatch.unwrap();
            let err = clear_fn(self.env, klass, field);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn is_modifiable_class(&self, klass: jni::jclass) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let is_fn = (*(*self.env).functions).IsModifiableClass.unwrap();
            let err = is_fn(self.env, klass, &mut res);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(res != 0)
        }
    }

    pub fn retransform_classes(&self, classes: &[jni::jclass]) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let retransform_fn = (*(*self.env).functions).RetransformClasses.unwrap();
            let err = retransform_fn(self.env, classes.len() as jni::jint, classes.as_ptr());
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn is_modifiable_module(&self, module: jni::jobject) -> Result<bool, jvmti::jvmtiError> {
        let mut res: jni::jboolean = 0;
        unsafe {
            let is_fn = (*(*self.env).functions).IsModifiableModule.unwrap();
            let err = is_fn(self.env, module, &mut res);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(res != 0)
        }
    }

    pub fn add_module_reads(&self, module: jni::jobject, source_module: jni::jobject) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let add_fn = (*(*self.env).functions).AddModuleReads.unwrap();
            let err = add_fn(self.env, module, source_module);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn add_module_exports(&self, module: jni::jobject, package: &str, to_module: jni::jobject) -> Result<(), jvmti::jvmtiError> {
        let c_package = CString::new(package).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)?;
        unsafe {
            let add_fn = (*(*self.env).functions).AddModuleExports.unwrap();
            let err = add_fn(self.env, module, c_package.as_ptr(), to_module);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn add_module_opens(&self, module: jni::jobject, package: &str, to_module: jni::jobject) -> Result<(), jvmti::jvmtiError> {
        let c_package = CString::new(package).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)?;
        unsafe {
            let add_fn = (*(*self.env).functions).AddModuleOpens.unwrap();
            let err = add_fn(self.env, module, c_package.as_ptr(), to_module);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn add_module_uses(&self, module: jni::jobject, service: jni::jclass) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let add_fn = (*(*self.env).functions).AddModuleUses.unwrap();
            let err = add_fn(self.env, module, service);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn add_module_provides(&self, module: jni::jobject, service: jni::jclass, implementation: jni::jclass) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let add_fn = (*(*self.env).functions).AddModuleProvides.unwrap();
            let err = add_fn(self.env, module, service, implementation);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_version_number(&self) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut version: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetVersionNumber.unwrap();
            let err = get_fn(self.env, &mut version);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(version)
        }
    }

    pub fn get_source_debug_extension(&self, klass: jni::jclass) -> Result<String, jvmti::jvmtiError> {
        let mut ext_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetSourceDebugExtension.unwrap();
            let err = get_fn(self.env, klass, &mut ext_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let ext = std::ffi::CStr::from_ptr(ext_ptr).to_string_lossy().into_owned();
            self.deallocate(ext_ptr as *mut u8)?;
            Ok(ext)
        }
    }

    pub fn get_thread_local_storage(&self, thread: jni::jthread) -> Result<*mut std::os::raw::c_void, jvmti::jvmtiError> {
        let mut data: *mut std::os::raw::c_void = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetThreadLocalStorage.unwrap();
            let err = get_fn(self.env, thread, &mut data);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(data)
        }
    }

    pub fn set_thread_local_storage(&self, thread: jni::jthread, data: *const std::os::raw::c_void) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetThreadLocalStorage.unwrap();
            let err = set_fn(self.env, thread, data);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn suspend_all_virtual_threads(&self) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let suspend_fn = (*(*self.env).functions).SuspendAllVirtualThreads.unwrap();
            let err = suspend_fn(self.env);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn resume_all_virtual_threads(&self) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let resume_fn = (*(*self.env).functions).ResumeAllVirtualThreads.unwrap();
            let err = resume_fn(self.env);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn set_jni_function_table(&self, function_table: *const jni::JNIEnv) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetJNIFunctionTable.unwrap();
            let err = set_fn(self.env, function_table);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_jni_function_table(&self) -> Result<*mut jni::JNIEnv, jvmti::jvmtiError> {
        let mut table_ptr: *mut jni::JNIEnv = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetJNIFunctionTable.unwrap();
            let err = get_fn(self.env, &mut table_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(table_ptr)
        }
    }

    pub fn generate_events(&self, event_type: u32) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let gen_fn = (*(*self.env).functions).GenerateEvents.unwrap();
            let err = gen_fn(self.env, event_type);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_extension_functions(&self) -> Result<Vec<ExtensionFunctionInfo>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut ext_ptr: *mut jvmti::jvmtiExtensionFunctionInfo = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetExtensionFunctions.unwrap();
            let err = get_fn(self.env, &mut count, &mut ext_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        let exts = unsafe { std::slice::from_raw_parts(ext_ptr, count as usize) };
        let base = ext_ptr as *const u8;
        let len = (count as usize) * std::mem::size_of::<jvmti::jvmtiExtensionFunctionInfo>();

        let mut out = Vec::with_capacity(count as usize);
        for ext in exts {
            let id = cstr_to_string(ext.id);
            let short_description = cstr_to_string(ext.short_description);

            let mut params = Vec::new();
            if ext.param_count > 0 && !ext.params.is_null() {
                let params_slice = unsafe { std::slice::from_raw_parts(ext.params, ext.param_count as usize) };
                let params_base = ext.params as *const u8;
                let params_len = (ext.param_count as usize) * std::mem::size_of::<jvmti::jvmtiExtensionParamInfo>();
                for p in params_slice {
                    let name = cstr_to_string(p.name);
                    params.push(ExtensionParamInfo {
                        name,
                        kind: p.kind,
                        base_type: p.base_type,
                        null_ok: p.null_ok != 0,
                    });

                    if !p.name.is_null()
                        && !ptr_in_range(p.name as *const u8, params_base, params_len)
                        && !ptr_in_range(p.name as *const u8, base, len)
                    {
                        self.deallocate(p.name as *mut u8)?;
                    }
                }
                if !ptr_in_range(ext.params as *const u8, base, len) {
                    self.deallocate(ext.params as *mut u8)?;
                }
            }

            let errors = if ext.error_count > 0 && !ext.errors.is_null() {
                unsafe { std::slice::from_raw_parts(ext.errors, ext.error_count as usize).to_vec() }
            } else {
                Vec::new()
            };
            if !ext.errors.is_null() && !ptr_in_range(ext.errors as *const u8, base, len) {
                self.deallocate(ext.errors as *mut u8)?;
            }

            if !ext.id.is_null() && !ptr_in_range(ext.id as *const u8, base, len) {
                self.deallocate(ext.id as *mut u8)?;
            }
            if !ext.short_description.is_null() && !ptr_in_range(ext.short_description as *const u8, base, len) {
                self.deallocate(ext.short_description as *mut u8)?;
            }

            out.push(ExtensionFunctionInfo {
                func: ext.func,
                id,
                short_description,
                params,
                errors,
            });
        }

        if !ext_ptr.is_null() {
            self.deallocate(ext_ptr as *mut u8)?;
        }
        Ok(out)
    }

    pub fn get_extension_events(&self) -> Result<Vec<ExtensionEventInfo>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut ext_ptr: *mut jvmti::jvmtiExtensionEventInfo = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetExtensionEvents.unwrap();
            let err = get_fn(self.env, &mut count, &mut ext_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        let exts = unsafe { std::slice::from_raw_parts(ext_ptr, count as usize) };
        let base = ext_ptr as *const u8;
        let len = (count as usize) * std::mem::size_of::<jvmti::jvmtiExtensionEventInfo>();

        let mut out = Vec::with_capacity(count as usize);
        for ext in exts {
            let id = cstr_to_string(ext.id);
            let short_description = cstr_to_string(ext.short_description);

            let mut params = Vec::new();
            if ext.param_count > 0 && !ext.params.is_null() {
                let params_slice = unsafe { std::slice::from_raw_parts(ext.params, ext.param_count as usize) };
                let params_base = ext.params as *const u8;
                let params_len = (ext.param_count as usize) * std::mem::size_of::<jvmti::jvmtiExtensionParamInfo>();
                for p in params_slice {
                    let name = cstr_to_string(p.name);
                    params.push(ExtensionParamInfo {
                        name,
                        kind: p.kind,
                        base_type: p.base_type,
                        null_ok: p.null_ok != 0,
                    });

                    if !p.name.is_null()
                        && !ptr_in_range(p.name as *const u8, params_base, params_len)
                        && !ptr_in_range(p.name as *const u8, base, len)
                    {
                        self.deallocate(p.name as *mut u8)?;
                    }
                }
                if !ptr_in_range(ext.params as *const u8, base, len) {
                    self.deallocate(ext.params as *mut u8)?;
                }
            }

            if !ext.id.is_null() && !ptr_in_range(ext.id as *const u8, base, len) {
                self.deallocate(ext.id as *mut u8)?;
            }
            if !ext.short_description.is_null() && !ptr_in_range(ext.short_description as *const u8, base, len) {
                self.deallocate(ext.short_description as *mut u8)?;
            }

            out.push(ExtensionEventInfo {
                extension_event_index: ext.extension_event_index,
                id,
                short_description,
                params,
            });
        }

        if !ext_ptr.is_null() {
            self.deallocate(ext_ptr as *mut u8)?;
        }
        Ok(out)
    }

    pub fn set_extension_event_callback(&self, extension_event_index: jni::jint, callback: jvmti::jvmtiExtensionEventCallback) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetExtensionEventCallback.unwrap();
            let err = set_fn(self.env, extension_event_index, callback);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_error_name(&self, error: jvmti::jvmtiError) -> Result<String, jvmti::jvmtiError> {
        let mut name_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetErrorName.unwrap();
            let err = get_fn(self.env, error, &mut name_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let name = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy().into_owned();
            self.deallocate(name_ptr as *mut u8)?;
            Ok(name)
        }
    }

    /// Best-effort conversion of a JVMTI error to a readable string.
    pub fn error_to_string(&self, error: jvmti::jvmtiError) -> String {
        self.get_error_name(error).unwrap_or_else(|_| format!("{error:?}"))
    }

    pub fn get_jlocation_format(&self) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut format: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetJLocationFormat.unwrap();
            let err = get_fn(self.env, &mut format);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(format)
        }
    }

    pub fn get_system_properties(&self) -> Result<Vec<String>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut props_ptr: *mut *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetSystemProperties.unwrap();
            let err = get_fn(self.env, &mut count, &mut props_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let mut props = Vec::with_capacity(count as usize);
            let slice = std::slice::from_raw_parts(props_ptr, count as usize);
            let base = props_ptr as *const u8;
            let len = (count as usize) * std::mem::size_of::<*mut std::os::raw::c_char>();
            for &p_ptr in slice {
                props.push(std::ffi::CStr::from_ptr(p_ptr).to_string_lossy().into_owned());
                if !ptr_in_range(p_ptr as *const u8, base, len) {
                    self.deallocate(p_ptr as *mut u8)?;
                }
            }
            self.deallocate(props_ptr as *mut u8)?;
            Ok(props)
        }
    }

    pub fn get_system_property(&self, property: &str) -> Result<String, jvmti::jvmtiError> {
        let c_property = CString::new(property).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)?;
        let mut value_ptr: *mut std::os::raw::c_char = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetSystemProperty.unwrap();
            let err = get_fn(self.env, c_property.as_ptr(), &mut value_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let value = std::ffi::CStr::from_ptr(value_ptr).to_string_lossy().into_owned();
            self.deallocate(value_ptr as *mut u8)?;
            Ok(value)
        }
    }

    pub fn set_system_property(&self, property: &str, value: &str) -> Result<(), jvmti::jvmtiError> {
        let c_property = CString::new(property).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)?;
        let c_value = CString::new(value).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)?;
        unsafe {
            let set_fn = (*(*self.env).functions).SetSystemProperty.unwrap();
            let err = set_fn(self.env, c_property.as_ptr(), c_value.as_ptr());
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_phase(&self) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut phase: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetPhase.unwrap();
            let err = get_fn(self.env, &mut phase);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(phase)
        }
    }

    pub fn get_current_thread_cpu_timer_info(&self) -> Result<jvmti::jvmtiTimerInfo, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiTimerInfo { max_value: 0, may_skip_forward: 0, may_skip_backward: 0, kind: 0 };
        unsafe {
            let get_fn = (*(*self.env).functions).GetCurrentThreadCpuTimerInfo.unwrap();
            let err = get_fn(self.env, &mut info);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(info)
        }
    }

    pub fn get_current_thread_cpu_time(&self) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut nanos: jni::jlong = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetCurrentThreadCpuTime.unwrap();
            let err = get_fn(self.env, &mut nanos);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(nanos)
        }
    }

    pub fn get_thread_cpu_timer_info(&self) -> Result<jvmti::jvmtiTimerInfo, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiTimerInfo { max_value: 0, may_skip_forward: 0, may_skip_backward: 0, kind: 0 };
        unsafe {
            let get_fn = (*(*self.env).functions).GetThreadCpuTimerInfo.unwrap();
            let err = get_fn(self.env, &mut info);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(info)
        }
    }

    pub fn get_thread_cpu_time(&self, thread: jni::jthread) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut nanos: jni::jlong = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetThreadCpuTime.unwrap();
            let err = get_fn(self.env, thread, &mut nanos);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(nanos)
        }
    }

    pub fn get_timer_info(&self) -> Result<jvmti::jvmtiTimerInfo, jvmti::jvmtiError> {
        let mut info = jvmti::jvmtiTimerInfo { max_value: 0, may_skip_forward: 0, may_skip_backward: 0, kind: 0 };
        unsafe {
            let get_fn = (*(*self.env).functions).GetTimerInfo.unwrap();
            let err = get_fn(self.env, &mut info);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(info)
        }
    }

    pub fn get_time(&self) -> Result<jni::jlong, jvmti::jvmtiError> {
        let mut nanos: jni::jlong = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetTime.unwrap();
            let err = get_fn(self.env, &mut nanos);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(nanos)
        }
    }

    pub fn relinquish_capabilities(&self, caps: &jvmti::jvmtiCapabilities) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let rel_fn = (*(*self.env).functions).RelinquishCapabilities.unwrap();
            let err = rel_fn(self.env, caps);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_available_processors(&self) -> Result<jni::jint, jvmti::jvmtiError> {
        let mut processors: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetAvailableProcessors.unwrap();
            let err = get_fn(self.env, &mut processors);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(processors)
        }
    }

    pub fn get_class_version_numbers(&self, klass: jni::jclass) -> Result<(jni::jint, jni::jint), jvmti::jvmtiError> {
        let mut minor: jni::jint = 0;
        let mut major: jni::jint = 0;
        unsafe {
            let get_fn = (*(*self.env).functions).GetClassVersionNumbers.unwrap();
            let err = get_fn(self.env, klass, &mut minor, &mut major);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok((minor, major))
        }
    }

    pub fn get_constant_pool(&self, klass: jni::jclass) -> Result<Vec<u8>, jvmti::jvmtiError> {
        let mut pool_count: jni::jint = 0;
        let mut byte_count: jni::jint = 0;
        let mut bytes_ptr: *mut u8 = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetConstantPool.unwrap();
            let err = get_fn(self.env, klass, &mut pool_count, &mut byte_count, &mut bytes_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let bytes = std::slice::from_raw_parts(bytes_ptr, byte_count as usize).to_vec();
            self.deallocate(bytes_ptr)?;
            Ok(bytes)
        }
    }

    pub fn get_environment_local_storage(&self) -> Result<*mut std::os::raw::c_void, jvmti::jvmtiError> {
        let mut data: *mut std::os::raw::c_void = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetEnvironmentLocalStorage.unwrap();
            let err = get_fn(self.env, &mut data);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            Ok(data)
        }
    }

    pub fn set_environment_local_storage(&self, data: *const std::os::raw::c_void) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetEnvironmentLocalStorage.unwrap();
            let err = set_fn(self.env, data);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn add_to_bootstrap_class_loader_search(&self, segment: &str) -> Result<(), jvmti::jvmtiError> {
        let c_segment = CString::new(segment).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)?;
        unsafe {
            let add_fn = (*(*self.env).functions).AddToBootstrapClassLoaderSearch.unwrap();
            let err = add_fn(self.env, c_segment.as_ptr());
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn set_verbose_flag(&self, flag: jni::jint, value: bool) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let set_fn = (*(*self.env).functions).SetVerboseFlag.unwrap();
            let err = set_fn(self.env, flag, if value { 1 } else { 0 });
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn add_to_system_class_loader_search(&self, segment: &str) -> Result<(), jvmti::jvmtiError> {
        let c_segment = CString::new(segment).map_err(|_| jvmti::jvmtiError::ILLEGAL_ARGUMENT)?;
        unsafe {
            let add_fn = (*(*self.env).functions).AddToSystemClassLoaderSearch.unwrap();
            let err = add_fn(self.env, c_segment.as_ptr());
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    pub fn get_owned_monitor_stack_depth_info(&self, thread: jni::jthread) -> Result<Vec<jvmti::jvmtiMonitorStackDepthInfo>, jvmti::jvmtiError> {
        let mut count: jni::jint = 0;
        let mut info_ptr: *mut jvmti::jvmtiMonitorStackDepthInfo = ptr::null_mut();
        unsafe {
            let get_fn = (*(*self.env).functions).GetOwnedMonitorStackDepthInfo.unwrap();
            let err = get_fn(self.env, thread, &mut count, &mut info_ptr);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
            let info = std::slice::from_raw_parts(info_ptr, count as usize).to_vec();
            self.deallocate(info_ptr as *mut u8)?;
            Ok(info)
        }
    }

    // =========================================================================
    // Native Method Prefixes
    // =========================================================================

    /// Sets a prefix for native method resolution.
    ///
    /// When the JVM attempts to resolve a native method, it will first try the
    /// prefixed name before falling back to the original name. This is useful
    /// for wrapping native methods with instrumentation.
    ///
    /// Requires `can_set_native_method_prefix` capability.
    ///
    /// # Example
    ///
    /// If prefix is "wrapped_" and native method is `native void foo()`,
    /// the JVM will first look for `wrapped_foo` before `foo`.
    pub fn set_native_method_prefix(&self, prefix: &str) -> Result<(), jvmti::jvmtiError> {
        let c_prefix = std::ffi::CString::new(prefix).map_err(|_| jvmti::jvmtiError::NULL_POINTER)?;
        unsafe {
            let set_fn = (*(*self.env).functions).SetNativeMethodPrefix.unwrap();
            let err = set_fn(self.env, c_prefix.as_ptr() as *mut _);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    /// Sets multiple prefixes for native method resolution.
    ///
    /// The JVM will try each prefix in order when resolving native methods.
    /// This allows multiple agents to each wrap native methods.
    ///
    /// Requires `can_set_native_method_prefix` capability.
    pub fn set_native_method_prefixes(&self, prefixes: &[&str]) -> Result<(), jvmti::jvmtiError> {
        let c_prefixes: Vec<std::ffi::CString> = prefixes
            .iter()
            .map(|p| std::ffi::CString::new(*p).map_err(|_| jvmti::jvmtiError::NULL_POINTER))
            .collect::<Result<Vec<_>, _>>()?;
        let mut prefix_ptrs: Vec<*mut std::os::raw::c_char> = c_prefixes
            .iter()
            .map(|s: &std::ffi::CString| s.as_ptr() as *mut std::os::raw::c_char)
            .collect();
        unsafe {
            let set_fn = (*(*self.env).functions).SetNativeMethodPrefixes.unwrap();
            let err = set_fn(self.env, prefixes.len() as jni::jint, prefix_ptrs.as_mut_ptr());
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

    // =========================================================================
    // Frame Pops (JDK 27+)
    // =========================================================================

    /// Clears all pending frame pop notifications for a thread.
    ///
    /// This removes all frame pop notifications that were requested via
    /// `notify_frame_pop` for the specified thread.
    ///
    /// **Note**: This function was added in JDK 27. Calling it on older JVMs
    /// will result in a null pointer dereference or undefined behavior.
    ///
    /// Requires `can_generate_frame_pop_events` capability.
    pub fn clear_all_frame_pops(&self, thread: jni::jthread) -> Result<(), jvmti::jvmtiError> {
        unsafe {
            let clear_fn = (*(*self.env).functions).ClearAllFramePops.unwrap();
            let err = clear_fn(self.env, thread);
            if err != jvmti::jvmtiError::NONE { return Err(err); }
        }
        Ok(())
    }

}
