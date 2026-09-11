package app.rustdl;

import org.json.JSONObject;
import java.io.IOException;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;

/** Encoding shared by the native episode player's watchlist requests. */
final class StreamingWatchlist {
    static String optionalString(JSONObject object, String key) {
        Object value = object.opt(key);
        return value instanceof String ? (String) value : "";
    }

    static String form(String token, String action, String url, String title, String poster)
            throws IOException {
        return part("token", token) + "&" + part("action", action)
                + "&" + part("url", url) + "&" + part("title", title)
                + "&" + part("poster", poster) + "&response=json";
    }

    private static String part(String key, String value) throws IOException {
        return key + "=" + URLEncoder.encode(value == null ? "" : value,
                StandardCharsets.UTF_8.name());
    }

    static boolean saved(int status, String body) throws Exception {
        JSONObject response;
        try {
            response = new JSONObject(body);
        } catch (Exception invalidJson) {
            throw new IOException("Watchlist update failed (HTTP " + status + ")");
        }
        if (status != 200 || !response.optBoolean("ok", false)) {
            String detail = optionalString(response, "error");
            throw new IOException(detail.isEmpty() ? "Watchlist update failed" : detail);
        }
        return response.getBoolean("watchlisted");
    }
}
