#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

if [[ $# -eq 0 ]]; then
    cat >&2 <<'EOF'
usage: scripts/check-classfile-corpus.sh JAVA_HOME [JAVA_HOME ...]

Parses every class in each supplied JDK's core runtime image. Scratch data is
kept under Cargo's target directory and removed after each runtime.
EOF
    exit 2
fi

TARGET_ROOT=${CARGO_TARGET_DIR:-$ROOT/target}
SCRATCH_ROOT="$TARGET_ROOT/classfile-corpus"
mkdir -p "$SCRATCH_ROOT"
export JVMTI_BENCH_SCRATCH="$SCRATCH_ROOT/archive"

# Respect the caller-selected toolchain (for example RUSTUP_TOOLCHAIN in CI)
# so the same corpus gate proves both the MSRV and current stable builds.
cargo build --locked --release --bin jar_parse_bench
BENCH="$TARGET_ROOT/release/jar_parse_bench"

for java_home in "$@"; do
    java_home=$(cd "$java_home" && pwd)
    label=$(basename "$java_home")
    echo "=== class-file corpus: $label ==="

    if [[ -f "$java_home/jre/lib/rt.jar" ]]; then
        JAVA_HOME="$java_home" "$BENCH" "$java_home/jre/lib/rt.jar"
        continue
    fi

    if [[ -x "$java_home/bin/jimage" && -f "$java_home/lib/modules" ]]; then
        image_dir="$SCRATCH_ROOT/image-$label-$$"
        mkdir "$image_dir"
        cleanup() {
            find "$image_dir" -depth -mindepth 1 -delete 2>/dev/null || true
            rmdir "$image_dir" 2>/dev/null || true
        }
        trap cleanup EXIT
        "$java_home/bin/jimage" extract --dir "$image_dir" "$java_home/lib/modules"
        "$BENCH" "$image_dir"
        cleanup
        trap - EXIT
        continue
    fi

    if [[ -f "$java_home/jmods/java.base.jmod" ]]; then
        JAVA_HOME="$java_home" "$BENCH" "$java_home/jmods/java.base.jmod"
        continue
    fi

    echo "unsupported JDK image layout: $java_home" >&2
    exit 1
done
