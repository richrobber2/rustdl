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
            "(?:[0-9]+-[1-9][0-9]*|youtube-[A-Za-z0-9_-]{11}|snapchat-[A-Za-z0-9_-]{20,160}|anime-[a-f0-9]{24})\\.(?:mp4|m4a)");
    private static final String POSITION_PREFIX = "position:";
    private static final String DURATION_PREFIX = "duration:";
    private static final String UPDATED_PREFIX = "updated:";
    private static final String WATCHED_PREFIX = "watched:";
    private static final String RATE = "rate";
    private static final String UP_NEXT = "up-next";
    private static final String UP_NEXT_IMPORTED = "up-next-imported";

    private final java.lang.ref.WeakReference<MainActivity> activity;
    private final SharedPreferences preferences;

    PlaybackBridge(MainActivity activity) {
        this((Context)activity,activity);
    }

    PlaybackBridge(Context context) {this(context,null);}

    private PlaybackBridge(Context context,MainActivity ui) {
        this.activity = new java.lang.ref.WeakReference<>(ui);
        preferences = context.getApplicationContext().getSharedPreferences("playback", Context.MODE_PRIVATE);
    }

    private MainActivity liveActivity() {
        MainActivity current=activity.get();
        return current!=null&&!current.isDestroyed()?current:null;
    }

    @JavascriptInterface
    public synchronized String getPlaybackQueue() {
        return preferences.getString(UP_NEXT, "[]");
    }

    @JavascriptInterface
    public synchronized void importPlaybackQueue(String legacy) {
        if (preferences.getBoolean(UP_NEXT_IMPORTED, false)) return;
        java.util.LinkedHashSet<String> merged = new java.util.LinkedHashSet<>();
        appendQueue(merged, getPlaybackQueue());
        appendQueue(merged, legacy);
        preferences.edit().putString(UP_NEXT, new JSONArray(merged).toString())
                .putBoolean(UP_NEXT_IMPORTED, true).apply();
    }

    @JavascriptInterface
    public synchronized void savePlaybackQueue(String data) {
        java.util.LinkedHashSet<String> names = new java.util.LinkedHashSet<>();
        appendQueue(names, data);
        preferences.edit().putString(UP_NEXT, new JSONArray(names).toString()).apply();
    }

    synchronized void changeQueued(String filename, boolean add) {
        if (!valid(filename)) return;
        java.util.LinkedHashSet<String> names = new java.util.LinkedHashSet<>();
        appendQueue(names, getPlaybackQueue());
        if (add && names.size() < 200) names.add(filename);
        if (!add) names.remove(filename);
        preferences.edit().putString(UP_NEXT, new JSONArray(names).toString()).apply();
    }

    synchronized int queuedCount(String current) {
        java.util.LinkedHashSet<String> names=new java.util.LinkedHashSet<>();
        appendQueue(names,getPlaybackQueue());names.remove(current);return names.size();
    }

    synchronized String nextQueued(String current) {
        java.util.LinkedHashSet<String> names=new java.util.LinkedHashSet<>();
        appendQueue(names,getPlaybackQueue());
        for(String name:names) if(!name.equals(current)) return name;
        return "";
    }

    private static void appendQueue(java.util.LinkedHashSet<String> target, String data) {
        try {
            JSONArray items = new JSONArray(data == null ? "[]" : data);
            for (int i = 0; i < items.length() && target.size() < 200; i++) {
                String name = items.optString(i);
                if (valid(name)) target.add(name);
            }
        } catch (Exception invalid) {
            // Malformed legacy state must not replace the validated native queue.
        }
    }

    @JavascriptInterface
    public double getPosition(String filename) {
        return valid(filename) ? preferences.getFloat(POSITION_PREFIX + filename, 0f) : 0d;
    }

    double getDuration(String filename) {
        return valid(filename) ? preferences.getFloat(DURATION_PREFIX + filename, 0f) : 0d;
    }

    boolean isWatched(String filename) {
        return valid(filename) && preferences.getBoolean(WATCHED_PREFIX + filename, false);
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
            MainActivity current=liveActivity();
            if(current!=null) current.sharePublishedDownload(filename);
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
        changeQueued(filename, false);
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
        MainActivity current=liveActivity();
        return current!=null&&current.supportsPictureInPicture();
    }

    @JavascriptInterface
    public void enterPictureInPicture(int width, int height) {
        MainActivity current=liveActivity();
        if(current!=null) current.requestPictureInPicture(width, height);
    }

    @JavascriptInterface
    public void setRotationLocked(boolean locked) {
        MainActivity current=liveActivity();
        if(current!=null) current.setPlaybackRotationLocked(locked);
    }

    @JavascriptInterface
    public void setPlaybackState(boolean playing, int width, int height) {
        MainActivity current=liveActivity();
        if(current!=null) current.setPlaybackActive(playing, width, height);
    }

    private static boolean valid(String filename) {
        return filename != null && VIDEO_NAME.matcher(filename).matches();
    }
}
