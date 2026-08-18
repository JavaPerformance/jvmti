#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

declare -a jdks=(
  "8:/opt/openjdk-bin-8.492_p09"
  "11:/opt/openjdk-bin-11.0.31_p11"
  "17:/opt/openjdk-bin-17.0.19_p10"
  "21:/opt/openjdk-bin-21.0.11_p10"
  "25:/opt/openjdk-bin-25.0.3_p9"
  "27:/opt/openjdk-bin-27_alpha20"
)

all_releases=false
if [[ "${1:-}" == "--all-releases" ]]; then
  all_releases=true
  shift
  jdks=("8:/opt/openjdk-bin-8.492_p09")
fi

if (($#)); then
  jdks=("$@")
fi

for entry in "${jdks[@]}"; do
  feature=${entry%%:*}
  home=${entry#*:}
  include="$home/include"
  platform_include="$include/linux"
  if [[ ! -f "$include/jvmti.h" || ! -f "$include/jni.h" ]]; then
    printf 'skip JDK %s: headers not found under %s\n' "$feature" "$include" >&2
    continue
  fi
  printf '\n== OpenJDK %s ABI: %s ==\n' "$feature" "$home"
  JVMTI_ABI_FEATURE="$feature" \
  JVMTI_ABI_INCLUDE_DIR="$include" \
  JVMTI_ABI_PLATFORM_INCLUDE_DIR="$platform_include" \
    cargo test --test abi_conformance -- --nocapture
done

check_source_jdk() {
  local feature=$1
  local source=$2
  if [[ ! -f "$source/src/hotspot/share/prims/jvmti.xml" ]]; then
    printf 'skip JDK %s: source not found under %s\n' "$feature" "$source" >&2
    return
  fi

  local generated="$repo_root/target/abi-conformance/jdk${feature}/include"
  mkdir -p "$generated/linux"
  xsltproc --stringparam majorversion "$feature" \
    "$source/src/hotspot/share/prims/jvmtiH.xsl" \
    "$source/src/hotspot/share/prims/jvmti.xml" \
    > "$generated/jvmti.h"
  cp "$source/src/java.base/share/native/include/jni.h" "$generated/jni.h"
  cp "$source/src/java.base/unix/native/include/jni_md.h" "$generated/linux/jni_md.h"
  printf '\n== OpenJDK %s ABI: generated from %s ==\n' "$feature" "$source"
  JVMTI_ABI_FEATURE="$feature" \
  JVMTI_ABI_INCLUDE_DIR="$generated" \
  JVMTI_ABI_PLATFORM_INCLUDE_DIR="$generated/linux" \
    cargo test --test abi_conformance -- --nocapture
}

if [[ "$all_releases" == true ]]; then
  "$repo_root/scripts/fetch-openjdk-abi-matrix.sh"
  source_root="${OPENJDK_ABI_SOURCE_ROOT:-$repo_root/target/openjdk-abi-sources}"
  while IFS=$'\t' read -r feature _tag _commit; do
    [[ -n "$feature" && "$feature" != \#* ]] || continue
    check_source_jdk "$feature" "$source_root/jdk$feature"
  done < "$repo_root/tests/abi/openjdk-releases.tsv"
else
  check_source_jdk 26 "${JDK26_SOURCE:-/opt/jvmsrc/jdk26u-jdk-26-32}"
  check_source_jdk 28 "${JDK28_SOURCE:-/opt/jvmsrc/jdk28-openjdk-4555cf213717}"
fi
