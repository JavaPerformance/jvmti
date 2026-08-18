#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
WORK="$ROOT/target/repeated-attach-live-proof"
AGENT="$WORK/agent"
JAVA_SRC="$WORK/java"
mkdir -p "$AGENT/src" "$JAVA_SRC"

cat > "$AGENT/Cargo.toml" <<EOF
[package]
name = "repeated-attach-live-agent"
version = "0.0.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
jvmti-bindings = { path = "$ROOT" }
EOF

cat > "$AGENT/src/lib.rs" <<'RS'
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct RepeatedAttachProof {
    attaches: AtomicUsize,
}

impl Agent for RepeatedAttachProof {
    fn on_load(&self, _context: AgentLoadContext<'_>) -> jni::jint {
        jni::JNI_OK
    }

    fn on_attach(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Some(path) = context.options_str().ok().flatten() else {
            return jni::JNI_ERR;
        };
        let count = self.attaches.fetch_add(1, Ordering::SeqCst) + 1;
        let Ok(mut output) = OpenOptions::new().create(true).append(true).open(path.as_ref()) else {
            return jni::JNI_ERR;
        };
        if writeln!(output, "{count}").is_err() {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }
}

jvmti_bindings::export_agent!(RepeatedAttachProof);
RS

cat > "$JAVA_SRC/AttachTarget.java" <<'JAVA'
import java.io.FileWriter;
import java.lang.management.ManagementFactory;

public final class AttachTarget {
    public static void main(String[] args) throws Exception {
        String pid = ManagementFactory.getRuntimeMXBean().getName().split("@")[0];
        FileWriter out = new FileWriter(args[0]);
        out.write(pid + "\n");
        out.close();
        Thread.sleep(30000L);
    }
}
JAVA

cat > "$JAVA_SRC/AttachTwice.java" <<'JAVA'
import com.sun.tools.attach.VirtualMachine;

public final class AttachTwice {
    public static void main(String[] args) throws Exception {
        VirtualMachine vm = VirtualMachine.attach(args[0]);
        try {
            vm.loadAgentPath(args[1], args[2]);
            vm.loadAgentPath(args[1], args[2]);
        } finally {
            vm.detach();
        }
    }
}
JAVA

cargo +1.85.0 build --release --manifest-path "$AGENT/Cargo.toml" \
  --target-dir "$WORK/cargo-target"
LIB="$WORK/cargo-target/release/librepeated_attach_live_agent.so"

declare -a jdks=(
  "/opt/openjdk-bin-8.492_p09"
  "/opt/openjdk-bin-27_alpha20"
)
if (($#)); then
  jdks=("$@")
fi

"${jdks[0]}/bin/javac" -cp "${jdks[0]}/lib/tools.jar" -d "$JAVA_SRC" \
  "$JAVA_SRC/AttachTarget.java" "$JAVA_SRC/AttachTwice.java"

target_pid=
cleanup() {
  if [[ -n "$target_pid" ]]; then
    kill "$target_pid" 2>/dev/null || true
    wait "$target_pid" 2>/dev/null || true
  fi
}
trap cleanup EXIT

for home in "${jdks[@]}"; do
  version=$($home/bin/java -version 2>&1 | head -1 | sed -E 's/.*version "([^"]+)".*/\1/')
  if [[ "$version" == 1.8.* ]]; then
    feature=8
  else
    feature=${version%%.*}
    feature=${feature%%-*}
    feature=${feature%%+*}
  fi
  pid_file="$WORK/jdk-${feature}.pid"
  result_file="$WORK/jdk-${feature}.result"
  rm -f "$pid_file" "$result_file"
  target_options=()
  attach_options=()
  classpath="$JAVA_SRC"
  if ((feature == 8)); then
    classpath="$classpath:$home/lib/tools.jar"
  else
    target_options+=("-XX:+EnableDynamicAgentLoading")
    attach_options+=("--add-modules" "jdk.attach")
  fi
  "$home/bin/java" "${target_options[@]}" -cp "$classpath" AttachTarget "$pid_file" \
    > "$WORK/jdk-${feature}.target.log" 2>&1 &
  target_pid=$!
  for _ in $(seq 1 100); do
    [[ -s "$pid_file" ]] && break
    sleep 0.05
  done
  [[ -s "$pid_file" ]] || {
    echo "error: target JVM did not publish its PID for JDK $feature" >&2
    exit 1
  }
  java_pid=$(tr -d '\r\n' < "$pid_file")
  "$home/bin/java" "${attach_options[@]}" -cp "$classpath" AttachTwice \
    "$java_pid" "$LIB" "$result_file"
  if [[ "$(cat "$result_file" 2>/dev/null || true)" != $'1\n2' ]]; then
    echo "error: repeated attach proof failed on $home" >&2
    cat "$result_file" 2>/dev/null >&2 || true
    exit 1
  fi
  echo "JDK $feature repeated Agent_OnAttach proof: 1, 2"
  cleanup
  target_pid=
done
