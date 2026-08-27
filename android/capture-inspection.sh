#!/data/data/com.termux/files/usr/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
SCREEN=${1:-home}
OUTPUT=${2:-"$ROOT_DIR/target/inspection-$SCREEN.png"}
SERIAL=${3:-${ADB_SERIAL:-}}
FORWARD_PORT=18765
PART="$OUTPUT.part"

case "$SCREEN" in
    home|result|player) ;;
    *)
        echo "usage: $0 [home|result|player] [output.png] [adb-serial]" >&2
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

close_inspection() {
    run_adb shell am start -W --activity-no-animation \
        -n app.rustdl/.InspectionCloserActivity >/dev/null 2>&1 || true
}

cleanup() {
    close_inspection
    run_adb forward --remove "tcp:$FORWARD_PORT" >/dev/null 2>&1 || true
    rm -f "$PART"
}
trap cleanup EXIT HUP INT TERM

mkdir -p "$(dirname -- "$OUTPUT")"
rm -f "$PART"
close_inspection
run_adb forward "tcp:$FORWARD_PORT" tcp:37659 >/dev/null
run_adb shell am start -W --activity-no-animation \
    -a app.rustdl.action.CAPTURE_INSPECTION \
    --es app.rustdl.extra.SCREEN "$SCREEN" \
    -n app.rustdl/.InspectionActivity >/dev/null

attempt=0
while [ "$attempt" -lt 60 ]; do
    if curl --fail --silent --show-error \
        "http://127.0.0.1:$FORWARD_PORT/__inspect/capture.png" \
        -o "$PART" 2>/dev/null; then
        mv -f "$PART" "$OUTPUT"
        trap - EXIT HUP INT TERM
        close_inspection
        run_adb forward --remove "tcp:$FORWARD_PORT" >/dev/null 2>&1 || true
        echo "$OUTPUT"
        echo "Synthetic WebView only; the Android display was not captured."
        exit 0
    fi
    attempt=$((attempt + 1))
    sleep 0.25
done

echo "RustDL did not produce the synthetic inspection render in time" >&2
exit 1
