#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

java_home=${JAVA_HOME:-/etc/java-config-2/current-system-vm}
java="$java_home/bin/java"
javac="$java_home/bin/javac"
cc=${CC:-cc}
warmup=${CALLBACK_BENCH_WARMUP:-2000000}
iterations=${CALLBACK_BENCH_ITERATIONS:-20000000}
repetitions=${CALLBACK_BENCH_REPETITIONS:-5}
threads=${CALLBACK_BENCH_THREADS:-1}

[[ -x "$java" && -x "$javac" ]] || {
    echo "JAVA_HOME must contain executable java and javac: $java_home" >&2
    exit 2
}
command -v "$cc" >/dev/null || {
    echo "C compiler not found: $cc" >&2
    exit 2
}
[[ "$warmup" =~ ^[1-9][0-9]*$ && "$iterations" =~ ^[1-9][0-9]*$ ]] || {
    echo "warmup and measured iteration counts must be positive integers" >&2
    exit 2
}
[[ "$repetitions" =~ ^[1-9][0-9]*$ ]] || {
    echo "CALLBACK_BENCH_REPETITIONS must be a positive integer" >&2
    exit 2
}
[[ "$threads" =~ ^[1-9][0-9]*$ ]] || {
    echo "CALLBACK_BENCH_THREADS must be a positive integer" >&2
    exit 2
}

run_id=$(date -u +%Y%m%dT%H%M%SZ)-$$
work="$root/target/callback-dispatch-bench/$run_id"
classes="$work/classes"
mkdir -p "$classes"

cargo build --release \
    --example callback_idle_bench \
    --example callback_noop_bench \
    --example callback_counter_bench
"$javac" -Xlint:all -Werror -d "$classes" benchmarks/callbacks/CallbackDispatchBench.java

case "$(uname -s)" in
    Linux)
        library_suffix=so
        "$cc" -O3 -Wall -Wextra -Werror -fPIC -shared \
            -I"$java_home/include" -I"$java_home/include/linux" \
            benchmarks/callbacks/raw_noop_agent.c \
            -o "$work/libcallback_raw_noop_bench.$library_suffix"
        ;;
    Darwin)
        library_suffix=dylib
        "$cc" -O3 -Wall -Wextra -Werror -fPIC -dynamiclib \
            -I"$java_home/include" -I"$java_home/include/darwin" \
            benchmarks/callbacks/raw_noop_agent.c \
            -o "$work/libcallback_raw_noop_bench.$library_suffix"
        ;;
    *)
        echo "live callback benchmark currently supports Linux and macOS" >&2
        exit 2
        ;;
esac

idle_agent="$root/target/release/examples/libcallback_idle_bench.$library_suffix"
noop_agent="$root/target/release/examples/libcallback_noop_bench.$library_suffix"
counter_agent="$root/target/release/examples/libcallback_counter_bench.$library_suffix"
raw_agent="$work/libcallback_raw_noop_bench.$library_suffix"
for library in "$idle_agent" "$noop_agent" "$counter_agent" "$raw_agent"; do
    [[ -f "$library" ]] || {
        echo "missing benchmark agent: $library" >&2
        exit 1
    }
done

results="$work/results.tsv"
printf 'variant\trepetition\telapsed_ns\tns_per_call\tcalls_per_second\tchecksum\tcallbacks\n' >"$results"

variants=(baseline rust_idle c_noop rust_noop rust_counter)
expected_checksum=

run_variant() {
    local variant=$1
    local repetition=$2
    local agent_option=()
    case "$variant" in
        baseline) ;;
        rust_idle) agent_option=("-agentpath:$idle_agent") ;;
        c_noop) agent_option=("-agentpath:$raw_agent") ;;
        rust_noop) agent_option=("-agentpath:$noop_agent") ;;
        rust_counter) agent_option=("-agentpath:$counter_agent") ;;
        *) echo "unknown variant: $variant" >&2; exit 2 ;;
    esac

    local output
    output=$("$java" \
        -Xms256m -Xmx256m \
        -XX:-TieredCompilation -Xbatch \
        -XX:CompileCommand=quiet \
        -XX:CompileCommand=dontinline,CallbackDispatchBench.profiledLeaf \
        "${agent_option[@]}" \
        -cp "$classes" CallbackDispatchBench "$warmup" "$iterations" "$threads" 2>&1)

    local elapsed ns_per_call calls_per_second checksum callbacks
    elapsed=$(sed -n 's/^elapsed_ns=//p' <<<"$output" | tail -n 1)
    ns_per_call=$(sed -n 's/^ns_per_call=//p' <<<"$output" | tail -n 1)
    calls_per_second=$(sed -n 's/^calls_per_second=//p' <<<"$output" | tail -n 1)
    checksum=$(sed -n 's/^checksum=//p' <<<"$output" | tail -n 1)
    callbacks=$(sed -n 's/^callback_bench_agent=rust_counter callbacks=//p' <<<"$output" | tail -n 1)
    callbacks=${callbacks:-0}

    [[ -n "$elapsed" && -n "$ns_per_call" && -n "$calls_per_second" && -n "$checksum" ]] || {
        printf '%s\n' "$output" >&2
        echo "incomplete benchmark output for $variant" >&2
        exit 1
    }
    if [[ -z "$expected_checksum" ]]; then
        expected_checksum=$checksum
    elif [[ "$checksum" != "$expected_checksum" ]]; then
        echo "checksum mismatch for $variant: $checksum != $expected_checksum" >&2
        exit 1
    fi

    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$variant" "$repetition" "$elapsed" "$ns_per_call" \
        "$calls_per_second" "$checksum" "$callbacks" >>"$results"
    printf 'variant=%s repetition=%s ns_per_call=%s calls_per_second=%s callbacks=%s\n' \
        "$variant" "$repetition" "$ns_per_call" "$calls_per_second" "$callbacks"
}

for ((repetition = 1; repetition <= repetitions; repetition++)); do
    offset=$(( (repetition - 1) % ${#variants[@]} ))
    for ((index = 0; index < ${#variants[@]}; index++)); do
        variant=${variants[$(( (index + offset) % ${#variants[@]} ))]}
        run_variant "$variant" "$repetition"
    done
done

values_for() {
    local variant=$1
    local column=$2
    awk -F'\t' -v variant="$variant" -v column="$column" \
        'NR > 1 && $1 == variant { print $column }' "$results" | sort -n
}

median_for() {
    values_for "$1" "$2" | awk '{ values[NR] = $1 } END {
        if (NR % 2) print values[(NR + 1) / 2];
        else printf "%.3f\n", (values[NR / 2] + values[NR / 2 + 1]) / 2;
    }'
}

minimum_for() {
    values_for "$1" "$2" | head -n 1
}

maximum_for() {
    values_for "$1" "$2" | tail -n 1
}

baseline_median=$(median_for baseline 4)
printf '\nbenchmark=callback_dispatch_summary\n'
printf 'java_home=%s\n' "$java_home"
printf 'java_version=%s\n' "$("$java" -version 2>&1 | head -n 1)"
printf 'rustc=%s\n' "$(rustc --version)"
printf 'warmup_iterations=%s\niterations_per_thread=%s\nthreads=%s\nrepetitions=%s\n' \
    "$warmup" "$iterations" "$threads" "$repetitions"
for variant in "${variants[@]}"; do
    median_ns=$(median_for "$variant" 4)
    minimum_ns=$(minimum_for "$variant" 4)
    maximum_ns=$(maximum_for "$variant" 4)
    median_rate=$(median_for "$variant" 5)
    slowdown=$(awk -v value="$median_ns" -v baseline="$baseline_median" \
        'BEGIN { printf "%.3f", value / baseline }')
    printf 'variant=%s median_ns_per_call=%s range_ns_per_call=%s..%s median_calls_per_second=%s slowdown_vs_baseline=%sx\n' \
        "$variant" "$median_ns" "$minimum_ns" "$maximum_ns" \
        "$median_rate" "$slowdown"
done

c_noop_median=$(median_for c_noop 4)
rust_noop_median=$(median_for rust_noop 4)
rust_counter_median=$(median_for rust_counter 4)
awk -v c="$c_noop_median" -v rust="$rust_noop_median" -v counter="$rust_counter_median" \
    'BEGIN {
        printf "rust_minus_c_noop_ns=%.3f\n", rust - c;
        printf "rust_dispatch_vs_c_noop=%.3fx\n", rust / c;
        printf "counter_minus_rust_noop_ns=%.3f\n", counter - rust;
    }'
printf 'results=%s\n' "$results"
