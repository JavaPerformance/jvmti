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
    let _system = env.find_class("java/lang/System").unwrap();

    vm.destroy()?;
    Ok(())
}
