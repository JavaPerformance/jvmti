#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

feature=${1:-28}
row=$(awk -F '\t' -v feature="$feature" '$1 == feature { print; exit }' \
  tests/abi/openjdk-releases.tsv)
if [[ -z "$row" ]]; then
  echo "error: JDK $feature is not pinned in tests/abi/openjdk-releases.tsv" >&2
  exit 2
fi

command -v xsltproc >/dev/null || {
  echo "error: xsltproc is required for the pinned source ABI proof" >&2
  exit 2
}

scripts/fetch-openjdk-abi-matrix.sh "$feature"
source_root="${OPENJDK_ABI_SOURCE_ROOT:-$ROOT/target/openjdk-abi-sources}/jdk$feature"
include="$ROOT/target/abi-conformance/jdk$feature/include"
mkdir -p "$include/linux"

xsltproc --stringparam majorversion "$feature" \
  "$source_root/src/hotspot/share/prims/jvmtiH.xsl" \
  "$source_root/src/hotspot/share/prims/jvmti.xml" \
  > "$include/jvmti.h"
cp "$source_root/src/java.base/share/native/include/jni.h" "$include/jni.h"
cp "$source_root/src/java.base/unix/native/include/jni_md.h" "$include/linux/jni_md.h"

JVMTI_ABI_FEATURE="$feature" \
JVMTI_ABI_INCLUDE_DIR="$include" \
JVMTI_ABI_PLATFORM_INCLUDE_DIR="$include/linux" \
  cargo +1.85.0 test --locked --test abi_conformance -- --nocapture

JVMTI_HEADER="$include/jvmti.h" \
JNI_HEADER="$include/jni.h" \
JNI_PLATFORM_INCLUDE="$include/linux" \
  scripts/check-raw-header-inventory.sh

JVMTI_HEADER="$include/jvmti.h" \
JNI_HEADER="$include/jni.h" \
JNI_PLATFORM_INCLUDE="$include/linux" \
  scripts/check-raw-signatures.sh

JVMTI_HEADER="$include/jvmti.h" \
JNI_HEADER="$include/jni.h" \
JNI_PLATFORM_INCLUDE="$include/linux" \
  scripts/check-raw-record-layouts.sh
