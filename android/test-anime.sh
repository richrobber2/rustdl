#!/data/data/com.termux/files/usr/bin/sh
set -eu
ROOT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
TEST_DIR="$ROOT_DIR/target/anime-tests"
ANDROID_JAR="$ROOT_DIR/android/platform/android.jar"
mkdir -p "$TEST_DIR/classes" "$TEST_DIR/native-classes" "$TEST_DIR/dex"
javac --release 8 -Xlint:-options -d "$TEST_DIR/classes" "$ROOT_DIR/android/AnimePlaylist.java" "$ROOT_DIR/tests/android/AnimePlaylistTest.java"
java -cp "$TEST_DIR/classes" app.rustdl.AnimePlaylistTest
javac --release 8 -Xlint:-options -classpath "$ANDROID_JAR" -d "$TEST_DIR/native-classes" \
    "$ROOT_DIR/android/AnimeMedia.java" "$ROOT_DIR/tests/android/AnimeMediaTest.java"
if [ -f "$TEST_DIR/dex/classes.dex" ]; then chmod u+w "$TEST_DIR/dex/classes.dex"; fi
d8 --lib "$ANDROID_JAR" --output "$TEST_DIR/dex" "$TEST_DIR/native-classes/app/rustdl/"*.class
chmod 0400 "$TEST_DIR/dex/classes.dex"
env -u LD_LIBRARY_PATH -u LD_PRELOAD CLASSPATH="$TEST_DIR/dex/classes.dex" \
    /system/bin/app_process -Xnoimage-dex2oat / app.rustdl.AnimeMediaTest "$TEST_DIR/fixtures"
