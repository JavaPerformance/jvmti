#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
manifest="$repo_root/tests/abi/openjdk-releases.tsv"
destination="${OPENJDK_ABI_SOURCE_ROOT:-$repo_root/target/openjdk-abi-sources}"
base_url="https://raw.githubusercontent.com/openjdk/jdk"
requested=("$@")

command -v curl >/dev/null || {
  echo "curl is required to fetch the pinned OpenJDK ABI sources" >&2
  exit 1
}

while IFS=$'\t' read -r feature tag commit; do
  [[ -n "$feature" && "$feature" != \#* ]] || continue
  if ((${#requested[@]})); then
    selected=false
    for wanted in "${requested[@]}"; do
      [[ "$feature" == "$wanted" ]] && selected=true
    done
    [[ "$selected" == true ]] || continue
  fi
  source_root="$destination/jdk$feature"
  if [[ "$feature" == 9 ]]; then
    mappings=(
      "hotspot/src/share/vm/prims/jvmti.xml:src/hotspot/share/prims/jvmti.xml"
      "hotspot/src/share/vm/prims/jvmtiH.xsl:src/hotspot/share/prims/jvmtiH.xsl"
      "hotspot/src/share/vm/prims/jvmtiLib.xsl:src/hotspot/share/prims/jvmtiLib.xsl"
      "jdk/src/java.base/share/native/include/jni.h:src/java.base/share/native/include/jni.h"
      "jdk/src/java.base/unix/native/include/jni_md.h:src/java.base/unix/native/include/jni_md.h"
    )
  else
    mappings=(
      "src/hotspot/share/prims/jvmti.xml:src/hotspot/share/prims/jvmti.xml"
      "src/hotspot/share/prims/jvmtiH.xsl:src/hotspot/share/prims/jvmtiH.xsl"
      "src/hotspot/share/prims/jvmtiLib.xsl:src/hotspot/share/prims/jvmtiLib.xsl"
      "src/java.base/share/native/include/jni.h:src/java.base/share/native/include/jni.h"
      "src/java.base/unix/native/include/jni_md.h:src/java.base/unix/native/include/jni_md.h"
    )
  fi
  marker="$source_root/.openjdk-pinned-source"
  expected_marker=$(printf '%s\t%s\t%s' "$feature" "$tag" "$commit")
  cached_marker=
  if [[ -f "$marker" ]]; then
    cached_marker=$(cat "$marker")
  fi
  for mapping in "${mappings[@]}"; do
    upstream=${mapping%%:*}
    relative=${mapping#*:}
    output="$source_root/$relative"
    if [[ "$cached_marker" == "$expected_marker" && -s "$output" ]]; then
      continue
    fi
    mkdir -p "$(dirname -- "$output")"
    printf 'fetch JDK %s (%s) %s\n' "$feature" "$tag" "$upstream"
    curl --fail --location --silent --show-error --retry 3 \
      "$base_url/$commit/$upstream" \
      --output "$output"
  done
  marker_tmp="$marker.$$"
  printf '%s\n' "$expected_marker" > "$marker_tmp"
  mv "$marker_tmp" "$marker"
done < "$manifest"

printf 'Pinned OpenJDK ABI sources are under %s\n' "$destination"
