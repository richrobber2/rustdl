package app.rustdl;
import java.net.URL;
import java.io.IOException;
public class AnimePlaylistTest {
    static void check(boolean value) { if (!value) throw new AssertionError(); }
    static void rejects(String body) throws Exception {
        try { AnimePlaylist.parse(new URL("https://media.example/episode/list.m3u8"),body); throw new AssertionError("Accepted unsupported playlist"); }
        catch (IOException expected) {}
    }
    public static void main(String[] args) throws Exception {
        URL base = new URL("https://media.example/episode/master.m3u8");
        AnimePlaylist master = AnimePlaylist.parse(base,"#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",DEFAULT=YES,URI=\"audio/list.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=100,AUDIO=\"a\"\nlow.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=900,AUDIO=\"a\"\nhigh.m3u8\n");
        check(master.variant.toString().equals("https://media.example/episode/high.m3u8"));
        check(master.audio.toString().equals("https://media.example/episode/audio/list.m3u8"));
        AnimePlaylist media = AnimePlaylist.parse(base,"#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:5,\na.m4s\n#EXTINF:5,\nb.m4s\n#EXT-X-ENDLIST\n");
        check(media.segments.size()==3);check(media.segments.get(1).toString().endsWith("/episode/a.m4s"));
        rejects("#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key\"\n#EXTINF:5,\na.ts\n#EXT-X-ENDLIST");
        rejects("#EXTM3U\n#EXTINF:5,\na.ts\n");
        rejects("#EXTM3U\n#EXT-X-BYTERANGE:10@0\n#EXTINF:5,\na.ts\n#EXT-X-ENDLIST");
        rejects("#EXTM3U\n#EXT-X-DISCONTINUITY\n#EXTINF:5,\na.ts\n#EXT-X-ENDLIST");
        rejects("#EXTM3U\n#EXTINF:5,\nhttp://media.example/a.ts\n#EXT-X-ENDLIST");
        rejects("#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=500\n");
        System.out.println("HLS parser: master/audio selection, relative segments, and unsupported formats passed");
    }
}
