#!/usr/bin/env python3
"""Native navigation contract, tested without launching an app or reading media."""
import os
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


class NativeHomeRoutesTest(unittest.TestCase):
    def test_anime_native_media_policy_without_loading_media(self):
        with tempfile.TemporaryDirectory(prefix="rustdl-native-stream-") as folder:
            fixture = pathlib.Path(folder) / "StreamPolicyTest.java"
            fixture.write_text("""package app.rustdl;
public class StreamPolicyTest {
 public static void main(String[] args) {
  for(String url:new String[]{"https://media.example.invalid/master.m3u8?token=synthetic","https://media.example.invalid/episode.mp4"})
   if(!StreamingMediaPolicy.playable(url))throw new AssertionError("supported media");
  for(String url:new String[]{"https://media.example.invalid/embed/player","https://media.example.invalid/init.mp4","file:///episode.mp4","blob:https://example.invalid/id","http://example.invalid/x.mp4","https://localhost/x.m3u8","https://127.0.0.1/x.mp4","https://user@example.invalid/x.mp4"})
   if(StreamingMediaPolicy.playable(url))throw new AssertionError("unsafe/non-media");
 }
}
""")
            subprocess.run(["javac", "-d", folder,
                            str(ROOT / "android/StreamingMediaPolicy.java"), str(fixture)], check=True)
            subprocess.run(["java", "-cp", folder, "app.rustdl.StreamPolicyTest"], check=True)

    def test_scroll_viewport_with_synthetic_overflowing_rows(self):
        directory = ROOT / "target/native-scroll-layout-fixture"
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "Cargo.toml").write_text("""[package]
name="rustdl-scroll-layout-fixture"
version="0.0.0"
edition="2024"
[workspace]
[dependencies]
taffy="=0.13.0"
[lib]
path="../../tests/fixtures/native_scroll_layout.rs"
""")
        subprocess.run(["cargo", "test", "--offline", "--manifest-path",
                        str(directory / "Cargo.toml"), "--lib"], check=True)

    def test_anime_poster_policy_rejects_unsafe_addresses_without_fetching(self):
        with tempfile.TemporaryDirectory(prefix="rustdl-poster-policy-") as folder:
            fixture = pathlib.Path(folder) / "PosterPolicyTest.java"
            fixture.write_text("""package app.rustdl;
public class PosterPolicyTest {
 public static void main(String[] args) {
  if(!NativeAnimePosters.allowed("https://static.aniwaves.ru/synthetic.jpg"))throw new AssertionError("HTTPS");
  for(String url:new String[]{"http://example.invalid/x","file:///x","javascript:alert(1)","https://localhost/x","https://127.0.0.1/x","https://user@example.invalid/x","https://example.invalid/x#fragment"})
   if(NativeAnimePosters.allowed(url))throw new AssertionError("unsafe poster");
 }
}
""")
            platform = str(ROOT / "android/platform/android.jar")
            subprocess.run(["javac", "-cp", platform, "-d", folder,
                            str(ROOT / "android/NativeAnimePosters.java"), str(fixture)], check=True)
            subprocess.run(["java", "-cp", folder + os.pathsep + platform,
                            "app.rustdl.PosterPolicyTest"], check=True)

    def test_native_scroll_distance_survives_batching_and_rejects_stale_flings(self):
        with tempfile.TemporaryDirectory(prefix="rustdl-scroll-distance-") as folder:
            binary = pathlib.Path(folder) / "scroll-tests"
            subprocess.run(["rustc", "--edition", "2024", "--test",
                            str(ROOT / "native-ui/src/scroll.rs"), "-o", str(binary)], check=True)
            subprocess.run([str(binary)], check=True)

    def test_native_history_expansion_keeps_note_order_and_older_releases(self):
        with tempfile.TemporaryDirectory(prefix="rustdl-history-rows-") as folder:
            binary = pathlib.Path(folder) / "history-tests"
            subprocess.run(["rustc", "--edition", "2024", "--test",
                            str(ROOT / "native-ui/src/history.rs"), "-o", str(binary)], check=True)
            subprocess.run([str(binary)], check=True)

    def test_queued_semantic_actions_reject_changed_context(self):
        with tempfile.TemporaryDirectory(prefix="rustdl-semantic-action-") as folder:
            binary=pathlib.Path(folder) / "actions"
            subprocess.run(["rustc","--edition","2024","--test",
                            str(ROOT / "native-ui/a11y-action-policy.rs"),"-o",str(binary)],check=True)
            subprocess.run([str(binary)],check=True)

    def test_native_accessibility_with_synthetic_semantic_trees(self):
        directory = ROOT / "target/native-accessibility-fixture"
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "Cargo.toml").write_text("""[package]
name="rustdl-accessibility-fixture"
version="0.0.0"
edition="2024"
[workspace]
[dependencies]
accesskit="=0.24.1"
serde={version="1",features=["derive"]}
serde_json="1"
[lib]
path="../../native-ui/src/accessibility.rs"
""")
        subprocess.run(["cargo", "test", "--offline", "--manifest-path",
                        str(directory / "Cargo.toml"), "--lib"], check=True)

    def test_aggregate_progress_with_synthetic_counters(self):
        with tempfile.TemporaryDirectory(prefix="rustdl-native-progress-") as folder:
            binary = pathlib.Path(folder) / "progress-tests"
            subprocess.run([
                "rustc", "--edition", "2024", "--test",
                str(ROOT / "native-ui/src/progress.rs"), "-o", str(binary),
            ], check=True)
            subprocess.run([str(binary)], check=True)

    def test_native_release_notes_are_shared_without_web_dependencies(self):
        with tempfile.TemporaryDirectory(prefix="rustdl-native-history-") as folder:
            directory = pathlib.Path(folder)
            generator = directory / "generate-notes"
            subprocess.run([
                "rustc", "--edition", "2024", str(ROOT / "native-ui/build.rs"),
                "-o", str(generator),
            ], check=True)
            subprocess.run([str(generator)], cwd=ROOT / "native-ui",
                           env={**os.environ, "OUT_DIR": str(directory)}, check=True)
            fixture = directory / "history-test.rs"
            fixture.write_text('''include!("release_notes.rs");
fn main() {
    assert_eq!(CHANGELOG.first().unwrap().0, "0.1.45");
    assert_eq!(CHANGELOG.last().unwrap().0, "0.1.0");
    assert!(CHANGELOG.iter().all(|(_, notes)| !notes.is_empty()));
    assert!(CHANGELOG.len() > 13);
}''')
            binary = directory / "history-test"
            subprocess.run(["rustc", "--edition", "2024", str(fixture),
                            "-o", str(binary)], check=True)
            subprocess.run([str(binary)], check=True)

    def test_streaming_download_choices_preserve_hls_priority_without_metadata(self):
        fixture = r'''package app.rustdl;
import java.util.*;
public class StreamingDownloadChoicesFixture {
    public static void main(String[] ignored) {
        String mp4="https://example.invalid/private-title.mp4";
        String hls="https://example.invalid/private-title.m3u8?secret=fixture";
        List<String> sources=Arrays.asList(mp4,hls,mp4,null,"");
        List<StreamingDownloadChoices.Choice> choices=StreamingDownloadChoices.select(sources,Collections.singleton(hls));
        if(choices.size()!=1||!choices.get(0).hls||!choices.get(0).source.equals(hls)) throw new AssertionError("HLS priority lost");
        if(!choices.get(0).label(0).equals("HLS · 1")) throw new AssertionError("Metadata leaked into label");
        choices=StreamingDownloadChoices.select(sources,Collections.emptySet());
        if(choices.size()!=2||!choices.get(0).source.equals(mp4)||choices.get(0).hls) throw new AssertionError("Fallback order or deduplication lost");
        if(!StreamingDownloadChoices.select(Collections.emptyList(),Collections.emptySet()).isEmpty()) throw new AssertionError("Empty candidates invented");
    }
}'''
        with tempfile.TemporaryDirectory(prefix="rustdl-stream-download-") as folder:
            directory=pathlib.Path(folder)
            source=directory / "StreamingDownloadChoicesFixture.java"
            source.write_text(fixture)
            subprocess.run(["javac","--release","8","-Xlint:-options","-d",str(directory),str(ROOT / "android/StreamingDownloadChoices.java"),str(source)],check=True)
            subprocess.run(["java","-cp",str(directory),"app.rustdl.StreamingDownloadChoicesFixture"],check=True)

    def test_streaming_control_filters_preserve_decoder_indexes(self):
        fixture=r'''package app.rustdl;
import java.util.*;
public class StreamingControlPolicyFixture {
    public static void main(String[] ignored) {
        List<String> languages=new ArrayList<>(); List<Boolean> ready=new ArrayList<>();
        for(int i=0;i<65;i++) { languages.add(i==64?"DUB":"sub");ready.add(i!=64); }
        if(!StreamingControlPolicy.matchingIndices(languages,ready,"dub").equals(Arrays.asList(64))) throw new AssertionError("Decoder indexes lost");
        if(!StreamingControlPolicy.matchingIndices(languages,ready,"issues").equals(Arrays.asList(64))) throw new AssertionError("Unavailable selection lost");
        if(StreamingControlPolicy.matchingIndices(languages,ready,"ready").size()!=64) throw new AssertionError("Ready filtering failed");
        if(!StreamingControlPolicy.currentGeneration(7,7)||StreamingControlPolicy.currentGeneration(6,7)||StreamingControlPolicy.currentGeneration(-1,-1)) throw new AssertionError("Stale decoder command accepted");
        if(StreamingControlPolicy.matches("sub",true,"unknown")) throw new AssertionError("Unvalidated filter accepted");
        if(!StreamingControlPolicy.matchingIndices(Collections.emptyList(),Collections.emptyList(),"all").isEmpty()) throw new AssertionError("Invented server");
    }
}'''
        with tempfile.TemporaryDirectory(prefix="rustdl-stream-controls-") as folder:
            directory=pathlib.Path(folder);source=directory / "StreamingControlPolicyFixture.java";source.write_text(fixture)
            subprocess.run(["javac","--release","8","-Xlint:-options","-d",str(directory),str(ROOT / "android/StreamingControlPolicy.java"),str(source)],check=True)
            subprocess.run(["java","-cp",str(directory),"app.rustdl.StreamingControlPolicyFixture"],check=True)

    def test_native_preferences_reject_unvalidated_commands(self):
        fixture = '''package app.rustdl;
public class NativeSettingsRequestFixture {
    public static void main(String[] ignored) {
        String[][] accepted = {{"appearance", "dark"}, {"appearance", "system"},
            {"backgroundTheme", "rainy-city"}, {"mobileDownloadPolicy", "block"},
            {"inspectionPrivacy", "true"}, {"allowScreenshots", "false"},
            {"diagnosticsRefreshSeconds", "30"}, {"downloadFolder", ""}, {"reset", ""}};
        for (String[] request : accepted) {
            if (!NativeSettingsRequest.valid(request[0], request[1]))
                throw new AssertionError("Expected preference rejected");
        }
        String[][] rejected = {{null, "dark"}, {"appearance", null},
            {"appearance", "javascript:alert(1)"}, {"unknown", "true"},
            {"allowScreenshots", "yes"}, {"diagnosticsRefreshSeconds", "0"},
            {"mobileDownloadPolicy", "wifi"}, {"reset", "true"},
            {"downloadFolder", "../arbitrary"}};
        for (String[] request : rejected) {
            if (NativeSettingsRequest.valid(request[0], request[1]))
                throw new AssertionError("Unvalidated preference accepted");
        }
    }
}'''
        with tempfile.TemporaryDirectory(prefix="rustdl-native-preferences-") as folder:
            directory = pathlib.Path(folder)
            source = directory / "NativeSettingsRequestFixture.java"
            source.write_text(fixture)
            subprocess.run(["javac", "--release", "8", "-Xlint:-options", "-d", str(directory),
                            str(ROOT / "android/NativeSettingsRequest.java"), str(source)], check=True)
            subprocess.run(["java", "-cp", str(directory),
                            "app.rustdl.NativeSettingsRequestFixture"], check=True)

    def test_native_playback_rejects_foreign_sources_and_hidden_frames(self):
        fixture = r'''package app.rustdl;
public class NativePlaybackPolicyFixture {
    public static void main(String[] ignored) {
        String file="youtube-abcdefghijk.mp4", origin="http://127.0.0.1:37758/";
        if(!NativePlaybackPolicy.artworkResultAllowed(7,7,false,true,false)||NativePlaybackPolicy.artworkResultAllowed(6,7,false,true,false)||NativePlaybackPolicy.artworkResultAllowed(7,7,true,true,false)||NativePlaybackPolicy.artworkResultAllowed(7,7,false,false,false)||NativePlaybackPolicy.artworkResultAllowed(7,7,false,true,true)) throw new AssertionError("Late or private artwork published");
        if(NativePlaybackPolicy.artworkSample(512,256)!=1||NativePlaybackPolicy.artworkSample(2048,1024)!=4||NativePlaybackPolicy.artworkSample(0,100)!=0||NativePlaybackPolicy.artworkSample(16385,100)!=0||NativePlaybackPolicy.artworkSample(16384,16384)!=0) throw new AssertionError("Unbounded artwork decoding");
        for (String route : new String[]{"media", "stream"}) {
            if (!NativePlaybackPolicy.validSource(file, origin+route+"/"+file, origin))
                throw new AssertionError("Validated local route rejected");
        }
        if(NativePlaybackPolicy.completed(true,false,100d,100d,true)||!NativePlaybackPolicy.completed(true,false,100d,100d,false)) throw new AssertionError("Partial download marked watched");
        if(!NativePlaybackPolicy.transitionCurrent(2,2,3,3)||NativePlaybackPolicy.transitionCurrent(2,3,3,3)||NativePlaybackPolicy.transitionCurrent(2,2,3,4)) throw new AssertionError("Outdated completion transition accepted");
        if(!NativePlaybackPolicy.previewAvailable(25,100,50,true)||NativePlaybackPolicy.previewAvailable(75,100,50,true)||NativePlaybackPolicy.previewAvailable(25,100,Double.NaN,true)||!NativePlaybackPolicy.previewAvailable(75,100,0,false)||NativePlaybackPolicy.previewAvailable(-1,100,100,false)||NativePlaybackPolicy.previewAvailable(Double.NaN,100,100,false)||NativePlaybackPolicy.previewAvailable(1,0,100,false)) throw new AssertionError("Preview availability gate");
        if(Math.abs(NativePlaybackPolicy.growingSeekSeconds(90,10,100,50,true)-49.65)>.001 || NativePlaybackPolicy.growingSeekSeconds(5,10,100,0,true)!=5 || NativePlaybackPolicy.growingSeekSeconds(90,10,100,Double.NaN,true)!=10 || NativePlaybackPolicy.growingSeekSeconds(90,10,100,0,false)!=90) throw new AssertionError("Growing seek boundary");
        if(!NativePlaybackPolicy.growingRetryAllowed(true,50,40,0,5000)||NativePlaybackPolicy.growingRetryAllowed(false,50,40,0,5000)||NativePlaybackPolicy.growingRetryAllowed(true,40,40,0,5000)||NativePlaybackPolicy.growingRetryAllowed(true,50,40,3,5000)||NativePlaybackPolicy.growingRetryAllowed(true,50,40,0,4999)) throw new AssertionError("Growing recovery bounds");
        if(!NativePlaybackPolicy.playerScreen(12)||!NativePlaybackPolicy.playerScreen(15)||NativePlaybackPolicy.playerScreen(5)||NativePlaybackPolicy.playerScreen(-1)) throw new AssertionError("Native player layouts");
        if(!NativePlaybackPolicy.autoplayAllowed(true,true,false)||NativePlaybackPolicy.autoplayAllowed(true,false,false)||NativePlaybackPolicy.autoplayAllowed(true,true,true)||NativePlaybackPolicy.autoplayAllowed(false,true,false)) throw new AssertionError("Background preparation resumed playback");
        if(NativePlaybackPolicy.doubleTapSeek(10,100)!=-10d||NativePlaybackPolicy.doubleTapSeek(90,100)!=10d||NativePlaybackPolicy.doubleTapSeek(Double.NaN,100)!=0d||NativePlaybackPolicy.doubleTapSeek(1,0)!=0d) throw new AssertionError("Invalid double-tap seek");
        if(!NativePlaybackPolicy.allowPip(false,true,false,true,false,true)||NativePlaybackPolicy.allowPip(true,true,false,true,false,true)||NativePlaybackPolicy.allowPip(false,false,false,true,false,true)||NativePlaybackPolicy.allowPip(false,true,false,true,true,true)||NativePlaybackPolicy.allowPip(false,true,true,true,false,true)) throw new AssertionError("Unsafe PiP eligibility");
        if(!NativePlaybackPolicy.pipCommandAllowed("pause",true)||NativePlaybackPolicy.pipCommandAllowed("play",true)||!NativePlaybackPolicy.pipCommandAllowed("play",false)||NativePlaybackPolicy.pipCommandAllowed("unknown",false)) throw new AssertionError("Unvalidated PiP command");
        String[] rejected={"https://example.com/media/"+file,
            "http://127.0.0.1:1/media/"+file, "http://user@127.0.0.1:37758/media/"+file,
            origin+"media/"+file+"?token=secret", origin+"media/"+file+"#frame",
            origin+"media/../"+file, origin+"media/%252e%252e/"+file,
            "file:///private/"+file, "javascript:alert(1)"};
        for (String source : rejected)
            if (NativePlaybackPolicy.validSource(file, source, origin))
                throw new AssertionError("Foreign or untrusted source accepted");
        if (NativePlaybackPolicy.validSource("../"+file,origin+"media/"+file,origin))
            throw new AssertionError("Invalid filename accepted");
        if (NativePlaybackPolicy.showFrames(true,true,false)
                || NativePlaybackPolicy.showFrames(false,false,false)
                || NativePlaybackPolicy.showFrames(false,true,true))
            throw new AssertionError("Protected frames exposed");
        if (!NativePlaybackPolicy.allowFullscreen(false,true,false,true)
                || NativePlaybackPolicy.allowFullscreen(true,true,false,true)
                || NativePlaybackPolicy.allowFullscreen(false,false,false,true)
                || NativePlaybackPolicy.allowFullscreen(false,true,true,true)
                || NativePlaybackPolicy.allowFullscreen(false,true,false,false))
            throw new AssertionError("Invalid fullscreen eligibility");
        if (!NativePlaybackPolicy.showFrames(false,true,false))
            throw new AssertionError("User playback surface blocked");
        for(int change:new int[]{-1,-2,-3})
            if(!NativePlaybackPolicy.pauseForFocusChange(change)) throw new AssertionError("Focus loss ignored");
        for(int change:new int[]{0,1,2,3,4,-4})
            if(NativePlaybackPolicy.pauseForFocusChange(change)) throw new AssertionError("Unexpected focus pause");
        for(double minutes:new double[]{0,15,30,60})
            if(!NativePlaybackPolicy.validSleepMinutes(minutes)) throw new AssertionError("Valid sleep choice rejected");
        for(double minutes:new double[]{-1,1,15.5,120,Double.NaN,Double.POSITIVE_INFINITY})
            if(NativePlaybackPolicy.validSleepMinutes(minutes)) throw new AssertionError("Invalid sleep choice accepted");
        if (!NativePlaybackPolicy.completed(true,false,100,100)
                || NativePlaybackPolicy.completed(false,false,100,100)
                || NativePlaybackPolicy.completed(true,true,100,100)
                || NativePlaybackPolicy.completed(true,false,90,100)
                || NativePlaybackPolicy.completed(true,false,Double.NaN,100)
                || NativePlaybackPolicy.completed(true,false,100,Double.POSITIVE_INFINITY))
            throw new AssertionError("Invalid completion state");
        if (NativePlaybackPolicy.seekSeconds(Double.NaN,100)!=0
                || NativePlaybackPolicy.seekSeconds(10,Double.POSITIVE_INFINITY)!=0
                || NativePlaybackPolicy.seekSeconds(-10,100)!=0
                || NativePlaybackPolicy.seekSeconds(120,100)!=100)
            throw new AssertionError("Invalid seek bounds");
    }
}'''
        with tempfile.TemporaryDirectory(prefix="rustdl-native-playback-") as folder:
            directory=pathlib.Path(folder)
            source=directory/"NativePlaybackPolicyFixture.java"
            source.write_text(fixture)
            subprocess.run(["javac","--release","8","-Xlint:-options","-d",str(directory),
                            str(ROOT/"android/NativePlaybackPolicy.java"),str(source)],check=True)
            subprocess.run(["java","-cp",str(directory),"app.rustdl.NativePlaybackPolicyFixture"],check=True)

    def test_only_fixed_local_destinations_are_allowed(self):
        fixture = '''package app.rustdl;
public class NativeHomeRoutesFixture {
    public static void main(String[] ignored) {
        String[][] routes = {{"library", ""}, {"anime", "streaming"},
            {"queue", "queue"}, {"settings", "settings"}, {"diagnostics", "diagnostics"}, {"changelog", "changelog"}};
        for (String[] route : routes) {
            if (!route[1].equals(NativeHomeRoutes.path(route[0])))
                throw new AssertionError("Incorrect fixed destination");
        }
        String[] rejected = {null, "", "LIBRARY", "../watch", "https://example.com",
            "http://127.0.0.1/queue/action", "javascript:alert(1)", "queue?file=synthetic.mp4"};
        for (String route : rejected) {
            if (NativeHomeRoutes.path(route) != null)
                throw new AssertionError("Unexpected navigation accepted");
        }
    }
}'''
        with tempfile.TemporaryDirectory(prefix="rustdl-native-routes-") as folder:
            directory = pathlib.Path(folder)
            source = directory / "NativeHomeRoutesFixture.java"
            source.write_text(fixture)
            subprocess.run([
                "javac", "--release", "8", "-Xlint:-options", "-d", str(directory),
                str(ROOT / "android/NativeHomeRoutes.java"), str(source),
            ], check=True)
            subprocess.run(["java", "-cp", str(directory),
                            "app.rustdl.NativeHomeRoutesFixture"], check=True)


if __name__ == "__main__":
    unittest.main()
