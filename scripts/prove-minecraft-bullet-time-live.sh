#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work="$root/target/minecraft-bullet-time-live"
source_root="$work/src"
classes="$work/classes"

command -v java >/dev/null || {
    echo "java is required" >&2
    exit 2
}
command -v javac >/dev/null || {
    echo "javac is required" >&2
    exit 2
}

mkdir -p "$source_root/net/minecraft/client" "$classes"

cat >"$source_root/net/minecraft/client/Minecraft.java" <<'JAVA'
package net.minecraft.client;

public final class Minecraft {
    public void runTick(boolean renderLevel) {
        if (!renderLevel) throw new AssertionError("probe always renders");
    }
}
JAVA

cat >"$source_root/net/minecraft/client/MouseHandler.java" <<'JAVA'
package net.minecraft.client;

public final class MouseHandler {
    public double lastVertical = Double.NaN;

    public void onScroll(long window, double horizontal, double vertical) {
        lastVertical = vertical;
    }
}
JAVA

cat >"$source_root/net/minecraft/client/KeyboardHandler.java" <<'JAVA'
package net.minecraft.client;

public final class KeyboardHandler {
    public void keyPress(long window, int key, int scanCode, int action, int modifiers) {
        // The agent reads these arguments at bytecode location zero.
    }
}
JAVA

cat >"$source_root/BulletTimeProbe.java" <<'JAVA'
import net.minecraft.client.KeyboardHandler;
import net.minecraft.client.Minecraft;
import net.minecraft.client.MouseHandler;

public final class BulletTimeProbe {
    private static final int GLFW_RELEASE = 0;
    private static final int GLFW_PRESS = 1;
    private static final int GLFW_KEY_F8 = 297;

    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        KeyboardHandler keyboard = new KeyboardHandler();
        MouseHandler mouse = new MouseHandler();
        Minecraft minecraft = new Minecraft();

        mouse.onScroll(1L, 0.0, 2.0);
        require(mouse.lastVertical == 2.0, "unarmed wheel must pass through");

        keyboard.keyPress(1L, GLFW_KEY_F8, 0, GLFW_PRESS, 0);
        mouse.onScroll(1L, 0.0, -1.0);
        require(mouse.lastVertical == 0.0, "F8+wheel must be consumed");

        long started = System.nanoTime();
        minecraft.runTick(true);
        long elapsedMs = (System.nanoTime() - started) / 1_000_000L;
        require(elapsedMs >= 7L, "armed wheel must add about 10ms tick delay: " + elapsedMs);

        keyboard.keyPress(1L, GLFW_KEY_F8, 0, GLFW_RELEASE, 0);
        mouse.onScroll(1L, 0.0, 3.0);
        require(mouse.lastVertical == 3.0, "released F8 must restore wheel passthrough");

        System.out.println("bullet-time-live-proof: pass, delayed_tick_ms=" + elapsedMs);
    }
}
JAVA

cat >"$source_root/BulletTimeAttachProbe.java" <<'JAVA'
import java.io.File;
import java.io.FileWriter;
import java.lang.management.ManagementFactory;
import net.minecraft.client.KeyboardHandler;
import net.minecraft.client.Minecraft;
import net.minecraft.client.MouseHandler;

public final class BulletTimeAttachProbe {
    private static final int GLFW_PRESS = 1;
    private static final int GLFW_KEY_F8 = 297;

    private static void require(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws Exception {
        // Construct every target before attach so CLASS_PREPARE cannot rescue
        // an agent that forgot to inventory already-loaded classes.
        KeyboardHandler keyboard = new KeyboardHandler();
        MouseHandler mouse = new MouseHandler();
        Minecraft minecraft = new Minecraft();

        String pid = ManagementFactory.getRuntimeMXBean().getName().split("@")[0];
        FileWriter pidOut = new FileWriter(args[0]);
        pidOut.write(pid + "\n");
        pidOut.close();

        File continueFile = new File(args[1]);
        long deadline = System.nanoTime() + 30_000_000_000L;
        while (!continueFile.isFile() && System.nanoTime() < deadline) {
            Thread.sleep(10L);
        }
        require(continueFile.isFile(), "attach driver did not release target");

        keyboard.keyPress(1L, GLFW_KEY_F8, 0, GLFW_PRESS, 0);
        mouse.onScroll(1L, 0.0, -1.0);
        require(mouse.lastVertical == 0.0, "late-attached agent must consume F8+wheel");

        long started = System.nanoTime();
        minecraft.runTick(true);
        long elapsedMs = (System.nanoTime() - started) / 1_000_000L;
        require(elapsedMs >= 7L, "late-attached agent must delay tick: " + elapsedMs);
        System.out.println("bullet-time-attach-proof: pass, delayed_tick_ms=" + elapsedMs);
    }
}
JAVA

cat >"$source_root/AttachBulletTime.java" <<'JAVA'
import com.sun.tools.attach.VirtualMachine;
import java.io.FileWriter;

public final class AttachBulletTime {
    public static void main(String[] args) throws Exception {
        VirtualMachine vm = VirtualMachine.attach(args[0]);
        try {
            vm.loadAgentPath(args[1], "");
        } finally {
            vm.detach();
        }
        FileWriter release = new FileWriter(args[2]);
        release.write("attached\n");
        release.close();
    }
}
JAVA

javac -d "$classes" \
    "$source_root/BulletTimeProbe.java" \
    "$source_root/net/minecraft/client/KeyboardHandler.java" \
    "$source_root/net/minecraft/client/Minecraft.java" \
    "$source_root/net/minecraft/client/MouseHandler.java"

cargo +1.85.0 build --locked --release --example minecraft_bullet_time --all-features

case "$(uname -s)" in
    Darwin) agent="$root/target/release/examples/libminecraft_bullet_time.dylib" ;;
    Linux) agent="$root/target/release/examples/libminecraft_bullet_time.so" ;;
    *)
        echo "live shell proof supports Unix hosts; the agent example also builds on Windows" >&2
        exit 2
        ;;
esac

java -agentpath:"$agent" -cp "$classes" BulletTimeProbe

detect_active_jdk_home() {
    local candidate=${JAVA_HOME:-}
    if [[ -n "$candidate" && -x "$candidate/bin/java" && -x "$candidate/bin/javac" ]]; then
        printf '%s\n' "$candidate"
        return
    fi

    candidate=$(java -XshowSettings:properties -version 2>&1 \
        | sed -n 's/^[[:space:]]*java\.home = //p')
    if [[ -n "$candidate" && -x "$candidate/bin/java" && -x "$candidate/bin/javac" ]]; then
        printf '%s\n' "$candidate"
        return
    fi

    cat >&2 <<'EOF'
could not identify an active JDK containing both bin/java and bin/javac
set JAVA_HOME or pass one or more explicit JDK homes to this script
EOF
    return 2
}

declare -a attach_jdks
if (($#)); then
    attach_jdks=("$@")
else
    attach_jdks=("$(detect_active_jdk_home)")
fi

target_pid=
cleanup() {
    if [[ -n "$target_pid" ]]; then
        kill "$target_pid" 2>/dev/null || true
        wait "$target_pid" 2>/dev/null || true
    fi
}
trap cleanup EXIT

for home in "${attach_jdks[@]}"; do
    [[ -x "$home/bin/java" && -x "$home/bin/javac" ]] || {
        echo "missing Java runtime/compiler: $home" >&2
        exit 2
    }
    version=$($home/bin/java -version 2>&1 | head -1 | sed -E 's/.*version "([^"]+)".*/\1/')
    if [[ "$version" == 1.8.* ]]; then
        feature=8
    else
        feature=${version%%.*}
        feature=${feature%%-*}
        feature=${feature%%+*}
    fi

    scan_log="$work/vm-init-scan-$feature.log"
    scan_options='tick_class=Ljava/lang/Object;,tick_method=toString,tick_signature=()Ljava/lang/String;,scroll_class=Lmissing/Mouse;,keyboard_class=Lmissing/Keyboard;'
    "$home/bin/java" -agentpath:"$agent"="$scan_options" -version >"$scan_log" 2>&1
    grep -q 'installed tick breakpoint source=vm-init class=Ljava/lang/Object;' "$scan_log"
    echo "JDK $feature VMInit already-loaded-class scan proof: pass"

    attach_classes="$work/attach-classes-$feature"
    pid_file="$work/attach-$feature.pid"
    continue_file="$work/attach-$feature.continue"
    target_log="$work/attach-$feature.target.log"
    driver_log="$work/attach-$feature.driver.log"
    rm -rf "$attach_classes"
    rm -f "$pid_file" "$continue_file" "$target_log" "$driver_log"
    mkdir -p "$attach_classes"

    compiler_options=()
    target_options=()
    attach_options=()
    classpath="$attach_classes"
    if ((feature == 8)); then
        compiler_options+=("-cp" "$home/lib/tools.jar")
        classpath="$classpath:$home/lib/tools.jar"
    else
        compiler_options+=("--add-modules" "jdk.attach")
        attach_options+=("--add-modules" "jdk.attach")
        if ((feature >= 21)); then
            target_options+=("-XX:+EnableDynamicAgentLoading")
        fi
    fi
    "$home/bin/javac" "${compiler_options[@]}" -d "$attach_classes" \
        "$source_root/AttachBulletTime.java" \
        "$source_root/BulletTimeAttachProbe.java" \
        "$source_root/net/minecraft/client/KeyboardHandler.java" \
        "$source_root/net/minecraft/client/Minecraft.java" \
        "$source_root/net/minecraft/client/MouseHandler.java"

    "$home/bin/java" "${target_options[@]}" -cp "$classpath" \
        BulletTimeAttachProbe "$pid_file" "$continue_file" >"$target_log" 2>&1 &
    target_pid=$!
    for _ in $(seq 1 200); do
        [[ -s "$pid_file" ]] && break
        sleep 0.05
    done
    [[ -s "$pid_file" ]] || {
        echo "late-attach target did not publish a PID for JDK $feature" >&2
        cat "$target_log" >&2 || true
        exit 1
    }
    java_pid=$(tr -d '\r\n' <"$pid_file")
    set +e
    "$home/bin/java" "${attach_options[@]}" -cp "$classpath" AttachBulletTime \
        "$java_pid" "$agent" "$continue_file" >"$driver_log" 2>&1
    attach_status=$?
    set -e
    if ((attach_status == 0)); then
        wait "$target_pid"
        target_pid=
        grep -q 'bullet-time-attach-proof: pass' "$target_log"
        grep -q 'installed tick breakpoint source=attach' "$target_log"
        grep -q 'installed scroll breakpoint source=attach' "$target_log"
        grep -q 'installed keyboard breakpoint source=attach' "$target_log"
        grep -q 'loaded-class scan source=attach' "$target_log"
        echo "JDK $feature late-attach bullet-time proof: pass"
        tail -n 8 "$target_log"
    else
        grep -q 'required capabilities unavailable mode=attach' "$target_log" || {
            echo "unexpected late-attach failure on JDK $feature" >&2
            cat "$target_log" "$driver_log" >&2
            exit 1
        }
        cleanup
        target_pid=
        echo "JDK $feature late attach rejected cleanly: required live capabilities unavailable"
        tail -n 2 "$target_log"
    fi
done
