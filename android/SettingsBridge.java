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
    private static final String ALLOW_SCREENSHOTS = "allow-screenshots";
    private static final String BACKGROUND_THEME = "background-theme";
    private static final String REDUCE_MOTION = "reduce-motion";
    private static final String SPACE_EFFECT = "space-effect";
    private static final String MOBILE_DOWNLOAD_POLICY = "mobile-download-policy";

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
            String requestedAppearance, boolean spaceEffect, String requestedBackground, boolean allowScreenshots, boolean reduceMotion, String mobilePolicy) {
        if (!"allow".equals(mobilePolicy) && !"ask".equals(mobilePolicy) && !"block".equals(mobilePolicy)) {
            return response(false, "Choose a supported mobile-data download policy");
        }
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
        if (!"space".equals(requestedBackground) && !"rainy-city".equals(requestedBackground)) {
            return response(false, "Choose a supported background theme");
        }
        boolean saved = preferences.edit()
                .putString(DOWNLOAD_FOLDER, folder)
                .putBoolean(KEEP_SCREEN_AWAKE, keepAwake)
                .putInt(DIAGNOSTICS_REFRESH_SECONDS, refreshSeconds)
                .putString(APPEARANCE, appearance)
                .putString(BACKGROUND_THEME, requestedBackground)
                .putBoolean(SPACE_EFFECT, spaceEffect)
                .putBoolean(ALLOW_SCREENSHOTS, allowScreenshots)
                .putBoolean(REDUCE_MOTION, reduceMotion)
                .putString(MOBILE_DOWNLOAD_POLICY, mobilePolicy)
                .commit();
        if (saved) {
            activity.applyPlaybackScreenPreference();
            activity.applyScreenshotPreference();
            activity.getContentResolver().notifyChange(
                    android.net.Uri.parse("content://" + activity.getPackageName() + ".preferences"), null);
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
                .remove(BACKGROUND_THEME)
                .remove(SPACE_EFFECT)
                .remove(ALLOW_SCREENSHOTS)
                .remove(REDUCE_MOTION)
                .remove(MOBILE_DOWNLOAD_POLICY)
                .commit();
        if (saved) {
            activity.applyPlaybackScreenPreference();
            activity.applyScreenshotPreference();
            activity.getContentResolver().notifyChange(
                    android.net.Uri.parse("content://" + activity.getPackageName() + ".preferences"), null);
            activity.applyAppearance("system");
        }
        if (saved) DownloadNetworkPolicy.get(activity).resetApproval();
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
    public String backgroundTheme() {
        return "rainy-city".equals(preferences.getString(BACKGROUND_THEME, "space")) ? "rainy-city" : "space";
    }

    @JavascriptInterface
    public boolean spaceEffectEnabled() {
        return preferences.getBoolean(SPACE_EFFECT, true);
    }

    @JavascriptInterface
    public boolean reduceMotionEnabled() {
        return preferences.getBoolean(REDUCE_MOTION, false);
    }

    @JavascriptInterface
    public String mobileDownloadPolicy() {
        String value = preferences.getString(MOBILE_DOWNLOAD_POLICY, "ask");
        return "allow".equals(value) || "block".equals(value) ? value : "ask";
    }

    @JavascriptInterface
    public boolean setMobileDownloadPolicy(String value) {
        if (!"allow".equals(value) && !"ask".equals(value) && !"block".equals(value)) return false;
        return preferences.edit().putString(MOBILE_DOWNLOAD_POLICY, value).commit();
    }

    @JavascriptInterface
    public int downloadNetworkState() { return DownloadNetworkPolicy.get(activity).state(); }

    @JavascriptInterface
    public void requestMobileDownloadApproval() { DownloadNetworkPolicy.get(activity).requestApproval(activity); }

    static boolean screenshotsAllowed(Context context) {
        return context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
                .getBoolean(ALLOW_SCREENSHOTS, false);
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
            result.put("allowScreenshots", screenshotsAllowed(activity));
            result.put("diagnosticsRefreshSeconds", diagnosticsRefreshSeconds());
            result.put("appearance", appearance());
            result.put("backgroundTheme", backgroundTheme());
            result.put("spaceEffectEnabled", spaceEffectEnabled());
            result.put("reduceMotion", reduceMotionEnabled());
            result.put("mobileDownloadPolicy", mobileDownloadPolicy());
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
