#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
WORK="$ROOT/target/mutf8-live-proof"
AGENT="$WORK/agent"
JAVA_SRC="$WORK/java"
mkdir -p "$AGENT/src" "$JAVA_SRC"

cat > "$AGENT/Cargo.toml" <<EOF
[package]
name = "mutf8-live-agent"
version = "0.0.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
jvmti-bindings = { path = "$ROOT" }
EOF

cat > "$AGENT/src/lib.rs" <<'RS'
use std::path::PathBuf;
use std::sync::Mutex;

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct Mutf8Proof {
    output: Mutex<Option<PathBuf>>,
}

impl Agent for Mutf8Proof {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Some(path) = context.options_str().ok().flatten() else {
            return jni::JNI_ERR;
        };
        *self.output.lock().unwrap() = Some(PathBuf::from(path.as_ref()));
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[jvmti::JVMTI_EVENT_VM_INIT])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn vm_init(&self, context: CallbackContext<'_>, _event: ThreadEvent) {
        let Some(jni_env) = context.jni() else {
            return;
        };
        let expected = "nul=\0,rocket=\u{1f680},text=日本語";
        let Some(string) = jni_env.new_string_utf(expected) else {
            return;
        };
        let strict = unsafe { jni_env.get_string_utf(string) };
        let utf16 = unsafe { jni_env.get_string_utf16(string) };
        unsafe { jni_env.delete_local_ref(string.cast()) };
        if strict.as_deref() != Some(expected)
            || utf16.as_deref() != Some(expected.encode_utf16().collect::<Vec<_>>().as_slice())
        {
            return;
        }

        let exact_unpaired = [0xd800, b'A' as u16, 0xdc00];
        let Some(unpaired) = jni_env.new_string_utf16(&exact_unpaired) else {
            return;
        };
        let exact = unsafe { jni_env.get_string_utf16(unpaired) };
        let strict = unsafe { jni_env.get_string_utf(unpaired) };
        unsafe { jni_env.delete_local_ref(unpaired.cast()) };
        if exact.as_deref() != Some(exact_unpaired.as_slice()) || strict.is_some() {
            return;
        }

        if let Some(path) = self.output.lock().unwrap().as_ref() {
            std::fs::write(path, b"ok\n").unwrap();
        }
    }
}

jvmti_bindings::export_agent!(Mutf8Proof);
RS

cat > "$JAVA_SRC/Mutf8ProofMain.java" <<'JAVA'
public final class Mutf8ProofMain {
    public static void main(String[] args) {
        System.out.println("mutf8-live-proof-main");
    }
}
JAVA

cargo +1.85.0 build --release --manifest-path "$AGENT/Cargo.toml" \
  --target-dir "$WORK/cargo-target"
LIB="$WORK/cargo-target/release/libmutf8_live_agent.so"

declare -a jdks=(
  "/opt/openjdk-bin-8.492_p09"
  "/opt/openjdk-bin-27_alpha20"
)
if (($#)); then
  jdks=("$@")
fi

"${jdks[0]}/bin/javac" -d "$JAVA_SRC" "$JAVA_SRC/Mutf8ProofMain.java"
for home in "${jdks[@]}"; do
  [[ -x "$home/bin/java" ]] || {
    echo "error: java not found under $home" >&2
    exit 2
  }
  version=$($home/bin/java -version 2>&1 | head -1 | sed -E 's/.*version "([^"]+)".*/\1/')
  if [[ "$version" == 1.8.* ]]; then
    feature=8
  else
    feature=${version%%.*}
    feature=${feature%%-*}
    feature=${feature%%+*}
  fi
  sentinel="$WORK/jdk-${feature}.sentinel"
  rm -f "$sentinel"
  "$home/bin/java" -agentpath:"$LIB=$sentinel" -cp "$JAVA_SRC" Mutf8ProofMain
  if [[ "$(cat "$sentinel" 2>/dev/null || true)" != "ok" ]]; then
    echo "error: live Modified UTF-8 proof failed on $home" >&2
    exit 1
  fi
  echo "JDK $feature live Modified UTF-8 proof: ok"
done
