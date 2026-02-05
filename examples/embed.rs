//! Example: embed a JVM inside a Rust process.
//!
//! Run with:
//!   JAVA_HOME=/path/to/jdk \
//!   cargo run --example embed --features embed

use std::error::Error;

use jvmti_bindings::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let vm = JavaVmBuilder::new(jni::JNI_VERSION_1_8)
        .option("-Xms64m")?
        .option("-Xmx256m")?
        .option("-Djava.class.path=./myapp.jar")?
        .create()?;

    let env = unsafe { vm.creator_env() };
    let _system = env.find_class("java/lang/System").unwrap();

    vm.destroy()?;
    Ok(())
}
