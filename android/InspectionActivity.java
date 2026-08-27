package app.rustdl;

import android.os.Bundle;
import android.webkit.WebView;

public final class InspectionActivity extends MainActivity {
    private static boolean webViewDirectoryConfigured;

    @Override
    protected void onCreate(Bundle state) {
        configureWebViewDirectory();
        super.onCreate(state);
    }

    private static synchronized void configureWebViewDirectory() {
        if (webViewDirectoryConfigured) {
            return;
        }
        WebView.setDataDirectorySuffix("inspection");
        webViewDirectoryConfigured = true;
    }

    @Override
    protected boolean isDedicatedInspectionActivity() {
        return true;
    }
}
