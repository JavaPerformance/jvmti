#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
manifest="$repo_root/tests/abi/openjdk-releases.tsv"
destination="${OPENJDK_ABI_SOURCE_ROOT:-$repo_root/target/openjdk-abi-sources}"
base_url="https://raw.githubusercontent.com/openjdk/jdk"

command -v curl >/dev/null || {
  echo "curl is required to fetch the pinned OpenJDK ABI sources" >&2
  exit 1
}

while IFS=$'\t' read -r feature tag commit; do
  [[ -n "$feature" && "$feature" != \#* ]] || continue
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
  for mapping in "${mappings[@]}"; do
    upstream=${mapping%%:*}
    relative=${mapping#*:}
    output="$source_root/$relative"
    [[ -s "$output" ]] && continue
    mkdir -p "$(dirname -- "$output")"
    printf 'fetch JDK %s (%s) %s\n' "$feature" "$tag" "$upstream"
    curl --fail --location --silent --show-error --retry 3 \
      "$base_url/$commit/$upstream" \
      --output "$output"
  done
done < "$manifest"

printf 'Pinned OpenJDK ABI sources are under %s\n' "$destination"
