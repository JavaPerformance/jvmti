use std::error::Error;
use std::io;

use jvmti_bindings::prelude::*;

fn main() -> Result<(), Box<dyn Error>> {
    let vm = JavaVmBuilder::default()
        .option("-Xms16m")?
        .option("-Xmx64m")?
        .create()?;

    let version = vm
        .with_attached_current_thread(|env| {
            let class = env.find_class_cstr(c"java/lang/Runtime")?;
            let method = unsafe {
                env.get_static_method_id_cstr(class, c"version", c"()Ljava/lang/Runtime$Version;")?
            };
            Some(unsafe { env.call_static_object_method(class, method, &[]) })
        })
        .map_err(|code| {
            io::Error::other(format!(
                "thread attachment failed: {} ({code})",
                jni::result_name(code)
            ))
        })?;
    if version.is_none() {
        return Err("Runtime.version() was unavailable".into());
    }

    vm.destroy()
        .map_err(|code| io::Error::other(format!("DestroyJavaVM failed: {code}")))?;
    Ok(())
}
