#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
WORK="$ROOT/target/heap-graph-live-proof"
AGENT="$WORK/agent"
JAVA_SRC="$WORK/java"
mkdir -p "$AGENT/src" "$JAVA_SRC"

cat > "$AGENT/Cargo.toml" <<EOF
[package]
name = "heap-graph-live-agent"
version = "0.0.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
jvmti-bindings = { path = "$ROOT", features = ["heap-graph"] }
EOF

cat > "$AGENT/src/lib.rs" <<'RS'
use std::path::PathBuf;
use std::sync::Mutex;

use jvmti_bindings::advanced::heap_graph::{build_heap_graph, tag_all_objects};
use jvmti_bindings::prelude::*;

#[derive(Default)]
struct HeapGraphProof {
    output: Mutex<Option<PathBuf>>,
}

impl Agent for HeapGraphProof {
    fn on_load(&self, context: AgentLoadContext<'_>) -> jni::jint {
        let Some(path) = context.options_str().ok().flatten() else {
            return jni::JNI_ERR;
        };
        *self.output.lock().unwrap() = Some(PathBuf::from(path.as_ref()));
        let Ok(jvmti) = context.vm().jvmti() else {
            return jni::JNI_ERR;
        };
        if jvmti
            .add_capabilities_with(|capabilities| capabilities.set_can_tag_objects(true))
            .is_err()
            || jvmti.set_default_agent_callbacks().is_err()
            || jvmti
                .enable_events_global(&[jvmti::JVMTI_EVENT_VM_INIT])
                .is_err()
        {
            return jni::JNI_ERR;
        }
        jni::JNI_OK
    }

    fn vm_init(&self, context: CallbackContext<'_>, _event: ThreadEvent) {
        let Ok(tags) = tag_all_objects(context.jvmti(), 1) else {
            return;
        };
        let Ok(graph) = (unsafe {
            build_heap_graph(
                context.jvmti(),
                0,
                std::ptr::null_mut(),
            )
        }) else {
            return;
        };
        if tags.tagged <= 0 || graph.edges.is_empty() {
            return;
        }
        if let Some(path) = self.output.lock().unwrap().as_ref() {
            std::fs::write(
                path,
                format!("ok tagged={} edges={}\n", tags.tagged, graph.edges.len()),
            )
            .unwrap();
        }
    }
}

jvmti_bindings::export_agent!(HeapGraphProof);
RS

cat > "$JAVA_SRC/HeapGraphProofMain.java" <<'JAVA'
import java.util.ArrayList;

public final class HeapGraphProofMain {
    private static final ArrayList<Object> ROOT = new ArrayList<Object>();
    public static void main(String[] args) {
        for (int i = 0; i < 1000; i++) ROOT.add(new Object[] { "node-" + i, ROOT });
        System.out.println("heap-graph-live-proof-main " + ROOT.size());
    }
}
JAVA

cargo +1.85.0 build --release --manifest-path "$AGENT/Cargo.toml" \
  --target-dir "$WORK/cargo-target"
LIB="$WORK/cargo-target/release/libheap_graph_live_agent.so"

declare -a jdks=(
  "/opt/openjdk-bin-8.492_p09"
  "/opt/openjdk-bin-27_alpha20"
)
if (($#)); then
  jdks=("$@")
fi

"${jdks[0]}/bin/javac" -d "$JAVA_SRC" "$JAVA_SRC/HeapGraphProofMain.java"
for home in "${jdks[@]}"; do
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
  "$home/bin/java" -agentpath:"$LIB=$sentinel" -cp "$JAVA_SRC" HeapGraphProofMain
  result=$(cat "$sentinel" 2>/dev/null || true)
  if [[ "$result" != ok\ tagged=*\ edges=* ]]; then
    echo "error: live heap-graph proof failed on $home: $result" >&2
    exit 1
  fi
  echo "JDK $feature live heap-graph proof: $result"
done
