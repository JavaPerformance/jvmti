#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo +1.85.0 build --locked --release --example native_method_bind

case "$(uname -s)" in
    Darwin) agent="$root/target/release/examples/libnative_method_bind.dylib" ;;
    Linux) agent="$root/target/release/examples/libnative_method_bind.so" ;;
    *)
        echo "live shell proof supports Unix hosts; the agent example also builds on Windows" >&2
        exit 2
        ;;
esac

declare -a jdks=(
    "/opt/openjdk-bin-8.492_p09"
    "/opt/openjdk-bin-27_alpha20"
)
if (($#)); then
    jdks=("$@")
fi

for home in "${jdks[@]}"; do
    [[ -x "$home/bin/java" ]] || {
        echo "missing Java executable: $home/bin/java" >&2
        exit 2
    }
    version=$("$home/bin/java" -version 2>&1 | head -1)
    output=$("$home/bin/java" -agentpath:"$agent" -version 2>&1)
    bindings=$(sed -n 's/^\[native-bind\] bindings=\([0-9][0-9]*\)$/\1/p' <<<"$output")
    if [[ -z "$bindings" || "$bindings" == 0 ]]; then
        echo "native-method-bind live proof failed for $home" >&2
        printf '%s\n' "$output" >&2
        exit 1
    fi
    echo "$version: preserved and observed $bindings native bindings"
done
