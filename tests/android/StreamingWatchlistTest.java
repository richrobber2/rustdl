package app.rustdl;

import android.net.Uri;
import org.json.JSONObject;
import java.io.IOException;

public class StreamingWatchlistTest {
    private static void check(boolean value) {
        if (!value) throw new AssertionError();
    }

    public static void main(String[] args) throws Exception {
        // Android converts JSON null to the literal string "null" via optString.
        JSONObject manifest = new JSONObject("{\"posterUrl\":null}");
        check("null".equals(manifest.optString("posterUrl", "")));
        String poster = StreamingWatchlist.optionalString(manifest, "posterUrl");
        check(poster.isEmpty());
        check(StreamingWatchlist.optionalString(new JSONObject(), "posterUrl").isEmpty());
        String title = "Hero's Quest & Friends + 日本語";
        String url = "https://aniwaves.ru/watch/test-show-123";
        Uri form = Uri.parse("http://localhost/?" + StreamingWatchlist.form("test-token", "add", url, title, poster));
        check("".equals(form.getQueryParameter("poster")));
        check(title.equals(form.getQueryParameter("title")));
        check(url.equals(form.getQueryParameter("url")));
        check("json".equals(form.getQueryParameter("response")));
        manifest.put("posterUrl", "https://static.aniwaves.ru/resources/thumbnails/test.jpg");
        check(StreamingWatchlist.optionalString(manifest, "posterUrl").endsWith("test.jpg"));
        check(StreamingWatchlist.saved(200, "{\"ok\":true,\"watchlisted\":true}"));
        check(!StreamingWatchlist.saved(200, "{\"ok\":true,\"watchlisted\":false}"));
        try {
            StreamingWatchlist.saved(403, "{\"ok\":false,\"error\":\"Reopen the episode\"}");
            throw new AssertionError();
        } catch (IOException expected) {
            check(expected.getMessage().equals("Reopen the episode"));
        }
        try {
            StreamingWatchlist.saved(503, "Unavailable");
            throw new AssertionError();
        } catch (IOException expected) {
            check(expected.getMessage().contains("503"));
        }
        System.out.println("Native watchlist: null posters, form encoding, save/remove and errors passed");
    }
}
