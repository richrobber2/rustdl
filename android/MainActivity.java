package app.rustdl;

import android.Manifest;
import android.app.Activity;
import android.app.PictureInPictureParams;
import android.content.ContentResolver;
import android.content.ContentValues;
import android.content.Intent;
import android.content.pm.ActivityInfo;
import android.content.pm.PackageManager;
import android.content.res.Configuration;
import android.database.Cursor;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.media.MediaCodec;
import android.media.MediaExtractor;
import android.media.MediaFormat;
import android.media.MediaMuxer;
import android.net.ConnectivityManager;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.net.Uri;
import android.os.BatteryManager;
import android.os.Bundle;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.os.PowerManager;
import android.os.StatFs;
import android.os.SystemClock;
import android.provider.MediaStore;
import android.util.Rational;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.FrameLayout;
import android.widget.ProgressBar;
import android.widget.Toast;

import org.json.JSONObject;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.ByteBuffer;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class MainActivity extends Activity {
    static final String OPEN_QUEUE_ACTION = "app.rustdl.action.OPEN_QUEUE";
    private static final String INSPECTION_ACTION = "app.rustdl.action.INSPECT";
    private static final String CAPTURE_INSPECTION_ACTION =
            "app.rustdl.action.CAPTURE_INSPECTION";
    private static final String INSPECTION_SCREEN = "app.rustdl.extra.SCREEN";
    private static final String INSPECTION_CAPTURE_NAME = "inspection-capture.png";
    private static final Pattern SUPPORTED_URL = Pattern.compile(
            "https?://(?:(?:www|mobile|m)\\.)?(?:x\\.com|twitter\\.com|youtube\\.com|youtu\\.be|snapchat\\.com)/\\S+",
            Pattern.CASE_INSENSITIVE);
    private static final String MEDIA_NAME_PATTERN =
            "(?:[0-9]+-[1-9][0-9]*|youtube-[A-Za-z0-9_-]{11}|snapchat-[A-Za-z0-9_-]{20,160})\\.(?:mp4|m4a)";

    static {
        System.loadLibrary("rustdl");
    }

    private final Handler handler = new Handler(Looper.getMainLooper());
    private FrameLayout root;
    private WebView webView;
    private ProgressBar progressBar;
    private View fullscreenView;
    private WebChromeClient.CustomViewCallback fullscreenCallback;
    private int previousSystemUiVisibility;
    private int previousWindowFlags;
    private String baseUrl;
    private boolean inspectionMode;
    private boolean captureInspection;
    private boolean captureScheduled;
    private File inspectionCapture;
    private UpdateManager updateManager;
    private PlaybackBridge playbackBridge;
    private DiagnosticsBridge diagnosticsBridge;
    private SettingsBridge settingsBridge;
    private ActivityBridge activityBridge;
    private boolean playbackActive;
    private ConnectivityManager connectivityManager;
    private ConnectivityManager.NetworkCallback networkCallback;
    private PowerManager powerManager;
    private PowerManager.OnThermalStatusChangedListener thermalListener;
    private long lastRuntimeTuningUpdate;

    private native void nativeStartServer(String bind, String outputDir, boolean inspectionMode);
    private native String nativeUpdateManifestUrl();
    private native boolean nativeSetPeerPairing(String address, String key);
    private native void nativeSetRuntimeTuning(boolean unmetered, boolean charging,
            boolean powerSave, int thermalStatus, long freeBytes, int processors);

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        String action = getIntent().getAction();
        inspectionMode = isDedicatedInspectionActivity();
        captureInspection = inspectionMode && CAPTURE_INSPECTION_ACTION.equals(action);
        if (!inspectionMode) {
            getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
            if (Build.VERSION.SDK_INT >= 33
                    && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)
                    != PackageManager.PERMISSION_GRANTED) {
                requestPermissions(
                        new String[]{Manifest.permission.POST_NOTIFICATIONS}, 2401);
            }
        }
        getWindow().setStatusBarColor(Color.rgb(9, 10, 15));
        getWindow().setNavigationBarColor(Color.rgb(9, 10, 15));

        root = new FrameLayout(this);
        webView = new WebView(this);
        progressBar = new ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal);
        progressBar.setMax(100);

        root.addView(webView, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT));
        FrameLayout.LayoutParams progressLayout = new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                dp(4));
        root.addView(progressBar, progressLayout);
        setContentView(root);

        if (captureInspection) {
            webView.setLayerType(View.LAYER_TYPE_SOFTWARE, null);
        }
        configureWebView();
        String bind = inspectionMode ? "127.0.0.1:37659" : "127.0.0.1:37658";
        baseUrl = "http://" + bind + "/";
        File videoCache = inspectionMode
                ? new File(getCacheDir(), "inspection-ui")
                : new File(getFilesDir(), "videos");
        if (!videoCache.isDirectory() && !videoCache.mkdirs()) {
            throw new IllegalStateException("Could not create the RustDL video cache");
        }
        if (captureInspection) {
            inspectionCapture = new File(videoCache, INSPECTION_CAPTURE_NAME);
            if (inspectionCapture.exists() && !inspectionCapture.delete()) {
                throw new IllegalStateException("Could not clear the previous inspection render");
            }
        }
        nativeStartServer(bind, videoCache.getAbsolutePath(), inspectionMode);
        if (!inspectionMode) {
            startRuntimeTuning();
            updateManager = new UpdateManager(this, root, nativeUpdateManifestUrl());
            updateManager.start();
        }
        handler.postDelayed(() -> loadInitialScreen(getIntent()), 300);
    }

    private void startRuntimeTuning() {
        connectivityManager = (ConnectivityManager) getSystemService(CONNECTIVITY_SERVICE);
        powerManager = (PowerManager) getSystemService(POWER_SERVICE);
        networkCallback = new ConnectivityManager.NetworkCallback() {
            @Override
            public void onAvailable(Network network) {
                handler.post(MainActivity.this::updateRuntimeTuning);
            }

            @Override
            public void onLost(Network network) {
                handler.post(MainActivity.this::updateRuntimeTuning);
            }

            @Override
            public void onCapabilitiesChanged(
                    Network network, NetworkCapabilities capabilities) {
                handler.post(MainActivity.this::updateRuntimeTuning);
            }
        };
        connectivityManager.registerDefaultNetworkCallback(networkCallback, handler);
        thermalListener = status -> handler.post(this::updateRuntimeTuning);
        powerManager.addThermalStatusListener(thermalListener);
        updateRuntimeTuning();
    }

    private void updateRuntimeTuning() {
        if (inspectionMode) return;
        boolean unmetered = false;
        if (connectivityManager != null) {
            Network network = connectivityManager.getActiveNetwork();
            NetworkCapabilities capabilities = network == null
                    ? null : connectivityManager.getNetworkCapabilities(network);
            unmetered = capabilities != null && capabilities.hasCapability(
                    NetworkCapabilities.NET_CAPABILITY_NOT_METERED);
        }
        BatteryManager battery = (BatteryManager) getSystemService(BATTERY_SERVICE);
        boolean charging = battery != null && battery.isCharging();
        boolean powerSave = powerManager != null && powerManager.isPowerSaveMode();
        int thermalStatus = powerManager == null
                ? PowerManager.THERMAL_STATUS_NONE : powerManager.getCurrentThermalStatus();
        long freeBytes = new StatFs(getFilesDir().getAbsolutePath()).getAvailableBytes();
        nativeSetRuntimeTuning(
                unmetered,
                charging,
                powerSave,
                thermalStatus,
                freeBytes,
                Runtime.getRuntime().availableProcessors());
        lastRuntimeTuningUpdate = SystemClock.elapsedRealtime();
    }

    int currentThermalStatus() {
        PowerManager manager = powerManager != null
                ? powerManager : (PowerManager) getSystemService(POWER_SERVICE);
        return manager == null
                ? PowerManager.THERMAL_STATUS_NONE : manager.getCurrentThermalStatus();
    }

    protected boolean isDedicatedInspectionActivity() {
        return false;
    }

    private void configureWebView() {
        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(!inspectionMode);
        settings.setDomStorageEnabled(false);
        settings.setAllowFileAccess(false);
        settings.setAllowContentAccess(false);
        settings.setMediaPlaybackRequiresUserGesture(false);
        settings.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        settings.setBuiltInZoomControls(false);
        settings.setSupportZoom(false);
        if (!inspectionMode) {
            settingsBridge = new SettingsBridge(this);
            webView.addJavascriptInterface(settingsBridge, "RustDLSettings");
            activityBridge = new ActivityBridge(this);
            webView.addJavascriptInterface(activityBridge, "RustDLActivity");
            playbackBridge = new PlaybackBridge(this);
            webView.addJavascriptInterface(playbackBridge, "RustDLPlayback");
            diagnosticsBridge = new DiagnosticsBridge(this);
            webView.addJavascriptInterface(diagnosticsBridge, "RustDLDiagnostics");
        }

        webView.setWebChromeClient(new WebChromeClient() {
            @Override
            public void onProgressChanged(WebView view, int progress) {
                progressBar.setProgress(progress);
                progressBar.setVisibility(progress >= 100 ? View.GONE : View.VISIBLE);
            }

            @Override
            public void onShowCustomView(
                    View view,
                    WebChromeClient.CustomViewCallback callback) {
                showFullscreenView(view, callback);
            }

            @Override
            public void onHideCustomView() {
                hideFullscreenView();
            }
        });
        webView.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
                Uri uri = request.getUrl();
                if (handleModeSwitch(uri)) {
                    return true;
                }
                if ("127.0.0.1".equals(uri.getHost())) {
                    return false;
                }
                startActivity(new Intent(Intent.ACTION_VIEW, uri));
                return true;
            }

            @Override
            public void onReceivedError(
                    WebView view,
                    WebResourceRequest request,
                    WebResourceError error) {
                if (request.isForMainFrame() && error.getErrorCode() == ERROR_CONNECT) {
                    handler.postDelayed(view::reload, 350);
                }
            }

            @Override
            public void onPageFinished(WebView view, String url) {
                if (!inspectionMode) {
                    dispatchRustEvent("{\"type\":\"sync\",\"version\":1}");
                }
                if (!captureInspection || captureScheduled || !isExpectedInspectionUrl(url)) {
                    return;
                }
                captureScheduled = true;
                view.postVisualStateCallback(System.nanoTime(), new WebView.VisualStateCallback() {
                    @Override
                    public void onComplete(long requestId) {
                        handler.postDelayed(() -> captureInspectionView(view, 0), 400);
                    }
                });
            }
        });
    }

    private boolean handleModeSwitch(Uri uri) {
        if (!"rustdl".equals(uri.getScheme()) || !"mode".equals(uri.getHost())) {
            return false;
        }
        if ("/inspection".equals(uri.getPath())) {
            Intent inspection = new Intent(INSPECTION_ACTION);
            inspection.setClass(this, InspectionActivity.class);
            inspection.putExtra(INSPECTION_SCREEN, "home");
            inspection.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            startActivity(inspection);
            return true;
        }
        if ("/normal".equals(uri.getPath())) {
            Intent normal = new Intent(Intent.ACTION_MAIN);
            normal.setClass(this, MainActivity.class);
            normal.addCategory(Intent.CATEGORY_LAUNCHER);
            normal.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP);
            startActivity(normal);
            if (inspectionMode) {
                finishAndRemoveTask();
            }
            return true;
        }
        return true;
    }

    private void reloadGalleryIfVisible() {
        if (webView == null) {
            return;
        }
        String currentUrl = webView.getUrl();
        if (currentUrl == null) {
            return;
        }
        Uri current = Uri.parse(currentUrl);
        if ("127.0.0.1".equals(current.getHost()) && "/".equals(current.getPath())) {
            webView.reload();
        }
    }

    boolean supportsPictureInPicture() {
        return !inspectionMode && getPackageManager().hasSystemFeature(
                PackageManager.FEATURE_PICTURE_IN_PICTURE);
    }

    void requestPictureInPicture(int width, int height) {
        Runnable enter = () -> {
            if (!supportsPictureInPicture() || isInPictureInPictureMode()) {
                return;
            }
            int safeWidth = Math.max(1, width);
            int safeHeight = Math.max(1, height);
            try {
                PictureInPictureParams params = new PictureInPictureParams.Builder()
                        .setAspectRatio(new Rational(safeWidth, safeHeight))
                        .build();
                enterPictureInPictureMode(params);
            } catch (IllegalArgumentException | IllegalStateException ignored) {
            }
        };
        if (Looper.myLooper() == Looper.getMainLooper()) {
            enter.run();
        } else {
            handler.post(enter);
        }
    }

    void setPlaybackRotationLocked(boolean locked) {
        handler.post(() -> {
            try {
                setRequestedOrientation(locked
                        ? ActivityInfo.SCREEN_ORIENTATION_LOCKED
                        : ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED);
            } catch (IllegalStateException ignored) {
            }
        });
    }

    void setPlaybackActive(boolean active) {
        handler.post(() -> {
            playbackActive = active;
            applyPlaybackScreenPreferenceNow();
            if (android.os.Build.VERSION.SDK_INT >= 31 && supportsPictureInPicture()) {
                try {
                    PictureInPictureParams params = new PictureInPictureParams.Builder()
                            .setAutoEnterEnabled(active)
                            .build();
                    setPictureInPictureParams(params);
                } catch (IllegalArgumentException | IllegalStateException ignored) {
                }
            }
        });
    }

    private void showFullscreenView(
            View view,
            WebChromeClient.CustomViewCallback callback) {
        if (fullscreenView != null) {
            callback.onCustomViewHidden();
            return;
        }

        fullscreenView = view;
        fullscreenCallback = callback;
        previousSystemUiVisibility = getWindow().getDecorView().getSystemUiVisibility();
        previousWindowFlags = getWindow().getAttributes().flags;

        view.setBackgroundColor(Color.BLACK);
        root.addView(view, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT));
        webView.setVisibility(View.GONE);
        progressBar.setVisibility(View.GONE);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN);
        applyPlaybackScreenPreferenceNow();
        applyFullscreenSystemUi();
    }

    private void applyFullscreenSystemUi() {
        getWindow().getDecorView().setSystemUiVisibility(
                View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                        | View.SYSTEM_UI_FLAG_FULLSCREEN
                        | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                        | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_LAYOUT_STABLE);
    }

    private void hideFullscreenView() {
        if (fullscreenView == null) {
            return;
        }

        root.removeView(fullscreenView);
        fullscreenView = null;
        webView.setVisibility(View.VISIBLE);
        getWindow().setFlags(
                previousWindowFlags,
                WindowManager.LayoutParams.FLAG_FULLSCREEN
                        | WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        getWindow().getDecorView().setSystemUiVisibility(previousSystemUiVisibility);
        applyPlaybackScreenPreferenceNow();

        if (fullscreenCallback != null) {
            fullscreenCallback.onCustomViewHidden();
            fullscreenCallback = null;
        }
    }

    void applyPlaybackScreenPreference() {
        handler.post(this::applyPlaybackScreenPreferenceNow);
    }

    private void applyPlaybackScreenPreferenceNow() {
        boolean keepAwake = playbackActive && settingsBridge != null
                && settingsBridge.keepScreenAwake();
        if (keepAwake) {
            getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        } else {
            getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        }
    }

    private boolean isExpectedInspectionUrl(String url) {
        Uri uri = Uri.parse(url);
        if (!"http".equals(uri.getScheme())
                || !"127.0.0.1".equals(uri.getHost())
                || uri.getPort() != 37659) {
            return false;
        }
        String screen = getIntent().getStringExtra(INSPECTION_SCREEN);
        String expectedPath;
        if ("result".equals(screen)) {
            expectedPath = "/__inspect/result";
        } else if ("player".equals(screen)) {
            expectedPath = "/__inspect/player";
        } else {
            expectedPath = "/";
        }
        return expectedPath.equals(uri.getPath());
    }

    private void captureInspectionView(WebView view, int attempt) {
        int width = view.getWidth();
        int height = view.getHeight();
        if ((width <= 0 || height <= 0) && attempt < 10) {
            handler.postDelayed(() -> captureInspectionView(view, attempt + 1), 150);
            return;
        }
        if (width <= 0 || height <= 0 || inspectionCapture == null) {
            return;
        }

        Bitmap bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888);
        Canvas canvas = new Canvas(bitmap);
        view.draw(canvas);
        File pending = new File(inspectionCapture.getParentFile(),
                INSPECTION_CAPTURE_NAME + ".part");
        try (FileOutputStream output = new FileOutputStream(pending)) {
            if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) {
                throw new IOException("Android could not encode the inspection render");
            }
            output.flush();
            output.getFD().sync();
            if (!pending.renameTo(inspectionCapture)) {
                throw new IOException("Could not publish the inspection render");
            }
        } catch (IOException error) {
            pending.delete();
        } finally {
            bitmap.recycle();
        }
    }

    private void loadIntent(Intent intent) {
        if (intent != null && Intent.ACTION_VIEW.equals(intent.getAction())) {
            Uri data = intent.getData();
            if (data != null && "rustdl".equals(data.getScheme())
                    && "pair".equals(data.getHost())) {
                String address = data.getQueryParameter("address");
                String key = data.getQueryParameter("key");
                if (address != null && key != null && nativeSetPeerPairing(address, key)) {
                    Toast.makeText(this, "RustDL devices paired", Toast.LENGTH_SHORT).show();
                    webView.loadUrl(baseUrl + "peers/connected");
                } else {
                    Toast.makeText(this, "That pairing code is invalid", Toast.LENGTH_SHORT).show();
                    webView.loadUrl(baseUrl + "peers/connected");
                }
                return;
            }
        }
        if (intent != null && OPEN_QUEUE_ACTION.equals(intent.getAction())) {
            webView.loadUrl(baseUrl + "queue");
            return;
        }
        String sharedUrls = extractSharedUrls(intent);
        if (sharedUrls == null) {
            webView.loadUrl(baseUrl);
            return;
        }
        Toast.makeText(this, "Adding shared video links…", Toast.LENGTH_SHORT).show();
        webView.loadUrl(baseUrl + "discover?source=" + Uri.encode(sharedUrls));
    }

    private void loadInitialScreen(Intent intent) {
        if (!inspectionMode) {
            loadIntent(intent);
            return;
        }
        String screen = intent.getStringExtra(INSPECTION_SCREEN);
        if ("result".equals(screen)) {
            webView.loadUrl(baseUrl + "__inspect/result");
        } else if ("player".equals(screen)) {
            webView.loadUrl(baseUrl + "__inspect/player");
        } else {
            webView.loadUrl(baseUrl);
        }
    }

    private String extractSharedUrls(Intent intent) {
        if (intent == null || !Intent.ACTION_SEND.equals(intent.getAction())) {
            return null;
        }
        CharSequence shared = intent.getCharSequenceExtra(Intent.EXTRA_TEXT);
        if (shared == null) {
            return null;
        }
        Matcher matcher = SUPPORTED_URL.matcher(shared);
        StringBuilder urls = new StringBuilder();
        while (matcher.find()) {
            String url = matcher.group();
            while (!url.isEmpty()
                    && ".,;:!?)]}".indexOf(url.charAt(url.length() - 1)) >= 0) {
                url = url.substring(0, url.length() - 1);
            }
            if (!url.isEmpty()) {
                if (urls.length() > 0) urls.append('\n');
                urls.append(url);
            }
        }
        return urls.length() == 0 ? null : urls.toString();
    }

    public void dispatchRustEvent(String eventJson) {
        if (inspectionMode || eventJson == null || eventJson.length() > 2_048) return;
        final String encoded;
        try {
            JSONObject event = new JSONObject(eventJson);
            String type = event.optString("type", "");
            if (!("queue".equals(type) || "peer".equals(type)
                    || "update".equals(type) || "activity".equals(type)
                    || "sync".equals(type))
                    || event.optInt("version", -1) != 1) {
                return;
            }
            encoded = JSONObject.quote(event.toString());
        } catch (Exception invalid) {
            return;
        }
        handler.post(() -> {
            if (webView == null || webView.getUrl() == null) return;
            Uri current = Uri.parse(webView.getUrl());
            if (!"127.0.0.1".equals(current.getHost()) || current.getPort() != 37658) return;
            webView.evaluateJavascript(
                    "(()=>{try{const detail=JSON.parse(" + encoded
                            + ");window.dispatchEvent(new CustomEvent('rustdl:state',{detail}))"
                            + "}catch(_error){}})();",
                    null);
        });
    }

    String activityCenterStatus() {
        try {
            JSONObject result = new JSONObject();
            result.put("ok", true);
            result.put("update", updateManager == null
                    ? JSONObject.NULL : new JSONObject(updateManager.activityStatus()));
            return result.toString();
        } catch (Exception unavailable) {
            return "{\"ok\":false,\"detail\":\"Native status unavailable\"}";
        }
    }

    public void updateTransferNotification(int count, long downloaded, long total) {
        handler.post(() -> {
            if (SystemClock.elapsedRealtime() - lastRuntimeTuningUpdate >= 5000L) {
                updateRuntimeTuning();
            }
            Intent service = new Intent(this, DownloadService.class);
            service.setAction(DownloadService.ACTION_UPDATE);
            service.putExtra(DownloadService.EXTRA_COUNT, count);
            service.putExtra(DownloadService.EXTRA_DOWNLOADED, downloaded);
            service.putExtra(DownloadService.EXTRA_TOTAL, total);
            if (count > 0) {
                if (android.os.Build.VERSION.SDK_INT >= 26) {
                    startForegroundService(service);
                } else {
                    startService(service);
                }
            } else {
                stopService(service);
            }
        });
    }

    public String watchedDownloads() {
        return playbackBridge == null ? "" : playbackBridge.watchedFilenames();
    }

    public synchronized void deletePublishedDownload(String displayName) {
        if (inspectionMode || displayName == null
                || !displayName.matches(MEDIA_NAME_PATTERN)) {
            throw new SecurityException("Invalid RustDL media deletion");
        }
        ContentResolver resolver = getContentResolver();
        Uri downloads = MediaStore.Downloads.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY);
        String selection = MediaStore.Downloads.DISPLAY_NAME + "=? AND "
                + MediaStore.Downloads.RELATIVE_PATH + "=?";
        resolver.delete(downloads, selection,
                new String[]{displayName, publishedDownloadPath(displayName)});
        getPreferences(MODE_PRIVATE).edit()
                .remove("published:" + displayName)
                .remove("published-path:" + displayName)
                .apply();
        if (playbackBridge != null) playbackBridge.forget(displayName);
    }

    public void sharePublishedDownload(String displayName) {
        if (inspectionMode || displayName == null
                || !displayName.matches(MEDIA_NAME_PATTERN)) {
            throw new SecurityException("Invalid RustDL media share");
        }
        handler.post(() -> {
            ContentResolver resolver = getContentResolver();
            Uri downloads = MediaStore.Downloads.getContentUri(
                    MediaStore.VOLUME_EXTERNAL_PRIMARY);
            String selection = MediaStore.Downloads.DISPLAY_NAME + "=? AND "
                    + MediaStore.Downloads.RELATIVE_PATH + "=?";
            Uri media = null;
            try (Cursor cursor = resolver.query(
                    downloads,
                    new String[]{MediaStore.Downloads._ID},
                    selection,
                    new String[]{displayName, publishedDownloadPath(displayName)},
                    null)) {
                if (cursor != null && cursor.moveToFirst()) {
                    media = Uri.withAppendedPath(downloads, Long.toString(cursor.getLong(0)));
                }
            }
            if (media == null) {
                Toast.makeText(this, "Finish downloading before sharing", Toast.LENGTH_SHORT)
                        .show();
                return;
            }
            boolean audioOnly = displayName.endsWith(".m4a");
            Intent share = new Intent(Intent.ACTION_SEND)
                    .setType(audioOnly ? "audio/mp4" : "video/mp4")
                    .putExtra(Intent.EXTRA_STREAM, media)
                    .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            startActivity(Intent.createChooser(share, audioOnly ? "Share audio" : "Share video"));
        });
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        loadInitialScreen(intent);
    }

    @Override
    public void onBackPressed() {
        if (fullscreenView != null) {
            hideFullscreenView();
            return;
        }
        if (webView.canGoBack()) {
            webView.goBack();
        } else {
            super.onBackPressed();
        }
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus && fullscreenView != null) {
            applyFullscreenSystemUi();
        }
    }

    @Override
    protected void onUserLeaveHint() {
        super.onUserLeaveHint();
        if (playbackActive && android.os.Build.VERSION.SDK_INT < 31) {
            requestPictureInPicture(16, 9);
        }
    }

    @Override
    public void onPictureInPictureModeChanged(
            boolean isInPictureInPictureMode, Configuration newConfig) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig);
        if (webView != null) {
            webView.evaluateJavascript(
                    "document.body.classList.toggle('pip',"
                            + (isInPictureInPictureMode ? "true" : "false") + ")",
                    null);
        }
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (!inspectionMode) {
            updateRuntimeTuning();
        }
        if (updateManager != null) {
            updateManager.onResume();
        }
    }

    @Override
    protected void onDestroy() {
        hideFullscreenView();
        if (connectivityManager != null && networkCallback != null) {
            try {
                connectivityManager.unregisterNetworkCallback(networkCallback);
            } catch (IllegalArgumentException ignored) {
            }
        }
        if (powerManager != null && thermalListener != null) {
            powerManager.removeThermalStatusListener(thermalListener);
        }
        if (updateManager != null) {
            updateManager.destroy();
        }
        if (playbackBridge != null) {
            webView.removeJavascriptInterface("RustDLPlayback");
        }
        if (diagnosticsBridge != null) {
            webView.removeJavascriptInterface("RustDLDiagnostics");
        }
        if (settingsBridge != null) {
            webView.removeJavascriptInterface("RustDLSettings");
        }
        if (activityBridge != null) {
            webView.removeJavascriptInterface("RustDLActivity");
        }
        webView.destroy();
        super.onDestroy();
    }

    public synchronized boolean publishDownload(String sourcePath, String displayName)
            throws IOException {
        if (inspectionMode) {
            throw new SecurityException("MediaStore is disabled in UI inspection mode");
        }
        String preferenceKey = "published:" + displayName;
        if (getPreferences(MODE_PRIVATE).getBoolean(preferenceKey, false)) {
            return true;
        }
        ContentResolver resolver = getContentResolver();
        Uri downloads = MediaStore.Downloads.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY);
        String downloadPath = settingsBridge == null
                ? SettingsBridge.DEFAULT_DOWNLOAD_PATH : settingsBridge.relativeDownloadPath();
        String selection = MediaStore.Downloads.DISPLAY_NAME + "=? AND "
                + MediaStore.Downloads.RELATIVE_PATH + "=?";
        String[] arguments = new String[]{displayName, downloadPath};
        try (Cursor cursor = resolver.query(
                downloads,
                new String[]{MediaStore.Downloads._ID},
                selection,
                arguments,
                null)) {
            if (cursor != null && cursor.moveToFirst()) {
                getPreferences(MODE_PRIVATE).edit()
                        .putBoolean(preferenceKey, true)
                        .putString("published-path:" + displayName, downloadPath)
                        .apply();
                return true;
            }
        }

        ContentValues values = new ContentValues();
        values.put(MediaStore.Downloads.DISPLAY_NAME, displayName);
        boolean audioOnly = displayName.endsWith(".m4a");
        values.put(MediaStore.Downloads.MIME_TYPE, audioOnly ? "audio/mp4" : "video/mp4");
        values.put(MediaStore.Downloads.RELATIVE_PATH, downloadPath);
        values.put(MediaStore.Downloads.IS_PENDING, 1);
        Uri destination = resolver.insert(downloads, values);
        if (destination == null) {
            throw new IOException("MediaStore refused to create the download");
        }

        try (InputStream input = new FileInputStream(sourcePath);
             OutputStream output = resolver.openOutputStream(destination, "w")) {
            if (output == null) {
                throw new IOException("MediaStore did not provide an output stream");
            }
            byte[] buffer = new byte[64 * 1024];
            int count;
            while ((count = input.read(buffer)) != -1) {
                output.write(buffer, 0, count);
            }
            output.flush();
        } catch (IOException error) {
            resolver.delete(destination, null, null);
            throw error;
        }

        ContentValues ready = new ContentValues();
        ready.put(MediaStore.Downloads.IS_PENDING, 0);
        resolver.update(destination, ready, null, null);
        getPreferences(MODE_PRIVATE).edit()
                .putBoolean(preferenceKey, true)
                .putString("published-path:" + displayName, downloadPath)
                .apply();
        handler.post(this::reloadGalleryIfVisible);
        return false;
    }

    private String publishedDownloadPath(String displayName) {
        return getPreferences(MODE_PRIVATE).getString(
                "published-path:" + displayName, SettingsBridge.DEFAULT_DOWNLOAD_PATH);
    }

    public boolean ensureThumbnail(String sourcePath, String displayName) {
        if (inspectionMode || sourcePath == null || displayName == null
                || !displayName.matches(MEDIA_NAME_PATTERN) || displayName.endsWith(".m4a")) {
            return false;
        }
        return ThumbnailManager.generate(new File(sourcePath), displayName);
    }

    public void muxDownloads(String videoPath, String audioPath, String outputPath)
            throws IOException {
        MediaExtractor video = new MediaExtractor();
        MediaExtractor audio = new MediaExtractor();
        MediaMuxer muxer = null;
        boolean started = false;
        try {
            video.setDataSource(videoPath);
            audio.setDataSource(audioPath);
            muxer = new MediaMuxer(outputPath, MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4);
            int videoTrack = addFirstTrack(video, muxer, "video/");
            int audioTrack = addFirstTrack(audio, muxer, "audio/");
            muxer.start();
            started = true;
            copySelectedTrack(video, muxer, videoTrack);
            copySelectedTrack(audio, muxer, audioTrack);
        } finally {
            if (muxer != null) {
                if (started) {
                    muxer.stop();
                }
                muxer.release();
            }
            video.release();
            audio.release();
        }
    }

    public void extractAudioTrack(String sourcePath, String outputPath) throws IOException {
        MediaExtractor source = new MediaExtractor();
        MediaMuxer muxer = null;
        boolean started = false;
        try {
            source.setDataSource(sourcePath);
            muxer = new MediaMuxer(outputPath, MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4);
            int audioTrack = addFirstTrack(source, muxer, "audio/");
            muxer.start();
            started = true;
            copySelectedTrack(source, muxer, audioTrack);
        } finally {
            if (muxer != null) {
                if (started) {
                    muxer.stop();
                }
                muxer.release();
            }
            source.release();
        }
    }

    private static int addFirstTrack(
            MediaExtractor extractor, MediaMuxer muxer, String mimePrefix) throws IOException {
        for (int index = 0; index < extractor.getTrackCount(); index++) {
            MediaFormat format = extractor.getTrackFormat(index);
            String mime = format.getString(MediaFormat.KEY_MIME);
            if (mime != null && mime.startsWith(mimePrefix)) {
                extractor.selectTrack(index);
                return muxer.addTrack(format);
            }
        }
        throw new IOException("Downloaded file has no " + mimePrefix + " track");
    }

    private static void copySelectedTrack(
            MediaExtractor extractor, MediaMuxer muxer, int destinationTrack) {
        ByteBuffer buffer = ByteBuffer.allocateDirect(4 * 1024 * 1024);
        MediaCodec.BufferInfo info = new MediaCodec.BufferInfo();
        while (true) {
            buffer.clear();
            int size = extractor.readSampleData(buffer, 0);
            if (size < 0) {
                break;
            }
            info.offset = 0;
            info.size = size;
            info.presentationTimeUs = extractor.getSampleTime();
            info.flags = extractor.getSampleFlags();
            muxer.writeSampleData(destinationTrack, buffer, info);
            extractor.advance();
        }
    }

    private int dp(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }
}
