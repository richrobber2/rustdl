package app.rustdl;

import android.content.Context;
import android.content.SharedPreferences;
import android.os.Environment;
import android.webkit.JavascriptInterface;

import org.json.JSONException;
import org.json.JSONObject;

/** Persistent, APK-local preferences exposed only to RustDL's localhost WebView. */
final class SettingsBridge {
    static final String DEFAULT_DOWNLOAD_FOLDER = "RustDL";
    static final String DEFAULT_DOWNLOAD_PATH =
            Environment.DIRECTORY_DOWNLOADS + "/" + DEFAULT_DOWNLOAD_FOLDER + "/";
    private static final String PREFERENCES = "rustdl-settings";
    private static final String DOWNLOAD_FOLDER = "download-folder";
    private static final String KEEP_SCREEN_AWAKE = "keep-screen-awake";
    private static final String DIAGNOSTICS_REFRESH_SECONDS = "diagnostics-refresh-seconds";
    private static final String APPEARANCE = "appearance";
    private static final String SPACE_EFFECT = "space-effect";

    private final MainActivity activity;
    private final SharedPreferences preferences;

    SettingsBridge(MainActivity activity) {
        this.activity = activity;
        preferences = activity.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE);
    }

    @JavascriptInterface
    public String settings() {
        return response(true, "Settings loaded");
    }

    @JavascriptInterface
    public String save(String requestedFolder, boolean keepAwake, int refreshSeconds,
            String requestedAppearance, boolean spaceEffect) {
        String folder = normalizeFolder(requestedFolder);
        if (folder == null) {
            return response(false,
                    "Use 1–48 letters, numbers, spaces, periods, dashes, or underscores");
        }
        if (!validRefreshSeconds(refreshSeconds)) {
            return response(false, "Choose a supported diagnostics refresh interval");
        }
        String appearance = normalizeAppearance(requestedAppearance);
        if (appearance == null) {
            return response(false, "Choose system, light, or dark appearance");
        }
        boolean saved = preferences.edit()
                .putString(DOWNLOAD_FOLDER, folder)
                .putBoolean(KEEP_SCREEN_AWAKE, keepAwake)
                .putInt(DIAGNOSTICS_REFRESH_SECONDS, refreshSeconds)
                .putString(APPEARANCE, appearance)
                .putBoolean(SPACE_EFFECT, spaceEffect)
                .commit();
        if (saved) {
            activity.applyPlaybackScreenPreference();
            activity.applyAppearance(appearance);
        }
        return response(saved, saved ? "Settings saved" : "Android could not save settings");
    }

    @JavascriptInterface
    public String reset() {
        boolean saved = preferences.edit()
                .remove(DOWNLOAD_FOLDER)
                .remove(KEEP_SCREEN_AWAKE)
                .remove(DIAGNOSTICS_REFRESH_SECONDS)
                .remove(APPEARANCE)
                .remove(SPACE_EFFECT)
                .commit();
        if (saved) {
            activity.applyPlaybackScreenPreference();
            activity.applyAppearance("system");
        }
        return response(saved, saved ? "Defaults restored" : "Android could not reset settings");
    }

    @JavascriptInterface
    public int diagnosticsRefreshSeconds() {
        int value = preferences.getInt(DIAGNOSTICS_REFRESH_SECONDS, 5);
        return validRefreshSeconds(value) ? value : 5;
    }

    @JavascriptInterface
    public String appearance() {
        String value = preferences.getString(APPEARANCE, "system");
        String normalized = normalizeAppearance(value);
        return normalized == null ? "system" : normalized;
    }

    @JavascriptInterface
    public boolean setAppearance(String requestedAppearance) {
        String appearance = normalizeAppearance(requestedAppearance);
        boolean saved = appearance != null
                && preferences.edit().putString(APPEARANCE, appearance).commit();
        if (saved) activity.applyAppearance(appearance);
        return saved;
    }

    @JavascriptInterface
    public boolean spaceEffectEnabled() {
        return preferences.getBoolean(SPACE_EFFECT, true);
    }

    boolean keepScreenAwake() {
        return preferences.getBoolean(KEEP_SCREEN_AWAKE, true);
    }

    String relativeDownloadPath() {
        return Environment.DIRECTORY_DOWNLOADS + "/" + downloadFolder() + "/";
    }

    private String downloadFolder() {
        String stored = preferences.getString(DOWNLOAD_FOLDER, DEFAULT_DOWNLOAD_FOLDER);
        String normalized = normalizeFolder(stored);
        return normalized == null ? DEFAULT_DOWNLOAD_FOLDER : normalized;
    }

    private String response(boolean ok, String detail) {
        try {
            JSONObject result = new JSONObject();
            result.put("ok", ok);
            result.put("detail", detail);
            result.put("downloadFolder", downloadFolder());
            result.put("downloadPath", "Downloads/" + downloadFolder());
            result.put("keepScreenAwake", keepScreenAwake());
            result.put("diagnosticsRefreshSeconds", diagnosticsRefreshSeconds());
            result.put("appearance", appearance());
            result.put("spaceEffectEnabled", spaceEffectEnabled());
            return result.toString();
        } catch (JSONException impossible) {
            return "{\"ok\":false,\"detail\":\"Could not encode settings\"}";
        }
    }

    private static boolean validRefreshSeconds(int value) {
        return value == 3 || value == 5 || value == 10 || value == 30;
    }

    private static String normalizeAppearance(String value) {
        if (value == null) return null;
        String appearance = value.trim().toLowerCase();
        return appearance.equals("system") || appearance.equals("light")
                || appearance.equals("dark") ? appearance : null;
    }

    private static String normalizeFolder(String value) {
        if (value == null) return null;
        String folder = value.trim();
        if (folder.isEmpty() || folder.length() > 48
                || folder.equals(".") || folder.equals("..") || folder.charAt(0) == '.') {
            return null;
        }
        for (int index = 0; index < folder.length(); index++) {
            char character = folder.charAt(index);
            if (!Character.isLetterOrDigit(character)
                    && character != ' ' && character != '.'
                    && character != '-' && character != '_') {
                return null;
            }
        }
        return folder;
    }
}
