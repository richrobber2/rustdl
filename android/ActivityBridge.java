package app.rustdl;

import android.webkit.JavascriptInterface;

/** Small native-only status surface for the WebView Activity Center. */
final class ActivityBridge {
    private final MainActivity activity;

    ActivityBridge(MainActivity activity) {
        this.activity = activity;
    }

    @JavascriptInterface
    public String status() {
        return activity.activityCenterStatus();
    }
}
