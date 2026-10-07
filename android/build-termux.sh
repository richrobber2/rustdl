#!/data/data/com.termux/files/usr/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
ANDROID_DIR="$ROOT_DIR/android"
VARIANT=${RUSTDL_VARIANT:-standard}
case "$VARIANT" in
    standard) BUILD_DIR="$ROOT_DIR/target/android-termux" ;;
    alongside) BUILD_DIR="$ROOT_DIR/target/android-termux-alongside" ;;
    *) echo "RUSTDL_VARIANT must be standard or alongside" >&2; exit 2 ;;
esac
DEV=${RUSTDL_DEV:-0}
case "$DEV" in 0) ;; 1) BUILD_DIR="$BUILD_DIR-dev" ;; *) echo "RUSTDL_DEV must be 0 or 1" >&2; exit 2 ;; esac
NATIVE_UI=${RUSTDL_NATIVE_UI:-1}
case "$NATIVE_UI" in 0) ;; 1) BUILD_DIR="$BUILD_DIR-gpui" ;; *) echo "RUSTDL_NATIVE_UI must be 0 or 1" >&2; exit 2 ;; esac
FRAMEWORK_RES=/system/framework/framework-res.apk
ANDROID_JAR=${ANDROID_JAR:-}

for TOOL in cargo javac d8 aapt2 jar keytool apksigner; do
    if ! command -v "$TOOL" >/dev/null 2>&1; then
        echo "Missing build tool: $TOOL. Run 'make setup' first." >&2
        exit 1
    fi
done
if [ "$NATIVE_UI" = 1 ]; then
    for TOOL in clang clang++ llvm-ar llvm-strip llvm-readelf patchelf python3; do
        if ! command -v "$TOOL" >/dev/null 2>&1; then
            echo "Missing native UI build tool: $TOOL. Run 'make native-setup' first." >&2
            exit 1
        fi
    done
fi
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
if [ "$NATIVE_UI" = 1 ]; then
    NATIVE_LIBRARY=$(sh "$ROOT_DIR/native-ui/build-termux.sh")
fi

mkdir -p "$BUILD_DIR"
find "$BUILD_DIR" -mindepth 1 -delete
mkdir -p "$BUILD_DIR/classes" "$BUILD_DIR/dex" "$BUILD_DIR/compiled" \
    "$BUILD_DIR/apk/lib/arm64-v8a"

SOURCE_DIR="$BUILD_DIR/source"
# Stage every build so optional development sources never modify android/.
mkdir -p "$SOURCE_DIR"
cp "$ANDROID_DIR"/*.java "$SOURCE_DIR/"
cp "$ANDROID_DIR/AndroidManifest.xml" "$SOURCE_DIR/"
cp -R "$ANDROID_DIR/res" "$SOURCE_DIR/res"
if [ "$NATIVE_UI" = 1 ]; then
    python3 "$ROOT_DIR/native-ui/stage-android.py" "$SOURCE_DIR"
fi
if [ "$VARIANT" = alongside ]; then
    # Keep Java package/class names stable for the Rust JNI symbols.
    sed -i \
        -e 's/package="app.rustdl"/package="app.rustdl.next"/' \
        -e 's/android:label="RustDL"/android:label="RustDL Next"/' \
        -e 's/Download with RustDL/Download with RustDL Next/' \
        -e 's/app.rustdl.preferences/app.rustdl.next.preferences/g' \
        -e 's/app.rustdl.visual-review/app.rustdl.next.visual-review/g' \
        -e 's/app.rustdl.inspection/app.rustdl.next.inspection/g' \
        -e 's/app.rustdl.action./app.rustdl.next.action./g' \
        "$SOURCE_DIR/AndroidManifest.xml"
    sed -i 's/android:targetPackage="app.rustdl"/android:targetPackage="app.rustdl.next"/' \
        "$SOURCE_DIR/res/xml/shortcuts.xml"
    sed -i 's/RustDL/RustDL Next/g' "$SOURCE_DIR/res/values/strings.xml"
    for SOURCE in "$SOURCE_DIR"/*.java; do
        sed -i \
            -e 's/37658/37758/g' -e 's/37659/37759/g' \
            -e 's/37_658/37_758/g' -e 's/37_659/37_759/g' \
            -e 's/app.rustdl.action./app.rustdl.next.action./g' \
            -e 's/DEFAULT_DOWNLOAD_FOLDER = "RustDL"/DEFAULT_DOWNLOAD_FOLDER = "RustDL Next"/' \
            "$SOURCE"
    done
fi

if [ "$DEV" = 1 ]; then
    cp "$ROOT_DIR/src/dev/android/GalleryBenchmarkActivity.java" "$ROOT_DIR/src/dev/android/DevRealGalleryMetrics.java" "$SOURCE_DIR/"
    python3 "$ROOT_DIR/src/dev/instrument-main.py" "$SOURCE_DIR/MainActivity.java"
    sed -i '/    <\/application>/i\        <activity android:name="app.rustdl.GalleryBenchmarkActivity" android:exported="true" android:process=":dev_benchmark" android:launchMode="singleTask" android:label="RustDL dev gallery test" android:configChanges="keyboardHidden|orientation|screenSize" />' "$SOURCE_DIR/AndroidManifest.xml"
    mkdir -p "$BUILD_DIR/apk/assets/dev"
    cp "$ROOT_DIR/assets/html/index.html" "$ROOT_DIR/assets/html/gallery.html" \
        "$ROOT_DIR/assets/css/index.css" "$ROOT_DIR/assets/css/appearance.css" \
        "$ROOT_DIR/assets/js/playback.js" "$ROOT_DIR/src/dev/visual-scroll.js" "$ROOT_DIR/src/dev/visual-smoke.js" "$ROOT_DIR/src/dev/visual-audit.js" "$ROOT_DIR/src/dev/visual-canary.js" "$ROOT_DIR/src/dev/visual-batch500.js" "$ROOT_DIR/src/dev/real-gallery-metrics.js" \
        "$BUILD_DIR/apk/assets/dev/"
    cp "$ROOT_DIR/assets/images/aniwaves-rainy-city.webp" "$BUILD_DIR/apk/assets/dev/rainy-city.webp"
    cp "$ROOT_DIR/assets/images/aniwaves-rainy-city-light.webp" "$BUILD_DIR/apk/assets/dev/rainy-city-light.webp"
    python3 "$ROOT_DIR/src/dev/prepare-visual.py" "$BUILD_DIR/apk/assets/dev"
fi

# Keep Java 8 compatibility; silence only newer JDKs' obsolete-option notices.
javac --release 8 -Xlint:-options -classpath "$ANDROID_JAR" \
    -d "$BUILD_DIR/classes" \
    $(find "$SOURCE_DIR" -name '*.java' -type f)

d8 --lib "$ANDROID_JAR" --output "$BUILD_DIR/dex" \
    $(find "$BUILD_DIR/classes" -name '*.class' -type f)

aapt2 compile --dir "$SOURCE_DIR/res" -o "$BUILD_DIR/compiled"
aapt2 link -I "$FRAMEWORK_RES" --manifest "$SOURCE_DIR/AndroidManifest.xml" \
    --min-sdk-version 29 --target-sdk-version 35 \
    --version-code "$VERSION_CODE" --version-name "$VERSION_NAME" \
    -o "$BUILD_DIR/rustdl-unsigned.apk" \
    "$BUILD_DIR/compiled"/*.flat

cp "$BUILD_DIR/dex/classes.dex" "$BUILD_DIR/apk/classes.dex"
cp "$ROOT_DIR/target/release/librustdl.so" \
    "$BUILD_DIR/apk/lib/arm64-v8a/librustdl.so"
if [ "$NATIVE_UI" = 1 ]; then
    cp "$NATIVE_LIBRARY" "$BUILD_DIR/apk/lib/arm64-v8a/librustdl_ui.so"
    llvm-strip --strip-unneeded "$BUILD_DIR/apk/lib/arm64-v8a/librustdl_ui.so"
    patchelf --remove-rpath "$BUILD_DIR/apk/lib/arm64-v8a/librustdl_ui.so"
    if llvm-readelf -d "$NATIVE_LIBRARY" | grep -q 'NEEDED.*libc++_shared.so'; then
        cp "$PREFIX/lib/libc++_shared.so" "$BUILD_DIR/apk/lib/arm64-v8a/"
    fi
fi
(cd "$BUILD_DIR/apk" && jar uf "$BUILD_DIR/rustdl-unsigned.apk" \
    classes.dex lib)

if [ "$DEV" = 1 ]; then
    (cd "$BUILD_DIR/apk" && jar uf "$BUILD_DIR/rustdl-unsigned.apk" assets)
fi

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
