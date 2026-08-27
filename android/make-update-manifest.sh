#!/data/data/com.termux/files/usr/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APK_URL=${1:-}
APK_PATH=${2:-"$ROOT_DIR/target/android-termux/rustdl.apk"}
OUTPUT=${3:-"$ROOT_DIR/target/android-termux/latest.json"}

if [ -z "$APK_URL" ]; then
    echo "usage: $0 https://host/path/rustdl.apk [apk-path] [output.json]" >&2
    exit 2
fi
case "$APK_URL" in
    https://*) ;;
    *)
        echo "APK URL must use HTTPS" >&2
        exit 2
        ;;
esac
if [ ! -f "$APK_PATH" ]; then
    echo "APK not found: $APK_PATH" >&2
    exit 2
fi

PACKAGE_LINE=$(aapt2 dump badging "$APK_PATH" | sed -n '1p')
PACKAGE_NAME=$(printf '%s\n' "$PACKAGE_LINE" | sed -n "s/^package: name='\([^']*\)'.*/\1/p")
VERSION_CODE=$(printf '%s\n' "$PACKAGE_LINE" | sed -n "s/.*versionCode='\([^']*\)'.*/\1/p")
VERSION_NAME=$(printf '%s\n' "$PACKAGE_LINE" | sed -n "s/.*versionName='\([^']*\)'.*/\1/p")

if [ "$PACKAGE_NAME" != app.rustdl ] || [ -z "$VERSION_CODE" ] || [ -z "$VERSION_NAME" ]; then
    echo "APK metadata is not a valid RustDL release" >&2
    exit 2
fi

SHA256=$(sha256sum "$APK_PATH" | cut -d ' ' -f 1)
SIZE_BYTES=$(stat -c %s "$APK_PATH")
mkdir -p "$(dirname -- "$OUTPUT")"
printf '{\n  "version_code": %s,\n  "version_name": "%s",\n  "apk_url": "%s",\n  "sha256": "%s",\n  "size_bytes": %s\n}\n' \
    "$VERSION_CODE" "$VERSION_NAME" "$APK_URL" "$SHA256" "$SIZE_BYTES" >"$OUTPUT"

echo "$OUTPUT"
