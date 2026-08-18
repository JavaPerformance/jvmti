#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

java_home=${JAVA_HOME:-/etc/java-config-2/current-system-vm}
java="$java_home/bin/java"
javac="$java_home/bin/javac"
warmup=${CALLBACK_ALLOC_AUDIT_WARMUP:-2000000}
iterations=${CALLBACK_ALLOC_AUDIT_ITERATIONS:-20000000}
threads=${CALLBACK_ALLOC_AUDIT_THREADS:-1}

[[ -x "$java" && -x "$javac" ]] || {
    echo "JAVA_HOME must contain executable java and javac: $java_home" >&2
    exit 2
}
for value in "$warmup" "$iterations" "$threads"; do
    [[ "$value" =~ ^[1-9][0-9]*$ ]] || {
        echo "callback allocation audit counts must be positive integers" >&2
        exit 2
    }
done

run_id=$(date -u +%Y%m%dT%H%M%SZ)-$$
work="$root/target/callback-allocation-audit/$run_id"
classes="$work/classes"
mkdir -p "$classes"

cargo build --release --example callback_allocation_audit
"$javac" -Xlint:all -Werror -d "$classes" benchmarks/callbacks/CallbackDispatchBench.java

case "$(uname -s)" in
    Linux) library_suffix=so ;;
    Darwin) library_suffix=dylib ;;
    *) echo "live callback allocation proof currently supports Linux and macOS" >&2; exit 2 ;;
esac

agent="$root/target/release/examples/libcallback_allocation_audit.$library_suffix"
output=$(
    "$java" \
        -Xms256m -Xmx256m \
        -XX:-TieredCompilation -Xbatch \
        -XX:CompileCommand=quiet \
        -XX:CompileCommand=dontinline,CallbackDispatchBench.profiledLeaf \
        "-agentpath:$agent" \
        -cp "$classes" CallbackDispatchBench "$warmup" "$iterations" "$threads" 2>&1
)
printf '%s\n' "$output" | tee "$work/output.log"

audit=$(sed -n 's/^callback_allocation_audit //p' <<<"$output" | tail -n 1)
[[ -n "$audit" ]] || {
    echo "callback allocation audit agent emitted no result" >&2
    exit 1
}

field() {
    local key=$1
    tr ' ' '\n' <<<"$audit" | sed -n "s/^${key}=//p"
}

callbacks=$(field callbacks)
allocations=$(field allocations)
allocated_bytes=$(field allocated_bytes)
reallocations=$(field reallocations)
deallocations=$(field deallocations)

[[ "$callbacks" =~ ^[1-9][0-9]*$ ]] || {
    echo "callback allocation audit observed no method-entry callbacks" >&2
    exit 1
}
if [[ "$allocations" != 0 || "$allocated_bytes" != 0 || "$reallocations" != 0 || "$deallocations" != 0 ]]; then
    echo "normal callback dispatch touched the Rust allocator" >&2
    exit 1
fi

printf 'callback_allocation_proof=pass callbacks=%s threads=%s output=%s\n' \
    "$callbacks" "$threads" "$work/output.log"
