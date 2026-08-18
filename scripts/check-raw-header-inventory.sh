#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

HEADER=${JVMTI_HEADER:-}
if [[ -z "$HEADER" ]]; then
  for candidate in \
    target/abi-conformance/jdk28/include/jvmti.h \
    /opt/openjdk-bin-28/include/jvmti.h \
    /opt/openjdk-bin-27_alpha20/include/jvmti.h; do
    if [[ -f "$candidate" ]]; then
      HEADER=$candidate
      break
    fi
  done
fi
if [[ -z "$HEADER" && -n "${JAVA_HOME:-}" ]]; then
  HEADER="$JAVA_HOME/include/jvmti.h"
fi
if [[ -z "$HEADER" || ! -f "$HEADER" ]]; then
  echo "error: set JVMTI_HEADER or JAVA_HOME to an audited JDK jvmti.h" >&2
  exit 2
fi

OUT=target/raw-header-inventory
mkdir -p "$OUT"

grep -oE 'JVMTI_[A-Z0-9_]+' "$HEADER" \
  | grep -v '^JVMTI_H_$' \
  | sort -u > "$OUT/header-constants.txt"
sed -nE 's/.*pub const (JVMTI_[A-Z0-9_]+).*/\1/p' src/sys/jvmti.rs \
  | sort -u > "$OUT/rust-constants.txt"

comm -23 "$OUT/header-constants.txt" "$OUT/rust-constants.txt" \
  > "$OUT/missing-constants.txt"
if [[ -s "$OUT/missing-constants.txt" ]]; then
  echo "error: raw JVMTI constants are missing:" >&2
  sed 's/^/  /' "$OUT/missing-constants.txt" >&2
  exit 1
fi

required_types=(
  jthreadGroup jniNativeInterface jvmtiEvent jvmtiEventMode
  jvmtiHeapObjectFilter jvmtiHeapReferenceKind jvmtiHeapRootKind
  jvmtiIterationControl jvmtiJlocationFormat jvmtiObjectReferenceKind
  jvmtiParamKind jvmtiParamTypes jvmtiPhase jvmtiPrimitiveType
  jvmtiTimerKind jvmtiVerboseFlag jvmtiInterface_1
  jvmtiEventBreakpoint jvmtiEventClassFileLoadHook jvmtiEventClassLoad
  jvmtiEventClassPrepare jvmtiEventCompiledMethodLoad
  jvmtiEventCompiledMethodUnload jvmtiEventDataDumpRequest
  jvmtiEventDynamicCodeGenerated jvmtiEventException
  jvmtiEventExceptionCatch jvmtiEventFieldAccess
  jvmtiEventFieldModification jvmtiEventFramePop
  jvmtiEventGarbageCollectionFinish jvmtiEventGarbageCollectionStart
  jvmtiEventMethodEntry jvmtiEventMethodExit
  jvmtiEventMonitorContendedEnter jvmtiEventMonitorContendedEntered
  jvmtiEventMonitorWait jvmtiEventMonitorWaited
  jvmtiEventNativeMethodBind jvmtiEventObjectFree jvmtiEventReserved
  jvmtiEventResourceExhausted jvmtiEventSampledObjectAlloc
  jvmtiEventSingleStep jvmtiEventThreadEnd jvmtiEventThreadStart
  jvmtiEventVMDeath jvmtiEventVMInit jvmtiEventVMObjectAlloc
  jvmtiEventVMStart jvmtiEventVirtualThreadEnd jvmtiEventVirtualThreadStart
)

: > "$OUT/missing-types.txt"
for type_name in "${required_types[@]}"; do
  if ! grep -Eq "^pub type ${type_name}(<[^>]+>)? =" src/sys/jvmti.rs; then
    echo "$type_name" >> "$OUT/missing-types.txt"
  fi
done
if [[ -s "$OUT/missing-types.txt" ]]; then
  echo "error: raw JVMTI typedef aliases are missing:" >&2
  sed 's/^/  /' "$OUT/missing-types.txt" >&2
  exit 1
fi

echo "raw JVMTI inventory matches $(realpath "$HEADER")"
JVMTI_HEADER="$HEADER" "$ROOT/scripts/check-raw-constant-values.sh"
