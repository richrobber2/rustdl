#!/usr/bin/env python3
"""Stage the Java helpers from Cargo's exact pinned mobile-platform revision."""
import json
import pathlib
import shutil
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent
destination = pathlib.Path(sys.argv[1])
metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--offline", "--locked", "--format-version", "1",
    "--filter-platform", "aarch64-linux-android",
    "--manifest-path", str(root / "native-ui/Cargo.toml"),
]))
platform = next(package for package in metadata["packages"] if package["name"] == "gpui-pre-mobile")
source = pathlib.Path(platform["manifest_path"]).parent
source = source / "example/android/gradle/app/src/main/java/dev/gpui/mobile"
helpers = destination / "mobile"
helpers.mkdir(parents=True, exist_ok=True)
for name in ("GpuiPlatformView", "GpuiCamera", "GpuiVideoPlayer"):
    shutil.copyfile(source / f"{name}.java", helpers / f"{name}.java")
shutil.copyfile(root / "native-ui/android/NativeHomeHost.java", destination / "NativeHomeHost.java")
shutil.copyfile(root / "native-ui/android/NativeAccessibility.java", destination / "NativeAccessibility.java")

shutil.copyfile(root / "native-ui/android/NativePlaybackSession.java", destination / "NativePlaybackSession.java")

shutil.copyfile(root / "native-ui/android/NativeStreamingHost.java", destination / "NativeStreamingHost.java")

shutil.copyfile(root / "native-ui/android/NativeVisualHost.java", destination / "NativeVisualHost.java")

# RustDL-owned extension applied ONLY to the staged copy of the pinned helper.
# Fail closed if its implementation changes; never edit Cargo checkout sources.
video = helpers / "GpuiVideoPlayer.java"
text = video.read_text()
create = "                sPlayers.put(id, mp);"
dispose = "            sPlayers.remove(id);"
assert text.count(create) == 1 and text.count(dispose) == 1
text = text.replace(create, create + """
                sBuffered.put(id, 0);
                sBuffering.put(id, false);
                sFailed.put(id, false);
                mp.setOnErrorListener((player, what, extra) -> {
                    synchronized(sLock) {if(sPlayers.get(id)==player) {sFailed.put(id,true);sBuffering.put(id,false);}}
                    return true;
                });
                mp.setOnBufferingUpdateListener((player, percent) -> {
                    synchronized (sLock) { if(sPlayers.get(id)==player) sBuffered.put(id, Math.max(0,Math.min(100,percent))); }
                });
                mp.setOnInfoListener((player, what, extra) -> {
                    synchronized (sLock) {
                        if(sPlayers.get(id)==player) {
                            if(what==MediaPlayer.MEDIA_INFO_BUFFERING_START) sBuffering.put(id,true);
                            if(what==MediaPlayer.MEDIA_INFO_BUFFERING_END) sBuffering.put(id,false);
                        }
                    }
                    return false;
                });
""")
text = text.replace(dispose, dispose + "\n            sBuffered.delete(id);sBuffering.delete(id);sFailed.delete(id);")
assert text.count("    // Prevent instantiation.") == 1
text = text.replace("    // Prevent instantiation.", """
    private static final android.util.SparseIntArray sBuffered = new android.util.SparseIntArray();
    private static final android.util.SparseBooleanArray sBuffering = new android.util.SparseBooleanArray();
    private static final android.util.SparseBooleanArray sFailed = new android.util.SparseBooleanArray();
    public static int getTrackCount(int id,int type) {
        synchronized(sLock) {
            MediaPlayer player=sPlayers.get(id);if(player==null) return 0;
            try {MediaPlayer.TrackInfo[] tracks=player.getTrackInfo();int count=0;if(tracks!=null) for(MediaPlayer.TrackInfo track:tracks) if(track!=null&&track.getTrackType()==type) count++;return count;}
            catch(RuntimeException unavailable) {return 0;}
        }
    }
    public static boolean hasError(int id) { synchronized(sLock) {return sFailed.get(id,false);} }
    public static int getBufferedPercent(int id) { synchronized(sLock) { return sBuffered.get(id,0); } }
    public static boolean isBuffering(int id) { synchronized(sLock) { return sBuffering.get(id,false); } }
    // Prevent instantiation.
""")
video.write_text(text)

shutil.copyfile(root / "native-ui/android/NativePlaybackService.java", destination / "NativePlaybackService.java")
manifest=destination / "AndroidManifest.xml"
if manifest.exists():
    text=manifest.read_text()
    permission='<uses-permission android:name="android.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK" />'
    service='<service android:name="app.rustdl.NativePlaybackService" android:exported="false" android:foregroundServiceType="mediaPlayback" />'
    if permission not in text: text=text.replace('<application',permission+'\n    <application',1)
    if service not in text: text=text.replace('</application>',service+'\n    </application>',1)
    manifest.write_text(text)

shutil.copyfile(root / "native-ui/android/NativePlaybackPreview.java", destination / "NativePlaybackPreview.java")

shutil.copyfile(root / "native-ui/android/NativeAnimeScroll.java", destination / "NativeAnimeScroll.java")
