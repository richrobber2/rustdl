package app.rustdl;

import android.content.Context;
import android.content.SharedPreferences;
import android.webkit.JavascriptInterface;

import org.json.JSONArray;
import org.json.JSONObject;

import java.util.Map;
import java.util.regex.Pattern;

final class PlaybackBridge {
    private static final Pattern VIDEO_NAME = Pattern.compile(
            "(?:[0-9]+-[1-9][0-9]*|youtube-[A-Za-z0-9_-]{11}|snapchat-[A-Za-z0-9_-]{20,160})\\.(?:mp4|m4a)");
    private static final String POSITION_PREFIX = "position:";
    private static final String DURATION_PREFIX = "duration:";
    private static final String UPDATED_PREFIX = "updated:";
    private static final String WATCHED_PREFIX = "watched:";
    private static final String RATE = "rate";

    private final MainActivity activity;
    private final SharedPreferences preferences;

    PlaybackBridge(MainActivity activity) {
        this.activity = activity;
        preferences = activity.getSharedPreferences("playback", Context.MODE_PRIVATE);
    }

    @JavascriptInterface
    public double getPosition(String filename) {
        return valid(filename) ? preferences.getFloat(POSITION_PREFIX + filename, 0f) : 0d;
    }

    @JavascriptInterface
    public double getPlaybackRate() {
        return preferences.getFloat(RATE, 1f);
    }

    @JavascriptInterface
    public void savePlaybackRate(double rate) {
        if (Double.isFinite(rate) && rate >= 0.5d && rate <= 2d) {
            preferences.edit().putFloat(RATE, (float) rate).apply();
        }
    }

    @JavascriptInterface
    public void savePosition(String filename, double position, double duration) {
        if (!valid(filename) || !Double.isFinite(position) || !Double.isFinite(duration)
                || position < 0d || duration <= 0d) {
            return;
        }
        if (position >= duration - 5d) {
            clearPosition(filename);
            return;
        }
        preferences.edit()
                .putFloat(POSITION_PREFIX + filename, (float) Math.min(position, duration))
                .putFloat(DURATION_PREFIX + filename, (float) duration)
                .putLong(UPDATED_PREFIX + filename, System.currentTimeMillis())
                .remove(WATCHED_PREFIX + filename)
                .apply();
    }

    @JavascriptInterface
    public void clearPosition(String filename) {
        if (!valid(filename)) {
            return;
        }
        preferences.edit()
                .remove(POSITION_PREFIX + filename)
                .remove(DURATION_PREFIX + filename)
                .remove(UPDATED_PREFIX + filename)
                .apply();
    }

    @JavascriptInterface
    public void markWatched(String filename) {
        if (valid(filename)) {
            preferences.edit()
                    .putBoolean(WATCHED_PREFIX + filename, true)
                    .remove(POSITION_PREFIX + filename)
                    .remove(DURATION_PREFIX + filename)
                    .remove(UPDATED_PREFIX + filename)
                    .apply();
        }
    }

    @JavascriptInterface
    public void shareVideo(String filename) {
        if (valid(filename)) {
            activity.sharePublishedDownload(filename);
        }
    }

    String watchedFilenames() {
        StringBuilder filenames = new StringBuilder();
        for (Map.Entry<String, ?> entry : preferences.getAll().entrySet()) {
            if (!entry.getKey().startsWith(WATCHED_PREFIX)
                    || !Boolean.TRUE.equals(entry.getValue())) {
                continue;
            }
            String filename = entry.getKey().substring(WATCHED_PREFIX.length());
            if (valid(filename)) {
                if (filenames.length() > 0) filenames.append('\n');
                filenames.append(filename);
            }
        }
        return filenames.toString();
    }

    void forget(String filename) {
        if (!valid(filename)) return;
        preferences.edit()
                .remove(POSITION_PREFIX + filename)
                .remove(DURATION_PREFIX + filename)
                .remove(UPDATED_PREFIX + filename)
                .remove(WATCHED_PREFIX + filename)
                .apply();
    }

    @JavascriptInterface
    public String getContinueWatching() {
        JSONArray items = new JSONArray();
        for (Map.Entry<String, ?> entry : preferences.getAll().entrySet()) {
            String key = entry.getKey();
            if (!key.startsWith(POSITION_PREFIX)) {
                continue;
            }
            String filename = key.substring(POSITION_PREFIX.length());
            if (!valid(filename) || !(entry.getValue() instanceof Float)) {
                continue;
            }
            double position = (Float) entry.getValue();
            double duration = preferences.getFloat(DURATION_PREFIX + filename, 0f);
            if (position < 5d || duration <= 0d || position >= duration - 5d) {
                continue;
            }
            JSONObject item = new JSONObject();
            try {
                item.put("filename", filename);
                item.put("position", position);
                item.put("duration", duration);
                item.put("updated", preferences.getLong(UPDATED_PREFIX + filename, 0L));
                items.put(item);
            } catch (Exception ignored) {
            }
        }
        return items.toString();
    }

    @JavascriptInterface
    public boolean supportsPictureInPicture() {
        return activity.supportsPictureInPicture();
    }

    @JavascriptInterface
    public void enterPictureInPicture(int width, int height) {
        activity.requestPictureInPicture(width, height);
    }

    @JavascriptInterface
    public void setRotationLocked(boolean locked) {
        activity.setPlaybackRotationLocked(locked);
    }

    @JavascriptInterface
    public void setPlaybackState(boolean playing, int width, int height) {
        activity.setPlaybackActive(playing, width, height);
    }

    private static boolean valid(String filename) {
        return filename != null && VIDEO_NAME.matcher(filename).matches();
    }
}
