//! Example: embed a JVM inside a Rust process.
//!
//! Run with:
//!   JAVA_HOME=/path/to/jdk \
//!   cargo run --example embed --features embed

use std::error::Error;
use std::io;

use jvmti_bindings::embed::find_libjvm_verbose;
use jvmti_bindings::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let builder = JavaVmBuilder::default()
        .option("-Xms64m")?
        .option("-Xmx256m")?
        .option("-Djava.class.path=./myapp.jar")?;

    let libjvm = find_libjvm_verbose()?;
    let vm = builder.create_from_library(libjvm)?;

    let env = unsafe { vm.creator_env() };
    let system = env
        .find_class("java/lang/System")
        .ok_or_else(|| io::Error::other("java.lang.System was not found"))?;
    let key = env
        .new_string_utf("java.version")
        .ok_or_else(|| io::Error::other("could not allocate java.version key"))?;
    // `system` and `key` were returned by this environment and remain live in
    // the current local-reference frame.
    let value = unsafe {
        let get_prop = env
            .get_static_method_id(
                system,
                "getProperty",
                "(Ljava/lang/String;)Ljava/lang/String;",
            )
            .ok_or_else(|| io::Error::other("System.getProperty was not found"))?;
        env.call_static_object_method(system, get_prop, &[jni::jvalue { l: key }])
    };

    if env.exception_check() {
        env.exception_describe();
        env.exception_clear();
    } else {
        let version =
            unsafe { env.get_string_utf(value) }.unwrap_or_else(|| "<unknown>".to_string());
        println!("java.version={}", version);
    }

    if let Err(code) = vm.destroy() {
        return Err(std::io::Error::other(format!(
            "DestroyJavaVM failed: {} ({code})",
            jni::result_name(code)
        ))
        .into());
    }
    Ok(())
}
