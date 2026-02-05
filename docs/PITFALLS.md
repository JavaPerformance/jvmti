# JVMTI and JNI Pitfalls

This guide highlights common footguns when writing JVM agents.

1. Enabling events without the required capability yields silent failures.
2. Calling JNI from `GarbageCollectionStart/Finish` is forbidden.
3. Holding JVM monitors while calling JVMTI can deadlock the VM.
4. `JNIEnv` is thread-local and invalid on other threads.
5. Callback order can differ across JVM implementations.
6. `ClassFileLoadHook` can be called concurrently and very early.
7. Allocating memory in hot callbacks can cause severe pauses.
8. Mis-sized `new_class_data_len` corrupts class loading.
9. JVMTI buffers must be deallocated with `Deallocate`, not `free`.
10. Some events are disabled by default for performance reasons.
