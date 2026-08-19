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
