#!/data/data/com.termux/files/usr/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
ANDROID_DIR="$ROOT_DIR/android"
BUILD_DIR="$ROOT_DIR/target/android-termux"
FRAMEWORK_RES=/system/framework/framework-res.apk
ANDROID_JAR=${ANDROID_JAR:-}

for TOOL in cargo javac d8 aapt2 jar keytool apksigner; do
    if ! command -v "$TOOL" >/dev/null 2>&1; then
        echo "Missing build tool: $TOOL. Run 'make setup' first." >&2
        exit 1
    fi
done
PACKAGE_VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n 1)
VERSION_NAME=${RUSTDL_VERSION_NAME:-$PACKAGE_VERSION}

if [ -n "${RUSTDL_VERSION_CODE:-}" ]; then
    VERSION_CODE=$RUSTDL_VERSION_CODE
else
    VERSION_CORE=${PACKAGE_VERSION%%-*}
    OLD_IFS=$IFS
    IFS=.
    set -- $VERSION_CORE
    IFS=$OLD_IFS
    VERSION_MAJOR=${1:-0}
    VERSION_MINOR=${2:-0}
    VERSION_PATCH=${3:-0}
    VERSION_CODE=$((VERSION_MAJOR * 1000000 + VERSION_MINOR * 1000 + VERSION_PATCH))
    if [ "$VERSION_CODE" -lt 1 ]; then
        VERSION_CODE=1
    fi
fi

case "$VERSION_CODE" in
    ''|*[!0-9]*)
        echo "RUSTDL_VERSION_CODE must be a positive integer" >&2
        exit 2
        ;;
esac
if [ "$VERSION_CODE" -lt 1 ]; then
    echo "RUSTDL_VERSION_CODE must be a positive integer" >&2
    exit 2
fi

if [ -z "$ANDROID_JAR" ]; then
    for CANDIDATE in \
        "$ANDROID_DIR/platform/android.jar" \
        "${ANDROID_SDK_ROOT:-/nonexistent}/platforms/android-35/android.jar" \
        "${ANDROID_HOME:-/nonexistent}/platforms/android-35/android.jar"
    do
        if [ -f "$CANDIDATE" ]; then
            ANDROID_JAR=$CANDIDATE
            break
        fi
    done
fi

if [ ! -f "$ANDROID_JAR" ]; then
    echo "android.jar not found; run 'make setup' or set ANDROID_JAR." >&2
    exit 1
fi

cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --release --lib

mkdir -p "$BUILD_DIR"
find "$BUILD_DIR" -mindepth 1 -delete
mkdir -p "$BUILD_DIR/classes" "$BUILD_DIR/dex" "$BUILD_DIR/compiled" \
    "$BUILD_DIR/apk/lib/arm64-v8a"

javac --release 8 -classpath "$ANDROID_JAR" \
    -d "$BUILD_DIR/classes" \
    "$ANDROID_DIR"/*.java

d8 --lib "$ANDROID_JAR" --output "$BUILD_DIR/dex" \
    $(find "$BUILD_DIR/classes" -name '*.class' -type f)

aapt2 compile --dir "$ANDROID_DIR/res" -o "$BUILD_DIR/compiled"
aapt2 link -I "$FRAMEWORK_RES" --manifest "$ANDROID_DIR/AndroidManifest.xml" \
    --min-sdk-version 29 --target-sdk-version 35 \
    --version-code "$VERSION_CODE" --version-name "$VERSION_NAME" \
    -o "$BUILD_DIR/rustdl-unsigned.apk" \
    "$BUILD_DIR/compiled"/*.flat

cp "$BUILD_DIR/dex/classes.dex" "$BUILD_DIR/apk/classes.dex"
cp "$ROOT_DIR/target/release/librustdl.so" \
    "$BUILD_DIR/apk/lib/arm64-v8a/librustdl.so"
(cd "$BUILD_DIR/apk" && jar uf "$BUILD_DIR/rustdl-unsigned.apk" \
    classes.dex lib/arm64-v8a/librustdl.so)

KEYSTORE=${RUSTDL_KEYSTORE:-"$ANDROID_DIR/debug.keystore"}
KEY_ALIAS=${RUSTDL_KEY_ALIAS:-androiddebugkey}
KEYSTORE_PASSWORD=${RUSTDL_KEYSTORE_PASSWORD:-android}
KEY_PASSWORD=${RUSTDL_KEY_PASSWORD:-$KEYSTORE_PASSWORD}
if [ ! -f "$KEYSTORE" ]; then
    if [ -n "${RUSTDL_KEYSTORE:-}" ]; then
        echo "Configured RUSTDL_KEYSTORE does not exist: $KEYSTORE" >&2
        exit 2
    fi
    keytool -genkeypair -keystore "$KEYSTORE" -storepass android \
        -alias androiddebugkey -keypass android -dname "CN=RustDL Debug,O=RustDL,C=CA" \
        -keyalg RSA -keysize 2048 -validity 10000 >/dev/null 2>&1
fi

apksigner sign --v1-signing-enabled true --v2-signing-enabled true \
    --v3-signing-enabled true --ks "$KEYSTORE" --ks-pass "pass:$KEYSTORE_PASSWORD" \
    --ks-key-alias "$KEY_ALIAS" --key-pass "pass:$KEY_PASSWORD" \
    --out "$BUILD_DIR/rustdl.apk" \
    "$BUILD_DIR/rustdl-unsigned.apk"
apksigner verify --verbose "$BUILD_DIR/rustdl.apk"

echo "$BUILD_DIR/rustdl.apk"
