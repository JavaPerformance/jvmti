#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
WORK="$ROOT/target/attach-policy-live-proof"
AGENT="$WORK/agent"
JAVA_SRC="$WORK/java"
rm -rf "$WORK"
mkdir -p "$AGENT/src" "$JAVA_SRC"

cat > "$AGENT/Cargo.toml" <<EOF
[package]
name = "attach-policy-live-agent"
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

use jvmti_bindings::prelude::*;

#[derive(Default)]
struct AttachPolicyProof;

fn write_marker(context: AgentLoadContext<'_>, marker: &str) -> jni::jint {
    let Some(path) = context.options_str().ok().flatten() else {
        return jni::JNI_ERR;
    };
    let Ok(mut output) = OpenOptions::new().create(true).append(true).open(path.as_ref()) else {
        return jni::JNI_ERR;
    };
    if writeln!(output, "{marker}").is_err() {
        return jni::JNI_ERR;
    }
    jni::JNI_OK
}

impl Agent for AttachPolicyProof {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        write_marker(context, "startup")
    }

    fn on_attach(&self, context: AgentLoadContext<'_>) -> jni::jint {
        write_marker(context, "attach")
    }
}

jvmti_bindings::export_agent!(AttachPolicyProof);
RS

cat > "$JAVA_SRC/AttachTarget.java" <<'JAVA'
import java.io.FileWriter;
import java.lang.management.ManagementFactory;

public final class AttachTarget {
    public static void main(String[] args) throws Exception {
        String pid = ManagementFactory.getRuntimeMXBean().getName().split("@")[0];
        try (FileWriter out = new FileWriter(args[0])) {
            out.write(pid + "\n");
        }
        Thread.sleep(30000L);
    }
}
JAVA

cat > "$JAVA_SRC/AttachOnce.java" <<'JAVA'
import com.sun.tools.attach.VirtualMachine;

public final class AttachOnce {
    public static void main(String[] args) throws Exception {
        VirtualMachine vm = VirtualMachine.attach(args[0]);
        try {
            vm.loadAgentPath(args[1], args[2]);
        } finally {
            vm.detach();
        }
    }
}
JAVA

cargo +1.85.0 build --release --manifest-path "$AGENT/Cargo.toml" \
  --target-dir "$WORK/cargo-target"
LIB="$WORK/cargo-target/release/libattach_policy_live_agent.so"

declare -a jdks=(
  "/opt/openjdk-bin-21.0.11_p10"
  "/opt/openjdk-bin-25.0.3_p9"
  "/opt/openjdk-bin-27_alpha20"
)
if (($#)); then
  jdks=("$@")
fi

compiler_home=${jdks[0]}
"$compiler_home/bin/javac" --add-modules jdk.attach -d "$JAVA_SRC" \
  "$JAVA_SRC/AttachTarget.java" "$JAVA_SRC/AttachOnce.java"

target_pid=
cleanup() {
  if [[ -n "$target_pid" ]]; then
    kill "$target_pid" 2>/dev/null || true
    wait "$target_pid" 2>/dev/null || true
    target_pid=
  fi
}
trap cleanup EXIT

feature_number() {
  local home=$1 version
  version=$($home/bin/java -version 2>&1 | head -1 | sed -E 's/.*version "([^"]+)".*/\1/')
  version=${version%%.*}
  version=${version%%-*}
  version=${version%%+*}
  printf '%s\n' "$version"
}

start_target() {
  local home=$1 policy=$2 pid_file=$3 log_file=$4
  rm -f "$pid_file"
  "$home/bin/java" "$policy" -cp "$JAVA_SRC" AttachTarget "$pid_file" \
    >"$log_file" 2>&1 &
  target_pid=$!
  for _ in $(seq 1 100); do
    [[ -s "$pid_file" ]] && return 0
    if ! kill -0 "$target_pid" 2>/dev/null; then
      cat "$log_file" >&2
      return 1
    fi
    sleep 0.05
  done
  echo "error: target JVM did not publish its PID" >&2
  cat "$log_file" >&2
  return 1
}

for home in "${jdks[@]}"; do
  [[ -x "$home/bin/java" && -x "$home/bin/javac" ]] || {
    echo "error: incomplete JDK at $home" >&2
    exit 1
  }
  feature=$(feature_number "$home")
  ((feature >= 21)) || {
    echo "error: attach-policy proof requires JDK 21+, got $feature" >&2
    exit 1
  }

  startup_result="$WORK/jdk-${feature}.startup.result"
  rm -f "$startup_result"
  "$home/bin/java" -XX:-EnableDynamicAgentLoading \
    "-agentpath:$LIB=$startup_result" -version \
    >"$WORK/jdk-${feature}.startup.log" 2>&1
  [[ "$(cat "$startup_result" 2>/dev/null || true)" == "startup" ]] || {
    echo "error: startup loading failed with dynamic loading disabled on JDK $feature" >&2
    exit 1
  }

  denied_pid="$WORK/jdk-${feature}.denied.pid"
  denied_result="$WORK/jdk-${feature}.denied.result"
  rm -f "$denied_result"
  start_target "$home" -XX:-EnableDynamicAgentLoading "$denied_pid" \
    "$WORK/jdk-${feature}.denied.target.log"
  java_pid=$(tr -d '\r\n' < "$denied_pid")
  set +e
  "$home/bin/java" --add-modules jdk.attach -cp "$JAVA_SRC" AttachOnce \
    "$java_pid" "$LIB" "$denied_result" \
    >"$WORK/jdk-${feature}.denied.attach.log" 2>&1
  denied_status=$?
  set -e
  cleanup
  if ((denied_status == 0)) || [[ -e "$denied_result" ]]; then
    echo "error: dynamic attach was not denied on JDK $feature" >&2
    exit 1
  fi

  enabled_pid="$WORK/jdk-${feature}.enabled.pid"
  enabled_result="$WORK/jdk-${feature}.enabled.result"
  rm -f "$enabled_result"
  start_target "$home" -XX:+EnableDynamicAgentLoading "$enabled_pid" \
    "$WORK/jdk-${feature}.enabled.target.log"
  java_pid=$(tr -d '\r\n' < "$enabled_pid")
  "$home/bin/java" --add-modules jdk.attach -cp "$JAVA_SRC" AttachOnce \
    "$java_pid" "$LIB" "$enabled_result" \
    >"$WORK/jdk-${feature}.enabled.attach.log" 2>&1
  cleanup
  [[ "$(cat "$enabled_result" 2>/dev/null || true)" == "attach" ]] || {
    echo "error: explicitly enabled dynamic attach failed on JDK $feature" >&2
    exit 1
  }

  echo "JDK $feature attach policy: startup=yes denied=yes enabled=yes"
done
