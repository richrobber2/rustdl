#!/data/data/com.termux/files/usr/bin/sh
set -u

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
MODE=${1:-normal}
SERIAL=${2:-${ADB_SERIAL:-}}

case "$MODE" in
    normal|home|result|player) ;;
    *)
        echo "usage: $0 [normal|home|result|player] [adb-serial]" >&2
        exit 2
        ;;
esac

run_adb() {
    if [ -n "$SERIAL" ]; then
        adb -s "$SERIAL" "$@"
    else
        adb "$@"
    fi
}

snapshot() {
    {
        sha256sum "$ROOT_DIR/Cargo.toml"
        find "$ROOT_DIR/src" "$ROOT_DIR/android" -type f \
            \( -name '*.rs' -o -name '*.java' -o -name '*.xml' -o -name '*.sh' \) \
            -exec sha256sum {} +
    } | sort | sha256sum | cut -d ' ' -f 1
}

launch_selected_mode() {
    run_adb shell am force-stop app.rustdl
    if [ "$MODE" = normal ]; then
        run_adb shell am start -W --activity-no-animation \
            -a android.intent.action.MAIN \
            -c android.intent.category.LAUNCHER \
            -n app.rustdl/.MainActivity >/dev/null
    else
        run_adb shell am start -W --activity-no-animation \
            -a app.rustdl.action.INSPECT \
            --es app.rustdl.extra.SCREEN "$MODE" \
            -n app.rustdl/.InspectionActivity >/dev/null
    fi
}

rebuild_and_reload() {
    echo "Building RustDL…"
    if ! sh "$ROOT_DIR/android/build-termux.sh"; then
        echo "Build failed; the installed app was left unchanged." >&2
        return
    fi
    if ! run_adb install -r "$ROOT_DIR/target/android-termux/rustdl.apk"; then
        echo "Install failed; the installed app was left unchanged." >&2
        return
    fi
    launch_selected_mode
    echo "Reloaded $MODE mode."
}

rebuild_and_reload
LAST_SNAPSHOT=$(snapshot)
echo "Watching Rust, Java, XML, and Android scripts. Press Ctrl+C to stop."

while :; do
    sleep 0.75
    NEXT_SNAPSHOT=$(snapshot)
    if [ "$NEXT_SNAPSHOT" = "$LAST_SNAPSHOT" ]; then
        continue
    fi
    LAST_SNAPSHOT=$NEXT_SNAPSHOT
    rebuild_and_reload
done
