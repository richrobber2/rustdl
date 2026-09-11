package app.rustdl;

import java.io.IOException;
import java.net.URL;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** Local HLS parsing; downloading and media conversion belong to the service. */
final class AnimePlaylist {
    URL variant;
    URL audio;
    final List<URL> segments = new ArrayList<>();
    private static final Pattern ATTRIBUTE = Pattern.compile("([A-Z0-9-]+)=(?:\"([^\"]*)\"|([^,]*))(?:,|$)");

    static Map<String, String> attributes(String line) {
        Map<String, String> values = new LinkedHashMap<>();
        Matcher matcher = ATTRIBUTE.matcher(line.substring(line.indexOf(':') + 1));
        while (matcher.find()) values.put(matcher.group(1), matcher.group(2) == null ? matcher.group(3) : matcher.group(2));
        return values;
    }

    static AnimePlaylist parse(URL base, String text) throws IOException {
        if (!text.trim().startsWith("#EXTM3U")) throw new IOException("The player did not provide an HLS playlist");
        AnimePlaylist result = new AnimePlaylist();
        long bestBandwidth = -1;
        Map<String, String> pending = null;
        String audioGroup = null;
        List<Map<String, String>> audioTracks = new ArrayList<>();
        boolean ended = false;
        boolean media = false;
        for (String raw : text.split("\\r?\\n")) {
            String line = raw.trim();
            if (line.isEmpty()) continue;
            if (line.startsWith("#EXT-X-KEY:")) {
                if (!"NONE".equals(attributes(line).get("METHOD"))) throw new IOException("This encrypted source cannot be saved. Try another source.");
            } else if (line.startsWith("#EXT-X-BYTERANGE:") || line.equals("#EXT-X-DISCONTINUITY")) {
                throw new IOException("This source's segment layout is not supported. Try another source.");
            } else if (line.startsWith("#EXT-X-STREAM-INF:")) {
                pending = attributes(line);
            } else if (line.startsWith("#EXT-X-MEDIA:")) {
                Map<String, String> track = attributes(line);
                if ("AUDIO".equals(track.get("TYPE"))) audioTracks.add(track);
            } else if (line.startsWith("#EXT-X-MAP:")) {
                Map<String, String> map = attributes(line);
                if (map.containsKey("BYTERANGE")) throw new IOException("Byte-range streams are not supported");
                result.segments.add(resolve(base, map.get("URI")));
            } else if (line.startsWith("#EXTINF:")) {
                media = true;
            } else if (line.equals("#EXT-X-ENDLIST")) {
                ended = true;
            } else if (!line.startsWith("#")) {
                if (pending != null) {
                    long bandwidth;
                    try { bandwidth = Long.parseLong(pending.getOrDefault("BANDWIDTH", "0")); }
                    catch (NumberFormatException invalid) { throw new IOException("Invalid stream bandwidth"); }
                    if (bandwidth > bestBandwidth) {
                        bestBandwidth = bandwidth;
                        result.variant = resolve(base, line);
                        audioGroup = pending.get("AUDIO");
                    }
                    pending = null;
                } else if (media) {
                    result.segments.add(resolve(base, line));
                    media = false;
                }
            }
            if (result.segments.size() > 20000) throw new IOException("Episode has too many segments");
        }
        if (pending != null || media) throw new IOException("Incomplete episode playlist");
        if (result.variant != null) {
            for (Map<String, String> track : audioTracks) {
                if (audioGroup != null && audioGroup.equals(track.get("GROUP-ID")) && track.get("URI") != null) {
                    result.audio = resolve(base, track.get("URI"));
                    if ("YES".equals(track.get("DEFAULT"))) break;
                }
            }
        } else if (!ended || result.segments.isEmpty()) {
            throw new IOException("Only complete on-demand episodes can be saved");
        }
        return result;
    }

    private static URL resolve(URL base, String value) throws IOException {
        if (value == null || value.isEmpty()) throw new IOException("Missing playlist URL");
        URL url = new URL(base, value);
        if (!"https".equalsIgnoreCase(url.getProtocol()) || url.getUserInfo() != null)
            throw new IOException("Unsupported media URL");
        return url;
    }
}
