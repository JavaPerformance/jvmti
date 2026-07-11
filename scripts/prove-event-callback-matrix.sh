#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"

cargo test --test jvmti_event_abi
cargo build --release --example event_abi_smoke

case "$(uname -s)" in
    Linux) agent="$repo_root/target/release/examples/libevent_abi_smoke.so" ;;
    Darwin) agent="$repo_root/target/release/examples/libevent_abi_smoke.dylib" ;;
    *)
        echo "live event-callback matrix is supported on Linux and macOS" >&2
        exit 2
        ;;
esac

if (($#)); then
    java_homes=("$@")
else
    java_homes=()
    if [[ -n "${JAVA_HOME:-}" ]]; then
        java_homes+=("$JAVA_HOME")
    fi
    while IFS= read -r home; do
        java_homes+=("$home")
    done < <(find /opt -maxdepth 1 -type d -name 'openjdk-bin-*' -print 2>/dev/null | sort)
fi

if ((${#java_homes[@]} == 0)); then
    echo "no JDKs found; pass JAVA_HOME directories as arguments" >&2
    exit 2
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

cat >"$tmp/EventAbiSmoke.java" <<'JAVA'
public final class EventAbiSmoke {
    public static void main(String[] args) throws Exception {
        for (int round = 0; round < 4; round++) {
            byte[][] pressure = new byte[32][];
            for (int index = 0; index < pressure.length; index++) {
                pressure[index] = new byte[64 * 1024];
            }
            System.gc();
            Thread.sleep(25L);
        }
        System.out.println("EventAbiSmoke Java workload complete");
    }
}
JAVA

passed=0
seen_homes=()
for home in "${java_homes[@]}"; do
    [[ -x "$home/bin/java" && -x "$home/bin/javac" ]] || continue
    canonical=$(cd "$home" && pwd -P)
    duplicate=false
    for seen_home in "${seen_homes[@]}"; do
        if [[ "$seen_home" == "$canonical" ]]; then
            duplicate=true
            break
        fi
    done
    if $duplicate; then
        continue
    fi
    seen_homes+=("$canonical")

    classes="$tmp/classes-$passed"
    mkdir -p "$classes"
    "$home/bin/javac" -d "$classes" "$tmp/EventAbiSmoke.java"

    version=$("$home/bin/java" -version 2>&1 | head -n 1)
    echo "==> $version"
    output=$("$home/bin/java" \
        -Xverify:all \
        -Xms32m \
        -Xmx32m \
        "-agentpath:$agent" \
        -cp "$classes" \
        EventAbiSmoke 2>&1)
    printf '%s\n' "$output"

    marker=$(grep -Eo '\[event-abi\] PASS method_entries=[0-9]+ gc_starts=[0-9]+ gc_finishes=[0-9]+' <<<"$output" | tail -n 1)
    [[ -n "$marker" ]] || {
        echo "missing event ABI success marker for $home" >&2
        exit 1
    }
    method_entries=$(sed -E 's/.*method_entries=([0-9]+).*/\1/' <<<"$marker")
    gc_starts=$(sed -E 's/.*gc_starts=([0-9]+).*/\1/' <<<"$marker")
    gc_finishes=$(sed -E 's/.*gc_finishes=([0-9]+).*/\1/' <<<"$marker")
    ((method_entries > 0 && gc_starts > 0 && gc_finishes > 0)) || {
        echo "one or more callbacks were not delivered for $home: $marker" >&2
        exit 1
    }
    ((passed += 1))
done

((passed > 0)) || {
    echo "no runnable JDKs found" >&2
    exit 2
}

echo "JVMTI event callback ABI matrix passed ($passed JDKs)"
