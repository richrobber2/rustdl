#!/data/data/com.termux/files/usr/bin/sh
set -eu

SCREEN=${1:-home}
SERIAL=${2:-${ADB_SERIAL:-}}

case "$SCREEN" in
    home|result|player) ;;
    *)
        echo "usage: $0 [home|result|player] [adb-serial]" >&2
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

run_adb shell am start -W \
    -a app.rustdl.action.INSPECT \
    --es app.rustdl.extra.SCREEN "$SCREEN" \
    -n app.rustdl/.InspectionActivity

echo "RustDL inspection mode: $SCREEN"
echo "No screenshots were captured."
