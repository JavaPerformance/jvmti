//! Example: embed a JVM inside a Rust process.
//!
//! Run with:
//!   JAVA_HOME=/path/to/jdk \
//!   cargo run --example embed --features embed

use std::error::Error;

use jvmti_bindings::prelude::*;
use jvmti_bindings::embed::find_libjvm_verbose;

fn main() -> Result<(), Box<dyn Error>> {
    let builder = JavaVmBuilder::new(jni::JNI_VERSION_1_8)
        .option("-Xms64m")?
        .option("-Xmx256m")?
        .option("-Djava.class.path=./myapp.jar")?;

    let libjvm = find_libjvm_verbose()?;
    let vm = builder.create_from_library(libjvm)?;

    let env = unsafe { vm.creator_env() };
    let system = env.find_class("java/lang/System").unwrap();
    let get_prop = env
        .get_static_method_id(system, "getProperty", "(Ljava/lang/String;)Ljava/lang/String;")
        .unwrap();

    let key = env.new_string_utf("java.version").unwrap();
    let value = env.call_static_object_method(system, get_prop, &[jni::jvalue { l: key }]);

    if env.exception_check() {
        env.exception_describe();
        env.exception_clear();
    } else {
        let version = env.get_string_utf(value).unwrap_or_else(|| "<unknown>".to_string());
        println!("java.version={}", version);
    }

    if let Err(code) = vm.destroy() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("DestroyJavaVM failed: {code}"),
        )
        .into());
    }
    Ok(())
}
