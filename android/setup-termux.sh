#!/data/data/com.termux/files/usr/bin/sh
set -eu

ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PLATFORM_DIR="$ROOT_DIR/android/platform"
PLATFORM_JAR="$PLATFORM_DIR/android.jar"
PLATFORM_COMMIT=1e98db1a199e8f7f85541af26bfc27019501b132
PLATFORM_SHA256=4566663c3876e022b4fa4ced8c8697c4ab1688267f090114fd92d027b32e619b
PLATFORM_URL="https://raw.githubusercontent.com/Sable/android-platforms/$PLATFORM_COMMIT/android-35/android.jar"
CHECK_ONLY=false

if [ "${1:-}" = "--check" ]; then
    CHECK_ONLY=true
elif [ "$#" -ne 0 ]; then
    echo "Usage: $0 [--check]" >&2
    exit 2
fi

if [ ! -d /data/data/com.termux/files/usr ]; then
    echo "RustDL's Android builder runs inside Termux on an ARM64 Android device." >&2
    exit 1
fi

if [ "$CHECK_ONLY" = false ]; then
    if ! command -v pkg >/dev/null 2>&1; then
        echo "Termux pkg command not found." >&2
        exit 1
    fi
    packages=""
    command -v cargo >/dev/null 2>&1 || packages="$packages rust"
    command -v javac >/dev/null 2>&1 || packages="$packages openjdk-21"
    command -v d8 >/dev/null 2>&1 || packages="$packages d8"
    command -v aapt2 >/dev/null 2>&1 || packages="$packages aapt2"
    command -v apksigner >/dev/null 2>&1 || packages="$packages apksigner"
    command -v make >/dev/null 2>&1 || packages="$packages make"
    command -v curl >/dev/null 2>&1 || packages="$packages curl"
    if [ -n "$packages" ]; then
        # Intentionally word-split the package list into pkg arguments.
        pkg install -y $packages
    fi
fi

missing=false
for tool in cargo javac d8 aapt2 jar keytool apksigner make curl sha256sum; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Missing build tool: $tool" >&2
        missing=true
    fi
done
if [ "$missing" = true ]; then
    echo "Run 'make setup' to install the Termux build prerequisites." >&2
    exit 1
fi

valid_platform=false
if [ -f "$PLATFORM_JAR" ]; then
    actual=$(sha256sum "$PLATFORM_JAR" | cut -d' ' -f1)
    if [ "$actual" = "$PLATFORM_SHA256" ]; then
        valid_platform=true
    fi
fi

if [ "$valid_platform" = false ] && [ "$CHECK_ONLY" = true ]; then
    echo "Missing or unverified API-35 platform jar: $PLATFORM_JAR" >&2
    echo "Run 'make setup' to fetch the pinned, checksummed copy." >&2
    exit 1
fi

if [ "$valid_platform" = false ]; then
    mkdir -p "$PLATFORM_DIR"
    temporary=$(mktemp "$PLATFORM_DIR/android.jar.XXXXXX")
    trap 'rm -f "$temporary"' EXIT HUP INT TERM
    echo "Fetching pinned Android API 35 platform jar…"
    curl -L --fail --retry 3 -o "$temporary" "$PLATFORM_URL"
    actual=$(sha256sum "$temporary" | cut -d' ' -f1)
    if [ "$actual" != "$PLATFORM_SHA256" ]; then
        echo "android.jar checksum mismatch; refusing to use it." >&2
        exit 1
    fi
    mv "$temporary" "$PLATFORM_JAR"
    trap - EXIT HUP INT TERM
fi

if [ ! -f /system/framework/framework-res.apk ]; then
    echo "Android framework resources are unavailable on this device." >&2
    exit 1
fi

echo "RustDL Android build prerequisites are ready."
