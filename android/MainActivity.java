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
import android.view.KeyEvent;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.window.OnBackInvokedCallback;
import android.window.OnBackInvokedDispatcher;
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
    static final String OPEN_NATIVE_PLAYER_ACTION="app.rustdl.action.OPEN_NATIVE_PLAYER";
    static final String OPEN_QUEUE_ACTION = "app.rustdl.action.OPEN_QUEUE";
    private static final String INSPECTION_ACTION = "app.rustdl.action.INSPECT";
    private static final String CAPTURE_INSPECTION_ACTION =
            "app.rustdl.action.CAPTURE_INSPECTION";
    private static final String INSPECTION_SCREEN = "app.rustdl.extra.SCREEN";
    private static final String INSPECTION_CAPTURE_NAME = "inspection-capture.png";
    private static final Pattern SUPPORTED_URL = Pattern.compile(
            "https?://(?:(?:www|mobile|m)\\.)?(?:x\\.com|twitter\\.com|youtube\\.com|youtu\\.be|snapchat\\.com|aniwaves\\.ru)/\\S+",
            Pattern.CASE_INSENSITIVE);
    private static final String MEDIA_NAME_PATTERN =
            "(?:[0-9]+-[1-9][0-9]*|youtube-[A-Za-z0-9_-]{11}|snapchat-[A-Za-z0-9_-]{20,160}|anime-[a-f0-9]{24})\\.(?:mp4|m4a)";

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
    private int screenshotGeneration;
    private File inspectionCapture;
    private UpdateManager updateManager;
    private PlaybackBridge playbackBridge;
    private DiagnosticsBridge diagnosticsBridge;
    private SettingsBridge settingsBridge;
    private ActivityBridge activityBridge;
    private boolean playbackActive;
    private int playbackWidth = 16;
    private int playbackHeight = 9;
    private ConnectivityManager connectivityManager;
    private ConnectivityManager.NetworkCallback networkCallback;
    private PowerManager powerManager;
    private PowerManager.OnThermalStatusChangedListener thermalListener;
    private long lastRuntimeTuningUpdate;
    private NativeHome nativeHome;
    private boolean nativeHomeVisible;
    private volatile boolean nativeQueueVisible;
    private volatile int nativeQueueOffset;
    private volatile boolean nativeDiagnosticsVisible;
    private volatile String lastNativeDiagnostics;
    private volatile boolean nativeLibraryVisible;
    private int nativeLibraryOffset;
    private String nativeLibraryLocation = "";
    private String nativeLibrarySearch = "";
    private boolean nativeLibraryContinueOnly;
    private boolean nativeLibraryUpNextOnly;
    private int nativeLibraryGeneration;
    private int nativeDiscoveryGeneration;
    private int nativePeerGeneration;
    private String nativePeerItem = "";
    private volatile int nativeAnimeGeneration;
    private String nativeAnimeView = "catalog", nativeAnimeSection = "newest", nativeAnimeQuery = "";
    private int nativeAnimePage = 1;
    private String nativeAnimeSelection = "";
    private int nativeStorageGeneration, nativeStorageOffset;
    private int nativeActivityGeneration, nativeActivityOffset;
    private int nativeStreamingGeneration;
    private String nativeStreamingWatch="",nativeStreamingToken="",nativeStreamingEpisode="";
    private int nativeStreamingOffset,nativeStreamingSourceOffset;
    private String nativeStreamingFilter="all";
    private boolean nativeStreamingPrivacy=true;
    private int nativePlaybackGeneration;
    private boolean nativePlaybackFullscreen,nativePlaybackPip,nativePlaybackAudioOnly;
    private int nativePlaybackVideoWidth=16,nativePlaybackVideoHeight=9;
    boolean nativePlaybackInPip() {return nativePlaybackPip;}
    private int nativePlaybackDecorFlags;
    private boolean nativePlaybackPlaying;
    private boolean nativePlaybackRotationLocked;
    private String nativePlaybackItem="";
    private boolean nativePlaybackCanSource;
    private String nativePlaybackFallback="";
    private String nativePlaybackFilename="";
    private final Runnable nativePlaybackRefresh=()->{
        if (!isDestroyed()&&this.activityResumed&&nativeHomeVisible&&nativeHome!=null&&NativePlaybackPolicy.playerScreen(nativeHome.screen()))
            nativeHome.playerCommand("snapshot",0);
    };
    private String nativeActivityFilter="all";
    private boolean activityResumed;
    private final Runnable nativeActivityRefresh=()->{
        if (!isDestroyed()&&activityResumed&&nativeHomeVisible&&nativeHome!=null&&nativeHome.screen()==10)
            requestNativeActivity(nativeActivityFilter,nativeActivityOffset);
    };
    private String nativeDiscoveryPreparationToken;
    private final Runnable nativeDiagnosticsRefresh = new Runnable() {
        @Override public void run() {
            if (isDestroyed() || !nativeHomeVisible || !activityResumed || !nativeDiagnosticsVisible || nativeHome == null) return;
            nativeHome.updateDiagnostics(nativeDiagnosticsSnapshot());
            handler.postDelayed(this, (settingsBridge == null ? 5 : settingsBridge.diagnosticsRefreshSeconds()) * 1000L);
        }
    };
    private final java.util.concurrent.ThreadPoolExecutor nativeActions =
            new java.util.concurrent.ThreadPoolExecutor(1, 1, 0L,
                    java.util.concurrent.TimeUnit.MILLISECONDS,
                    new java.util.concurrent.ArrayBlockingQueue<Runnable>(8));
    private final java.util.concurrent.ThreadPoolExecutor nativeAnimeReads =
            new java.util.concurrent.ThreadPoolExecutor(1, 1, 0L,
                    java.util.concurrent.TimeUnit.MILLISECONDS,
                    new java.util.concurrent.ArrayBlockingQueue<Runnable>(1));
    private boolean resetNativeHistory;
    private String completedLocalUrl;
    private int nativeTransferCount;
    private long nativeDownloaded;
    private long nativeTotal;
    private boolean nativeDark = true;

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
            applyScreenshotPreference();
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
        if (Build.VERSION.SDK_INT >= 33) {
            getOnBackInvokedDispatcher().registerOnBackInvokedCallback(
                    OnBackInvokedDispatcher.PRIORITY_DEFAULT, () -> onBackPressed());
        }
        WindowLayout.fitContent(this, root);

        if (captureInspection) {
            webView.setLayerType(View.LAYER_TYPE_SOFTWARE, null);
        }
        configureWebView();
        installNativeHome();
        if (settingsBridge != null) {
            applyAppearance(settingsBridge.appearance());
        }
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
        if (!inspectionMode) DownloadNetworkPolicy.get(this);
        nativeStartServer(bind, videoCache.getAbsolutePath(), inspectionMode);
        if (!inspectionMode) {
            android.content.IntentFilter filter = new android.content.IntentFilter(AnimeDownloadService.ACTION_SAVED);
            if (android.os.Build.VERSION.SDK_INT >= 33) registerReceiver(animeImportReceiver, filter, RECEIVER_NOT_EXPORTED);
            else registerReceiver(animeImportReceiver, filter);
            animeImportReceiverRegistered = true;
        }
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
        settings.setUseWideViewPort(true);
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
                progressBar.setVisibility(nativeHomeVisible || progress >= 100 ? View.GONE : View.VISIBLE);
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
            public void onPageStarted(WebView view, String url, Bitmap favicon) {
                screenshotGeneration++;
                if (!inspectionMode) getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
            }
            @Override
            public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
                Uri uri = request.getUrl();
                if (handleModeSwitch(uri)) {
                    return true;
                }
                if ("127.0.0.1".equals(uri.getHost())) {
                    return false;
                }
                if (request.isRedirect() || !request.hasGesture()) {
                    Toast.makeText(MainActivity.this, "External redirect blocked",
                            Toast.LENGTH_SHORT).show();
                    return true;
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
                if (baseUrl != null && url != null && url.startsWith(baseUrl)) completedLocalUrl = url;
                if (resetNativeHistory && url != null && url.startsWith(baseUrl)) {
                    view.clearHistory();
                    resetNativeHistory = false;
                }
                if (!inspectionMode) {
                    applyScreenshotPreference();
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
        if (!"rustdl".equals(uri.getScheme())) {
            return false;
        }
        if ("stream".equals(uri.getHost())) {
            if (inspectionMode) {
                Toast.makeText(this, "Streaming is unavailable in inspection mode",
                        Toast.LENGTH_SHORT).show();
                return true;
            }
            String url = uri.getQueryParameter("url");
            if (!StreamingActivity.isAllowedUrl(url)) {
                Toast.makeText(this, "That streaming URL is not supported",
                        Toast.LENGTH_SHORT).show();
                return true;
            }
            Intent streaming = new Intent(this, StreamingActivity.class);
            streaming.putExtra(StreamingActivity.EXTRA_URL, url);
            String requestedEpisode=uri.getQueryParameter("episode");
            if (requestedEpisode!=null&&!requestedEpisode.isEmpty()&&requestedEpisode.length()<=64)
                streaming.putExtra(StreamingActivity.EXTRA_EPISODE, requestedEpisode);
            streaming.putExtra(
                    StreamingActivity.EXTRA_MANIFEST_URL,
                    baseUrl + "__app/stream-manifest.json?url=" + Uri.encode(url));
            startActivity(streaming);
            return true;
        }
        if (!"mode".equals(uri.getHost())) {
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

    private final android.content.BroadcastReceiver animeImportReceiver = new android.content.BroadcastReceiver() {
        @Override public void onReceive(android.content.Context context, Intent intent) {
            refreshGalleryIfVisible();
        }
    };
    private boolean animeImportReceiverRegistered;

    private void refreshGalleryIfVisible() {
        if (webView == null) {
            return;
        }
        String currentUrl = webView.getUrl();
        if (currentUrl == null) {
            return;
        }
        Uri current = Uri.parse(currentUrl);
        if ("127.0.0.1".equals(current.getHost()) && current.getPort() == 37658) {
            webView.evaluateJavascript(
                    "window.dispatchEvent(new Event('rustdl:gallery'));", null);
        }
    }

    boolean supportsPictureInPicture() {
        return !inspectionMode && getPackageManager().hasSystemFeature(
                PackageManager.FEATURE_PICTURE_IN_PICTURE);
    }

    void requestPictureInPicture(int width, int height) {
        Runnable enter = () -> {
            if (settingsBridge != null && settingsBridge.screenshotRedactionEnabled()) return;
            if (!supportsPictureInPicture() || isInPictureInPictureMode()) {
                return;
            }
            int safeWidth = Math.max(1, width);
            int safeHeight = Math.max(1, height);
            playbackWidth = safeWidth;
            playbackHeight = safeHeight;
            try {
                PictureInPictureParams params = new PictureInPictureParams.Builder()
                        .setAspectRatio(new Rational(playbackWidth, playbackHeight))
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

    private void releaseNativeScreenshot(int generation, int retries) {
        root.postOnAnimation(() -> root.postOnAnimation(() -> {
            if (generation != screenshotGeneration || !nativeHomeVisible || nativeHome == null
                    || !SettingsBridge.screenshotsAllowed(this)
                    || fullscreenView != null || nativePlaybackFullscreen || nativePlaybackPip || isInPictureInPictureMode()) return;
            if (!settingsBridge.screenshotRedactionEnabled() || nativeHome.privacyReady()) {
                getWindow().clearFlags(WindowManager.LayoutParams.FLAG_SECURE);
            } else if (retries > 0) {
                handler.postDelayed(() -> releaseNativeScreenshot(generation, retries - 1), 100L);
            }
        }));
    }

    void applyScreenshotPreference() {
        runOnUiThread(() -> {
            if (inspectionMode) return;
            getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
            if (webView == null || settingsBridge == null) return;
            boolean allowed = SettingsBridge.screenshotsAllowed(this);
            boolean privacy = settingsBridge.screenshotRedactionEnabled();
            int generation = ++screenshotGeneration;
            if (nativeHomeVisible) {
                // Native media cards must acknowledge a redacted frame before capture.
                if (allowed) releaseNativeScreenshot(generation, 10);
                return;
            }
            webView.evaluateJavascript("(()=>{document.documentElement.dataset.inspectionPrivacy='"
                    + (privacy ? "on" : "off") + "';"
                    + "const probe=document.createElement('span');probe.className='media-title';"
                    + "document.body.append(probe);const hidden=getComputedStyle(probe).visibility==='hidden';"
                    + "probe.remove();return " + (!privacy ? "true" : "hidden") + ";})()", result -> {
                if (!allowed || !"true".equals(result)) return;
                webView.postVisualStateCallback(System.nanoTime(), new WebView.VisualStateCallback() {
                    @Override public void onComplete(long requestId) {
                        if (generation == screenshotGeneration
                                && SettingsBridge.screenshotsAllowed(MainActivity.this)
                                && settingsBridge.screenshotRedactionEnabled() == privacy
                                && (!privacy || fullscreenView == null)) {
                            getWindow().clearFlags(WindowManager.LayoutParams.FLAG_SECURE);
                        }
                    }
                });
            });
        });
    }

    void applyAppearance(String requestedAppearance) {
        handler.post(() -> {
            String appearance = requestedAppearance == null ? "system" : requestedAppearance;
            boolean light = "light".equals(appearance)
                    || ("system".equals(appearance)
                    && (getResources().getConfiguration().uiMode
                    & Configuration.UI_MODE_NIGHT_MASK) != Configuration.UI_MODE_NIGHT_YES);
            getWindow().setStatusBarColor(light ? Color.rgb(220, 230, 237) : Color.rgb(9, 10, 15));
            getWindow().setNavigationBarColor(light ? Color.rgb(220, 230, 237) : Color.rgb(9, 10, 15));
            View decor = getWindow().getDecorView();
            int flags = decor.getSystemUiVisibility();
            int lightBars = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR
                    | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR;
            decor.setSystemUiVisibility(light ? flags | lightBars : flags & ~lightBars);
            nativeDark = !light;
            if (nativeHome != null) {
                if(nativePlaybackPrivacy()) setNativePlaybackFullscreen(false);
                if(nativeHomeVisible&&nativeHome.screen()==13&&nativeStreamingPrivacy!=nativePlaybackPrivacy()&&!nativeStreamingToken.isEmpty()) requestNativeStreaming("page",nativeStreamingToken,nativeStreamingOffset);
                nativeHome.playerPrivacy(nativePlaybackPrivacy());
                if(nativePlaybackPrivacy()&&nativePlaybackMediaSession!=null) nativePlaybackMediaSession.redact();
                refreshNativePipActions();
                nativeHome.update(nativeDark, nativeTransferCount, nativeDownloaded, nativeTotal);
                if (settingsBridge != null) nativeHome.updateSettings(settingsBridge.settings());
            }
        });
    }

    private void installNativeHome() {
        if (inspectionMode) return;
        android.view.accessibility.AccessibilityManager accessibility =
                (android.view.accessibility.AccessibilityManager) getSystemService(ACCESSIBILITY_SERVICE);
        if (accessibility != null && accessibility.isTouchExplorationEnabled()) return;
        try {
            nativeHome = (NativeHome) Class.forName("app.rustdl.NativeHomeHost")
                    .getDeclaredConstructor(MainActivity.class).newInstance(this);
            root.addView(nativeHome.view(), 0, new FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        } catch (ClassNotFoundException standardBuild) {
            // Explicit compatibility APKs omit the native UI and its dependencies.
        } catch (ReflectiveOperationException | LinkageError unavailable) {
            nativeHome = null;
            Toast.makeText(this, "Native home unavailable; opening library", Toast.LENGTH_LONG).show();
        }
    }

    private boolean showNativeHome() {
        if (nativeHome == null) return false;
        nativeHomeVisible = true;
        if (updateManager!=null) updateManager.setNativePresentation(true);
        resetNativeHistory = false;
        webView.setVisibility(View.GONE);
        progressBar.setVisibility(View.GONE);
        nativeHome.setActive(activityResumed);
        int screen = nativeHome.screen();
        nativeScreenChanged(screen == 3 ? "queue" : screen == 4 ? "diagnostics"
                : screen == 5 ? "library" : screen==13?"streaming":screen==12?"player":screen==15?"mini-player":"home");
        if (screen == 5||screen==15) requestNativeLibrary(nativeLibraryOffset, nativeLibraryLocation);
        if (screen == 6) handler.post(this::refreshNativePreparation);
        if (screen == 7) requestNativePeer("snapshot", "{}");
        if (screen == 8) requestNativeAnime(nativeAnimeView, "", nativeAnimePage);
        if (screen == 9) requestNativeStorage("snapshot", "", nativeStorageOffset);
        if (screen == 10) requestNativeActivity(nativeActivityFilter,nativeActivityOffset);
        if (screen == 11) requestNativeUpdates("snapshot");
        if (NativePlaybackPolicy.playerScreen(screen)) {nativeHome.playerVisible(activityResumed);nativeHome.playerCommand("snapshot",0);}
        if (screen == 3) nativeHome.updateQueue(nativeQueueSnapshot(nativeQueueOffset));
        if (settingsBridge != null) applyAppearance(settingsBridge.appearance());
        applyScreenshotPreference();
        handler.postDelayed(() -> {
            if (isDestroyed() || !nativeHomeVisible || nativeHome == null || nativeHome.ready()) return;
            nativeHome.destroy();
            root.removeView(nativeHome.view());
            nativeHome = null;
            showWebContent();
            webView.loadUrl(baseUrl);
            Toast.makeText(this, "Native home unavailable; opening library", Toast.LENGTH_LONG).show();
        }, 10000);
        return true;
    }

    private void showWebContent() {
        if(nativePlaybackMediaSession!=null) nativePlaybackMediaSession.deactivate();
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
        ++screenshotGeneration;
        nativeHomeVisible = false;
        if (updateManager!=null) updateManager.setNativePresentation(false);
        nativeQueueVisible = false;
        nativeDiagnosticsVisible = false;
        nativeLibraryVisible = false;
        handler.removeCallbacks(nativeDiagnosticsRefresh);
        if (nativeHome != null) nativeHome.setActive(false);
        webView.setVisibility(View.VISIBLE);
        webView.requestFocus();
        applyScreenshotPreference();
    }

    /** Called by the native command surface; never accepts an arbitrary URL. */
    public void openNativeDestination(String destination) {
        final String path = NativeHomeRoutes.path(destination);
        if (path == null) return;
        handler.post(() -> {
            if (isFinishing() || isDestroyed() || nativeHome == null) return;
            showWebContent();
            String target = baseUrl + path;
            if (!target.equals(webView.getUrl()) || !target.equals(completedLocalUrl)) {
                resetNativeHistory = true;
                webView.loadUrl(target);
            } else {
                webView.clearHistory();
            }
        });
    }

    public void nativeScreenChanged(String screen) {
        if (!"player".equals(screen)&&!"mini-player".equals(screen)&&nativeHome!=null) {
            nativeHome.playerVisible(false);nativeHome.playerCommand("pause",0);resetNativePlaybackRotation();handler.removeCallbacks(nativePlaybackRefresh);
            if(nativePlaybackMediaSession!=null) nativePlaybackMediaSession.deactivate();
        }
        if (!"activity".equals(screen)) handler.removeCallbacks(nativeActivityRefresh);
        nativeQueueVisible = "queue".equals(screen);
        nativeDiagnosticsVisible = "diagnostics".equals(screen);
        nativeLibraryVisible = "library".equals(screen)||"mini-player".equals(screen);
        handler.post(() -> {
            handler.removeCallbacks(nativeDiagnosticsRefresh);
            if (nativeDiagnosticsVisible) handler.post(nativeDiagnosticsRefresh);
        });
    }

    public void requestNativeLibrary(int offset, String location) {
        loadNativeLibrary(offset, location, 2);
    }

    private void loadNativeLibrary(int offset, String location, int thumbnailRetries) {
        if (offset < 0 || location == null || !(location.isEmpty() || location.matches("[a-f0-9]{32}"))) return;
        handler.post(() -> {
            if (isDestroyed() || nativeHome == null || !nativeLibraryVisible) return;
            nativeLibraryOffset = offset;
            nativeLibraryLocation = location;
            final int generation = ++nativeLibraryGeneration;
            final String search = nativeLibrarySearch;
            final boolean continueOnly = nativeLibraryContinueOnly;
            final boolean upNextOnly = nativeLibraryUpNextOnly;
            final boolean privacy = settingsBridge == null || settingsBridge.inspectionPrivacyEnabled();
            try {
                nativeActions.execute(() -> {
                    String data = decorateNativeLibraryPlayback(nativeLibraryData(offset, location, search, privacy, nativeResumeSelection(continueOnly, upNextOnly)));
                    handler.post(() -> {
                        if (isDestroyed() || nativeHome == null || !nativeHomeVisible || !nativeLibraryVisible
                                || generation != nativeLibraryGeneration) return;
                        boolean currentPrivacy = settingsBridge == null || settingsBridge.inspectionPrivacyEnabled();
                        if (privacy != currentPrivacy) {
                            loadNativeLibrary(offset, location, thumbnailRetries);
                            return;
                        }
                        nativeHome.updateLibrary(data);
                        if (!privacy && thumbnailRetries > 0) {
                            handler.postDelayed(() -> {
                                if (generation == nativeLibraryGeneration && nativeLibraryVisible && !isDestroyed())
                                    loadNativeLibrary(offset, location, thumbnailRetries - 1);
                            }, 1200L);
                        }
                    });
                });
            } catch (java.util.concurrent.RejectedExecutionException busy) {
                nativeHome.updateLibrary("{\"ok\":false,\"detail\":\"Library is busy. Try again.\",\"title\":\"Library\",\"location\":\"\",\"offset\":0,\"total\":0,\"items\":[],\"nextOffset\":null,\"previousOffset\":null}");
            }
        });
    }

    public String nativeLibraryThumbnailPath(String id) {
        if (id == null || !id.matches("[a-f0-9]{32}") || settingsBridge == null
                || settingsBridge.inspectionPrivacyEnabled()) return "";
        return nativeLibraryThumbnail(id, false);
    }

    public void searchNativeLibrary() {
        handler.post(() -> {
            if (isDestroyed() || nativeHome == null) return;
            android.widget.EditText field = new android.widget.EditText(this);
            field.setSingleLine(true);
            field.setFilters(new android.text.InputFilter[]{new android.text.InputFilter.LengthFilter(128)});
            field.setText(nativeLibrarySearch);
            android.widget.Spinner filter = new android.widget.Spinner(this);
            filter.setAdapter(new android.widget.ArrayAdapter<String>(this,
                    android.R.layout.simple_spinner_dropdown_item,
                    new String[]{"All library items", "Continue watching", "Up Next"}));
            filter.setSelection(nativeLibraryUpNextOnly ? 2 : nativeLibraryContinueOnly ? 1 : 0);
            android.widget.LinearLayout fields = new android.widget.LinearLayout(this);
            fields.setOrientation(android.widget.LinearLayout.VERTICAL);
            fields.addView(field);
            fields.addView(filter);
            new android.app.AlertDialog.Builder(this).setTitle("Search and filter library").setView(fields)
                    .setNegativeButton("Cancel", null)
                    .setNeutralButton("Clear", (dialog, which) -> {
                        nativeLibrarySearch = "";
                        nativeLibraryContinueOnly = false;
                        nativeLibraryUpNextOnly = false;
                        requestNativeLibrary(0, nativeLibraryLocation);
                    })
                    .setPositiveButton("Search", (dialog, which) -> {
                        nativeLibrarySearch = field.getText().toString();
                        nativeLibraryContinueOnly = filter.getSelectedItemPosition() == 1;
                        nativeLibraryUpNextOnly = filter.getSelectedItemPosition() == 2;
                        if (nativeLibraryContinueOnly || nativeLibraryUpNextOnly) nativeLibraryLocation = "";
                        requestNativeLibrary(0, nativeLibraryLocation);
                    }).show();
        });
    }

    private String nativeResumeSelection(boolean enabled, boolean upNext) {
        if (upNext) return "up-next:" + (playbackBridge == null ? "[]" : playbackBridge.getPlaybackQueue());
        if (!enabled) return "";
        if (playbackBridge == null) return "[]";
        try {
            org.json.JSONArray records = new org.json.JSONArray(playbackBridge.getContinueWatching());
            java.util.ArrayList<org.json.JSONObject> sorted = new java.util.ArrayList<>();
            for (int i = 0; i < records.length(); i++) sorted.add(records.getJSONObject(i));
            java.util.Collections.sort(sorted, (left, right) ->
                    Long.compare(right.optLong("updated"), left.optLong("updated")));
            org.json.JSONArray names = new org.json.JSONArray();
            for (org.json.JSONObject record : sorted) {
                String name = record.optString("filename");
                if (name.matches(MEDIA_NAME_PATTERN)) names.put(name);
            }
            return names.toString();
        } catch (org.json.JSONException invalid) {
            return "[]";
        }
    }

    private String decorateNativeLibraryPlayback(String data) {
        if (playbackBridge == null) return data;
        try {
            org.json.JSONObject page = new org.json.JSONObject(data);
            org.json.JSONArray items = page.optJSONArray("items");
            if (items == null) return data;
            for (int i = 0; i < items.length(); i++) {
                org.json.JSONObject item = items.getJSONObject(i);
                String filename = nativeLibraryMedia(item.optString("id"));
                if (filename == null || !filename.matches(MEDIA_NAME_PATTERN)) continue;
                double position = playbackBridge.getPosition(filename);
                double duration = playbackBridge.getDuration(filename);
                if (Double.isFinite(position) && Double.isFinite(duration)
                        && position >= 5d && duration > 0d && position < duration - 5d) {
                    item.put("positionSeconds", position);
                    item.put("durationSeconds", duration);
                }
                item.put("watched", playbackBridge.isWatched(filename));
                org.json.JSONArray queued = new org.json.JSONArray(playbackBridge.getPlaybackQueue());
                boolean inQueue = false;
                for (int j = 0; j < queued.length(); j++) if (filename.equals(queued.optString(j))) inQueue = true;
                item.put("queued", inQueue);
            }
            return page.toString();
        } catch (org.json.JSONException invalid) {
            return data;
        }
    }

    public void changeNativeLibraryItem(String id, String action) {
        if (id == null || !id.matches("[a-f0-9]{32}") || action == null) return;
        if (!("pip".equals(action)||"play".equals(action) || "share".equals(action) || "watched".equals(action)
                || "delete".equals(action) || "enqueue".equals(action) || "dequeue".equals(action)
                || "source".equals(action) || "quality".equals(action) || "send".equals(action))) return;
        handler.post(() -> {
            if (isDestroyed() || nativeHome == null) return;
            String filename = nativeLibraryMedia(id);
            if (filename == null || !filename.matches(MEDIA_NAME_PATTERN)) return;
            switch (action) {
                case "send":
                    nativePeerItem = id;
                    nativeHome.showPeers();
                    requestNativePeer("snapshot", "{}");
                    break;
                case "source":
                case "quality":
                    String source = nativeLibrarySource(id);
                    if (source == null || source.isEmpty()) return;
                    if ("source".equals(action)) {
                        try {startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(source)));}
                        catch (android.content.ActivityNotFoundException unavailable) {
                            Toast.makeText(this, "No browser is available", Toast.LENGTH_SHORT).show();
                        }
                    } else {
                        try {
                            JSONObject payload = new JSONObject(); payload.put("source", source);
                            nativeHome.showDiscovery();
                            requestNativeDiscovery("start", payload.toString());
                        } catch (org.json.JSONException invalid) { }
                    }
                    break;
                case "play": openNativePlayback(filename); break;
                case "enqueue":
                case "dequeue":
                    if (playbackBridge != null) playbackBridge.changeQueued(filename, "enqueue".equals(action));
                    requestNativeLibrary(nativeLibraryOffset, nativeLibraryLocation);
                    break;
                case "share": sharePublishedDownload(filename); break;
                case "watched":
                    if (playbackBridge != null) playbackBridge.markWatched(filename);
                    Toast.makeText(this, "Marked as watched", Toast.LENGTH_SHORT).show();
                    requestNativeLibrary(nativeLibraryOffset, nativeLibraryLocation);
                    break;
                case "delete":
                    new android.app.AlertDialog.Builder(this).setTitle("Delete downloaded item?")
                        .setMessage("This permanently removes this item and its published download.")
                        .setNegativeButton("Cancel", null)
                        .setPositiveButton("Delete", (dialog, which) -> {
                            try {
                                nativeActions.execute(() -> {
                                    boolean ok = nativeLibraryDelete(id);
                                    handler.post(() -> {
                                        if (isDestroyed()) return;
                                        Toast.makeText(this, ok ? "Item deleted" : "Could not delete this item. Pause active downloads first.", Toast.LENGTH_LONG).show();
                                        requestNativeLibrary(nativeLibraryOffset, nativeLibraryLocation);
                                    });
                                });
                            } catch (java.util.concurrent.RejectedExecutionException busy) {
                                Toast.makeText(this, "Library is busy. Try again.", Toast.LENGTH_SHORT).show();
                            }
                        }).show();
                    break;
            }
        });
    }

    public void inputNativeDiscovery() {
        handler.post(() -> {
            if (isDestroyed() || nativeHome == null || nativeHome.screen() != 6) return;
            android.widget.EditText field = new android.widget.EditText(this);
            field.setInputType(android.text.InputType.TYPE_CLASS_TEXT | android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE);
            field.setFilters(new android.text.InputFilter[]{new android.text.InputFilter.LengthFilter(16384)});
            field.setHint("Paste supported video links");
            new android.app.AlertDialog.Builder(this).setTitle("Discover downloads").setView(field)
                .setNegativeButton("Cancel", null).setPositiveButton("Discover", (dialog, which) -> {
                    try {
                        JSONObject payload = new JSONObject();
                        payload.put("source", field.getText().toString());
                        requestNativeDiscovery("start", payload.toString());
                    } catch (org.json.JSONException invalid) { }
                }).show();
        });
    }

    public void requestNativeDiscovery(String command, String payload) {
        if (!("start".equals(command) || "page".equals(command) || "import".equals(command)
                || "prepare".equals(command) || "status".equals(command) || "cancel".equals(command)
                || "formats".equals(command) || "bulk".equals(command) || "retry".equals(command))
                || payload == null || payload.length() > 131072) return;
        handler.post(() -> {
            if (isDestroyed() || nativeHome == null || !nativeHomeVisible || nativeHome.screen() != 6) return;
            final int generation = ++nativeDiscoveryGeneration;
            final boolean privacy = settingsBridge == null || settingsBridge.inspectionPrivacyEnabled();
            if (!"status".equals(command)) nativeHome.updateDiscovery("{\"ok\":true,\"kind\":\"loading\"}");
            try {
                nativeActions.execute(() -> {
                    String data = nativeDiscoveryCommand(command, payload, privacy);
                    handler.post(() -> {
                        if (isDestroyed() || nativeHome == null || !nativeHomeVisible
                                || nativeHome.screen() != 6 || generation != nativeDiscoveryGeneration) return;
                        boolean currentPrivacy = settingsBridge == null || settingsBridge.inspectionPrivacyEnabled();
                        if (privacy != currentPrivacy && "bulk".equals(command)) {
                            requestNativeDiscovery(command, payload);
                            return;
                        }
                        if (privacy != currentPrivacy && !"import".equals(command)) {
                            try {
                                JSONObject page = new JSONObject(data);
                                String token = page.optString("token");
                                if (!token.isEmpty() && ("qualities".equals(page.optString("kind"))
                                        || "playlist".equals(page.optString("kind")))) {
                                    JSONObject next = new JSONObject(); next.put("token", token);
                                    next.put("offset", page.optInt("offset"));
                                    requestNativeDiscovery("page", next.toString());
                                    return;
                                }
                            } catch (org.json.JSONException invalid) { }
                        }
                        nativeHome.updateDiscovery(data);
                        try {
                            JSONObject state = new JSONObject(data);
                            if ("preparation".equals(state.optString("kind"))
                                    && state.optBoolean("finished") && !state.optBoolean("cancelled")
                                    && state.optBoolean("canContinue") && state.optInt("issueCount") == 0) {
                                nativeDiscoveryPreparationToken = null;
                                JSONObject next = new JSONObject(); next.put("token", state.optString("token"));
                                requestNativeDiscovery("formats", next.toString());
                                return;
                            }
                            if ("preparation".equals(state.optString("kind"))
                                    && !state.optBoolean("finished")) {
                                nativeDiscoveryPreparationToken = state.optString("token");
                                handler.postDelayed(() -> {
                                    if (generation == nativeDiscoveryGeneration && activityResumed)
                                        refreshNativePreparation();
                                }, 1000L);
                            } else nativeDiscoveryPreparationToken = null;
                        } catch (org.json.JSONException invalid) {nativeDiscoveryPreparationToken = null;}
                    });
                });
            } catch (java.util.concurrent.RejectedExecutionException busy) {
                nativeHome.updateDiscovery("{\"ok\":false,\"detail\":\"Discovery is busy. Try again.\"}");
            }
        });
    }

    private void refreshNativePreparation() {
        if (nativeDiscoveryPreparationToken == null || !activityResumed || nativeHome == null
                || !nativeHomeVisible || nativeHome.screen() != 6 || isDestroyed()) return;
        try {
            JSONObject request = new JSONObject(); request.put("token", nativeDiscoveryPreparationToken);
            requestNativeDiscovery("status", request.toString());
        } catch (org.json.JSONException invalid) { }
    }

    public void pairNativePeer() {
        handler.post(() -> {
            if (isDestroyed() || nativeHome == null || nativeHome.screen() != 7) return;
            android.widget.EditText address = new android.widget.EditText(this);
            address.setHint("Receiver address"); address.setSingleLine(true);
            address.setFilters(new android.text.InputFilter[]{new android.text.InputFilter.LengthFilter(2048)});
            android.widget.EditText key = new android.widget.EditText(this);
            key.setHint("Pairing key"); key.setSingleLine(true);
            key.setInputType(android.text.InputType.TYPE_CLASS_TEXT | android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD);
            key.setFilters(new android.text.InputFilter[]{new android.text.InputFilter.LengthFilter(64)});
            android.widget.LinearLayout fields = new android.widget.LinearLayout(this);
            fields.setOrientation(android.widget.LinearLayout.VERTICAL);fields.addView(address);fields.addView(key);
            new android.app.AlertDialog.Builder(this).setTitle("Pair with receiver").setView(fields)
                .setNegativeButton("Cancel",null).setPositiveButton("Pair",(dialog,which) -> {
                    try {JSONObject request=new JSONObject();request.put("address",address.getText().toString());
                        request.put("key",key.getText().toString());requestNativePeer("pair",request.toString());
                    } catch (org.json.JSONException invalid) { }
                }).show();
        });
    }
    PlaybackBridge nativePlaybackPreferences() {return playbackBridge;}
    String nativePlaybackOrigin() {return baseUrl;}
    boolean nativePlaybackPrivacy() {return settingsBridge==null||settingsBridge.inspectionPrivacyEnabled();}

    private void openNativePlayback(String filename) {
        if (nativeHome==null||filename==null||!filename.matches(MEDIA_NAME_PATTERN)) return;
        int generation=++nativePlaybackGeneration;
        nativePlaybackFilename=filename;nativePlaybackFallback="";nativePlaybackItem="";nativePlaybackCanSource=false;
        nativeHome.playerVisible(false);nativeHome.playerCommand("pause",0);
        nativeHome.playerPrivacy(nativePlaybackPrivacy());nativeHome.showPlayer();
        nativeHome.updatePlayer("{\"state\":\"loading\",\"framesHidden\":true}");
        try {nativeActions.execute(()->{
            String route=nativePlaybackRoute(filename);
            String fallback=nativePlaybackNext(filename);
            String item=nativePlaybackHandle(filename);
            String source=item.isEmpty()?"":nativeLibrarySource(item);
            boolean canSource=source!=null&&!source.isEmpty();
            handler.post(()->{
                if (isDestroyed()||nativeHome==null||!NativePlaybackPolicy.playerScreen(nativeHome.screen())||generation!=nativePlaybackGeneration) return;
                if (!("media".equals(route)||"stream".equals(route))) {nativeHome.updatePlayer("{\"state\":\"error\",\"detail\":\"Playback unavailable\"}");return;}
                nativePlaybackFallback=fallback;nativePlaybackItem=item;nativePlaybackCanSource=canSource;
                playbackBridge.changeQueued(filename,false);
                nativeHome.openPlayer(filename,baseUrl+route+"/"+Uri.encode(filename));
            });
        });}catch(java.util.concurrent.RejectedExecutionException busy){nativeHome.updatePlayer("{\"state\":\"error\",\"detail\":\"Playback is busy. Try again.\"}");}
    }

    private NativePlaybackMediaSession nativePlaybackMediaSession;
    // Internal service selection; media identity never enters the GPUI snapshot DTO.
    void nativePlaybackSelection(String filename) {
        if(isDestroyed()||filename==null||filename.equals(nativePlaybackFilename)) return;
        if(!filename.isEmpty()&&!filename.matches(MEDIA_NAME_PATTERN)) return;
        nativePlaybackFilename=filename;
        final int generation=++nativePlaybackGeneration;
        nativePlaybackFallback="";nativePlaybackItem="";nativePlaybackCanSource=false;
        if(filename.isEmpty()) return;
        try {nativeActions.execute(()->{
            String fallback=nativePlaybackNext(filename),item=nativePlaybackHandle(filename);
            String source=item.isEmpty()?"":nativeLibrarySource(item);
            handler.post(()->{
                if(isDestroyed()||generation!=nativePlaybackGeneration) return;
                nativePlaybackFallback=fallback;nativePlaybackItem=item;nativePlaybackCanSource=source!=null&&!source.isEmpty();
            });
        });}catch(java.util.concurrent.RejectedExecutionException busy) {}
    }

    void nativePlaybackSnapshot(String data) {
        if (isDestroyed()||nativeHome==null||!nativeHomeVisible||!NativePlaybackPolicy.playerScreen(nativeHome.screen())) return;
        try {JSONObject snapshot=new JSONObject(data);nativePlaybackPlaying=snapshot.optBoolean("playing");refreshNativePipActions();refreshNativePipSurface();nativePlaybackAudioOnly=snapshot.optBoolean("audioOnly");nativePlaybackVideoWidth=snapshot.optInt("videoWidth",16);nativePlaybackVideoHeight=snapshot.optInt("videoHeight",9);snapshot.put("canPip",NativePlaybackPolicy.allowPip(nativePlaybackPrivacy(),nativeHomeVisible&&activityResumed,isDestroyed(),supportsPictureInPicture(),nativePlaybackAudioOnly,nativePlaybackPlaying));applyPlaybackScreenPreferenceNow();snapshot.put("canSource",nativePlaybackCanSource);snapshot.put("canSend",!nativePlaybackItem.isEmpty());snapshot.put("fullscreen",nativePlaybackFullscreen);snapshot.put("rotationLocked",nativePlaybackRotationLocked);snapshot.put("canNext",snapshot.optBoolean("canNext")||!nativePlaybackFallback.isEmpty());data=snapshot.toString();
            // Transport metadata and controls are owned by the foreground playback service.
        }catch(Exception invalid) {}
        nativeHome.updatePlayer(data);handler.removeCallbacks(nativePlaybackRefresh);
        if (activityResumed) handler.postDelayed(nativePlaybackRefresh,1000L);
    }

    public void requestNativePlayer(String action,double value) {
        if (action==null||!Double.isFinite(value)||!("play".equals(action)||"pause".equals(action)||"clear-queue".equals(action)||"source".equals(action)||"quality".equals(action)||"send".equals(action)||"share".equals(action)||"retry".equals(action)||"fullscreen".equals(action)||"seek-dialog".equals(action)||"seek".equals(action)
                ||"speed".equals(action)||"volume".equals(action)||"rotation".equals(action)||"sleep".equals(action)||"snapshot".equals(action)||"next".equals(action)||"mini".equals(action)||"expand".equals(action)||"close-mini".equals(action)||"stop".equals(action)||"seek-by".equals(action)||"toggle".equals(action)||"preview".equals(action)||"preview-hide".equals(action))) return;
        handler.post(()->{
            if (isDestroyed()||nativeHome==null||!nativeHomeVisible||!NativePlaybackPolicy.playerScreen(nativeHome.screen())) return;
            if("stop".equals(action)) {
                nativeHome.playerVisible(false);nativeHome.playerCommand("stop",0);nativePlaybackPlaying=false;
                nativeHome.playerLayout(2);if(nativePlaybackMediaSession!=null) nativePlaybackMediaSession.deactivate();
                requestNativeLibrary(nativeLibraryOffset,nativeLibraryLocation);return;
            }
            if ("mini".equals(action)||"expand".equals(action)||"close-mini".equals(action)) {
                setNativePlaybackFullscreen(false);
                if("close-mini".equals(action)) {nativeHome.playerVisible(false);nativeHome.playerCommand("stop",0);nativePlaybackPlaying=false;}
                nativeHome.playerLayout("mini".equals(action)?1:"expand".equals(action)?0:2);
                if(!"expand".equals(action)) requestNativeLibrary(nativeLibraryOffset,nativeLibraryLocation);
                if(!"close-mini".equals(action)) nativeHome.playerCommand("snapshot",0);
                return;
            }
            if ("clear-queue".equals(action)) {playbackBridge.savePlaybackQueue("[]");nativeHome.playerCommand("snapshot",0);return;}
            if ("source".equals(action)||"quality".equals(action)||"send".equals(action)) {
                if(!nativePlaybackItem.isEmpty()) changeNativeLibraryItem(nativePlaybackItem,action);return;
            }
            if ("retry".equals(action)) {openNativePlayback(nativePlaybackFilename);return;}
            if ("pip".equals(action)) {enterNativePlaybackPip();return;}
            if ("fullscreen".equals(action)) {
                setNativePlaybackFullscreen(!nativePlaybackFullscreen);nativeHome.playerCommand("snapshot",0);return;
            }
            if ("share".equals(action)) {sharePublishedDownload(nativePlaybackFilename);return;}
            if ("seek-dialog".equals(action)) {
                android.widget.EditText input=new android.widget.EditText(this);
                input.setInputType(android.text.InputType.TYPE_CLASS_NUMBER|android.text.InputType.TYPE_NUMBER_FLAG_DECIMAL);
                input.setHint("Position in seconds");
                new android.app.AlertDialog.Builder(this).setTitle("Seek to position").setView(input)
                    .setNegativeButton("Cancel",null).setPositiveButton("Seek",(dialog,which)->{
                        try {double seconds=Double.parseDouble(input.getText().toString());
                            if(Double.isFinite(seconds)&&seconds>=0d) requestNativePlayer("seek",seconds);
                        }catch(NumberFormatException invalid){android.widget.Toast.makeText(this,"Enter a valid position in seconds",android.widget.Toast.LENGTH_SHORT).show();}
                    }).show();return;
            }
            if ("rotation".equals(action)) {
                nativePlaybackRotationLocked=!nativePlaybackRotationLocked;
                setPlaybackRotationLocked(nativePlaybackRotationLocked);
                nativeHome.playerCommand("snapshot",0);return;
            }
            if ("next".equals(action)) {
                nativeHome.playerCommand("next",0);
                return;
            }
            nativeHome.playerCommand(action,value);
        });
    }
    public void positionNativePlayerSurface(float x,float y,float width,float height) {
        handler.post(()->{
            if (isDestroyed()||nativeHome==null||!nativeHomeVisible||!NativePlaybackPolicy.playerScreen(nativeHome.screen())||nativePlaybackPrivacy()) return;
            int[] host=new int[2],content=new int[2];nativeHome.view().getLocationInWindow(host);
            findViewById(android.R.id.content).getLocationInWindow(content);
            float density=getResources().getDisplayMetrics().density;
            nativeHome.playerVisible(activityResumed);
            nativeHome.playerSurface(host[0]-content[0]+Math.round(x*density),host[1]-content[1]+Math.round(y*density),Math.round(width*density),Math.round(height*density));
        });
    }
    private final Runnable nativePipRefresh=new Runnable() {
        @Override public void run() {
            if(!isDestroyed()&&nativePlaybackPip&&isInPictureInPictureMode()&&nativeHome!=null) {
                nativeHome.playerCommand("snapshot",0);handler.postDelayed(this,1000);
            }
        }
    };
    private void refreshNativePipActions() {
        if(nativePlaybackPip&&isInPictureInPictureMode()&&!isDestroyed()) {
            try {setPictureInPictureParams(new PictureInPictureParams.Builder().setActions(nativePipActions()).build());}
            catch(IllegalArgumentException | IllegalStateException unavailable) {}
        }
    }
    private void refreshNativePipSurface() {
        if(nativePlaybackPip&&isInPictureInPictureMode()&&nativeHome!=null&&!isDestroyed()) {
            nativeHome.playerVisible(true);
            View decor=getWindow().getDecorView();nativeHome.playerSurface(0,0,decor.getWidth(),decor.getHeight());
        }
    }

    void handleNativePipCommand(Intent intent) {
        if(isDestroyed()||intent==null||!nativePlaybackPip||!isInPictureInPictureMode()) return;
        String command=intent.getStringExtra("command");
        if(NativePlaybackPolicy.pipCommandAllowed(command,nativePlaybackPrivacy())) requestNativePlayer(command,0);
    }
    private java.util.List<android.app.RemoteAction> nativePipActions() {
        java.util.List<android.app.RemoteAction> actions=new java.util.ArrayList<>();
        String command=nativePlaybackPlaying?"pause":"play",label=nativePlaybackPlaying?"Pause":"Play";
        Intent intent=new Intent(this,NativePipReceiver.class).putExtra("command",command);
        android.app.PendingIntent pending=android.app.PendingIntent.getBroadcast(this,"pause".equals(command)?905:904,intent,android.app.PendingIntent.FLAG_UPDATE_CURRENT|android.app.PendingIntent.FLAG_IMMUTABLE);
        android.graphics.drawable.Icon icon=android.graphics.drawable.Icon.createWithResource(this,nativePlaybackPlaying?android.R.drawable.ic_media_pause:android.R.drawable.ic_media_play);
        android.app.RemoteAction action=new android.app.RemoteAction(icon,label,label,pending);action.setEnabled(NativePlaybackPolicy.pipCommandAllowed(command,nativePlaybackPrivacy()));actions.add(action);return actions;
    }
    private void enterNativePlaybackPip() {
        if(nativeHome==null||!NativePlaybackPolicy.playerScreen(nativeHome.screen())||!NativePlaybackPolicy.allowPip(nativePlaybackPrivacy(),nativeHomeVisible&&activityResumed,isDestroyed(),supportsPictureInPicture(),nativePlaybackAudioOnly,nativePlaybackPlaying)) return;
        setNativePlaybackFullscreen(false);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
        NativePipReceiver.bind(this);nativePlaybackPip=true;nativeHome.playerPip(true);
        boolean entered=false;
        int width=nativePlaybackVideoWidth,height=nativePlaybackVideoHeight;double ratio=height>0?(double)width/height:0;
        if(ratio<.4185d||ratio>2.39d) {width=16;height=9;}
        try {entered=enterPictureInPictureMode(new PictureInPictureParams.Builder().setAspectRatio(new Rational(width,height)).setActions(nativePipActions()).build());}
        catch(IllegalArgumentException | IllegalStateException unavailable) {}
        if(!entered) {nativePlaybackPip=false;nativeHome.playerPip(false);applyScreenshotPreference();}
    }

    private void setNativePlaybackFullscreen(boolean enabled) {
        if(enabled&&!NativePlaybackPolicy.allowFullscreen(nativePlaybackPrivacy(),nativeHomeVisible&&activityResumed,isDestroyed(),nativeHome!=null&&nativeHome.screen()==12)) return;
        if(enabled==nativePlaybackFullscreen) return;
        nativePlaybackFullscreen=enabled;
        if(enabled) {
            nativePlaybackDecorFlags=getWindow().getDecorView().getSystemUiVisibility();
            getWindow().addFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN|WindowManager.LayoutParams.FLAG_SECURE);
            applyFullscreenSystemUi();
        } else {
            getWindow().clearFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN);
            getWindow().getDecorView().setSystemUiVisibility(nativePlaybackDecorFlags);
            applyScreenshotPreference();
        }
    }
    void resetNativePlaybackRotation() {
        handler.post(()->setNativePlaybackFullscreen(false));
        nativePlaybackPlaying=false;handler.post(this::applyPlaybackScreenPreferenceNow);
        if(nativePlaybackRotationLocked) {nativePlaybackRotationLocked=false;setPlaybackRotationLocked(false);}
    }
    private void openNativeStreaming(String watch,String episode) {
        if(nativeHome==null||!StreamingActivity.isAllowedUrl(watch)) return;
        nativeStreamingWatch=watch;nativeStreamingEpisode=episode;nativeStreamingToken="";nativeStreamingOffset=0;nativeStreamingSourceOffset=0;nativeStreamingFilter="all";nativeHome.showStreaming();
        try {JSONObject payload=new JSONObject();payload.put("watchUrl",watch);payload.put("episode",episode);runNativeStreaming("open",payload);}catch(Exception invalid) {}
    }
    public void requestNativeStreaming(String action,String token,int index) {
        if(index<0||index>100000||token==null||token.length()>64||!("watchlist-add".equals(action)||"watchlist-remove".equals(action)||"filter".equals(action)||"sources".equals(action)||"refresh".equals(action)||"play".equals(action)||"episode".equals(action)||"page".equals(action))) return;
        handler.post(()->{
            if(isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=13) return;
            if("filter".equals(action)) {
                if(index>4) return; nativeStreamingFilter=new String[]{"all","sub","dub","ready","issues"}[index]; nativeStreamingSourceOffset=0;
                requestNativeStreaming("page",token,nativeStreamingOffset);return;
            }
            if("refresh".equals(action)) {
                if(nativeStreamingToken.isEmpty()) openNativeStreaming(nativeStreamingWatch,nativeStreamingEpisode);
                else requestNativeStreaming("page",nativeStreamingToken,nativeStreamingOffset);return;
            }
            if("play".equals(action)) {
                try {JSONObject source=new JSONObject(nativeStreamingSource(token,index));String url=source.optString("url");if(url.isEmpty()) {nativeStreamingToken="";nativeHome.updateStreaming("{\"detail\":\"Streaming selection expired. Refresh and select a server again.\"}");return;}
                    Intent launch=new Intent(this,StreamingActivity.class);launch.putExtra(StreamingActivity.EXTRA_URL,nativeStreamingWatch);
                    launch.putExtra(StreamingActivity.EXTRA_EPISODE,source.optString("episode"));launch.putExtra("native-source-url",url);launch.putExtra("native-manifest-token",token);
                    launch.putExtra(StreamingActivity.EXTRA_MANIFEST_URL,baseUrl+"__app/stream-manifest.json?url="+Uri.encode(nativeStreamingWatch));startActivity(launch);
                }catch(Exception unavailable) {}return;
            }
            try {JSONObject payload=new JSONObject();payload.put("token",token);payload.put("episode".equals(action)?"index":"offset",("sources".equals(action)||action.startsWith("watchlist-"))?nativeStreamingOffset:index);payload.put("sourceOffset","sources".equals(action)?index:nativeStreamingSourceOffset);payload.put("sourceFilter",nativeStreamingFilter);runNativeStreaming("sources".equals(action)?"page":action,payload);}catch(Exception invalid) {}
        });
    }
    private void runNativeStreaming(String action,JSONObject payload) {
        int generation=++nativeStreamingGeneration;boolean privacy=nativePlaybackPrivacy();nativeStreamingPrivacy=privacy;
        nativeHome.updateStreaming("{\"detail\":\"Resolving streaming controls…\"}");
        try {nativeActions.execute(()->{
            String data=nativeStreamingCommand(action,payload.toString(),privacy);
            handler.post(()->{if(isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=13||generation!=nativeStreamingGeneration) return;
                if(privacy!=nativePlaybackPrivacy()) {try {JSONObject result=new JSONObject(data);String token=result.optString("token");if(!token.isEmpty()){nativeStreamingSourceOffset=result.optInt("sourceOffset",nativeStreamingSourceOffset);requestNativeStreaming("page",token,result.optInt("episodeOffset",nativeStreamingOffset));return;}}catch(Exception invalid) {}nativeHome.updateStreaming("{\"detail\":\"Refresh streaming selection\"}");return;}
                try {JSONObject result=new JSONObject(data);if(result.optBoolean("ok")){nativeStreamingToken=result.optString("token");nativeStreamingOffset=result.optInt("episodeOffset");nativeStreamingSourceOffset=result.optInt("sourceOffset");nativeStreamingFilter=result.optString("sourceFilter","all");}}catch(Exception invalid) {}
                nativeHome.updateStreaming(data);
            });
        });}catch(java.util.concurrent.RejectedExecutionException busy){nativeHome.updateStreaming("{\"detail\":\"Streaming controls are busy. Try again.\"}");}
    }
    private native String nativeStreamingSource(String token,int index);
    private native String nativeStreamingCommand(String action,String payload,boolean privacy);
    private native String nativePlaybackHandle(String filename);
    private native String nativePlaybackNext(String filename);
    private native String nativePlaybackRoute(String filename);
    private native long nativePlaybackDownloaded(String filename);

    public void requestNativeUpdates(String action) {
        if (!("snapshot".equals(action)||"check".equals(action)||"install".equals(action))) return;
        handler.post(()->{
            if (isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=11) return;
            if (updateManager!=null) {
                if ("check".equals(action)) updateManager.checkNow();
                if ("install".equals(action)) updateManager.installReadyUpdate();
            }
            try {
                JSONObject data=updateManager==null?new JSONObject("{\"state\":\"disabled\",\"detail\":\"Updates unavailable\"}")
                        :new JSONObject(updateManager.activityStatus());
                if (settingsBridge==null||settingsBridge.inspectionPrivacyEnabled()) {
                    data.put("detail","Updates · "+data.optString("state","unavailable"));data.put("version","");
                }
                nativeHome.updateUpdates(data.toString());
            } catch (org.json.JSONException invalid) {nativeHome.updateUpdates("{\"detail\":\"Update status unavailable\"}");}
        });
    }

    public void openNativeActivityItem(String id,String action) {
        if (id==null||!("queue".equals(action)||"play".equals(action)||"transfer".equals(action))) return;
        handler.post(()->{
            if (isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=10) return;
            if ("queue".equals(action)) {
                nativeHome.showQueue();nativeHome.updateQueue(nativeQueueSnapshot(0));return;
            }
            changeNativeLibraryItem(id,"transfer".equals(action)?"send":"play");
        });
    }

    public void requestNativeActivity(String filter,int offset) {
        if (filter==null||offset<0||offset>100000||!("all".equals(filter)||"active".equals(filter)
                ||"issue".equals(filter)||"complete".equals(filter))) return;
        handler.post(()->{
            if (isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=10) return;
            handler.removeCallbacks(nativeActivityRefresh);
            nativeActivityOffset=offset;nativeActivityFilter=filter;
            final int generation=++nativeActivityGeneration;
            final boolean privacy=settingsBridge==null||settingsBridge.inspectionPrivacyEnabled();
            final String request;
            try {JSONObject data=new JSONObject();data.put("filter",filter);data.put("offset",offset);request=data.toString();}
            catch (org.json.JSONException invalid) {return;}
            try {
                nativeActions.execute(()->{
                    String response=nativeActivitySnapshot(request,privacy);
                    handler.post(()->{
                        if (isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=10||generation!=nativeActivityGeneration) return;
                        if (privacy!=(settingsBridge==null||settingsBridge.inspectionPrivacyEnabled())) {
                            requestNativeActivity(filter,offset);return;
                        }
                        try {
                            JSONObject data=new JSONObject(response);
                            nativeActivityOffset=data.optInt("offset",offset);
                            JSONObject update=new JSONObject(activityCenterStatus()).optJSONObject("update");
                            if (update!=null) data.put("updateDetail",privacy?"Updates · "+update.optString("state","unavailable"):update.optString("detail","Update status unavailable"));
                            nativeHome.updateActivity(data.toString());
                        } catch (org.json.JSONException invalid) {nativeHome.updateActivity("{\"detail\":\"Activity unavailable. Try again.\"}");}
                        if (activityResumed) handler.postDelayed(nativeActivityRefresh,15000L);
                    });
                });
            } catch (java.util.concurrent.RejectedExecutionException busy) {
                nativeHome.updateActivity("{\"detail\":\"Activity requests are busy. Try again.\"}");
                if (activityResumed) handler.postDelayed(nativeActivityRefresh,15000L);
            }
        });
    }

    public void requestNativeStorage(String action, String id, int offset) {
        if (id==null||id.length()>64||offset<0||offset>100000||!("snapshot".equals(action)
                ||"delete".equals(action)||"watched".equals(action)||"stale".equals(action)||"thumbnails".equals(action))) return;
        handler.post(() -> {
            if (isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=9) return;
            nativeStorageOffset=offset;
            if (!"snapshot".equals(action)) {
                String message="delete".equals(action)?"Delete this video from RustDL and Android Downloads?"
                        :"watched".equals(action)?"Delete all watched videos from RustDL and Android Downloads?"
                        :"stale".equals(action)?"Remove stale partial downloads?":"Clear cached thumbnails? They will be regenerated when needed.";
                new android.app.AlertDialog.Builder(this).setTitle("Confirm storage cleanup").setMessage(message)
                        .setPositiveButton("Remove",(dialog,which)->runNativeStorage(action,id,offset))
                        .setNegativeButton("Cancel",null).show();return;
            }
            runNativeStorage(action,id,offset);
        });
    }

    private void runNativeStorage(String action, String id, int offset) {
        if (isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=9) return;
        final int generation=++nativeStorageGeneration;
        final boolean privacy=settingsBridge==null||settingsBridge.inspectionPrivacyEnabled();
        final String request;
        try {JSONObject data=new JSONObject();data.put("id",id);data.put("offset",offset);data.put("action",action);request=data.toString();}
        catch (org.json.JSONException invalid) {return;}
        try {
            nativeActions.execute(() -> {
                String response=nativeStorageCommand("snapshot".equals(action)?"snapshot":"cleanup",request,privacy);
                handler.post(() -> {
                    if (isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=9||generation!=nativeStorageGeneration) return;
                    if (privacy!=(settingsBridge==null||settingsBridge.inspectionPrivacyEnabled())) {
                        requestNativeStorage("snapshot","",offset);return;
                    }
                    try {nativeStorageOffset=new JSONObject(response).optInt("offset",offset);}
                    catch (org.json.JSONException invalid) { }
                    nativeHome.updateStorage(response);
                });
            });
        } catch (java.util.concurrent.RejectedExecutionException busy) {
            nativeHome.updateStorage("{\"detail\":\"Storage requests are busy. Try again.\"}");
        }
    }

    private void inputNativeAnime() {
        final String[] sections = {"newest","updated","ongoing","added","type/tv-series","type/movies","type/ova","type/ona","type/special","type/music","genre/action","genre/adventure","genre/comedy","genre/drama","genre/fantasy","genre/historical","genre/horror","genre/isekai","genre/martial-arts","genre/mecha","genre/music","genre/mystery","genre/psychological","genre/romance","genre/school","genre/sci-fi","genre/sports","genre/supernatural","genre/suspense"};
        final String[] labels = {"Newest","Updated","Ongoing","Recently added","TV Series","Movies","OVAs","ONAs","Specials","Music Videos","Action","Adventure","Comedy","Drama","Fantasy","Historical","Horror","Isekai","Martial Arts","Mecha","Music","Mystery","Psychological","Romance","School","Sci-Fi","Sports","Supernatural","Suspense"};
        android.widget.LinearLayout form = new android.widget.LinearLayout(this);
        form.setOrientation(android.widget.LinearLayout.VERTICAL);
        android.widget.EditText query = new android.widget.EditText(this);
        query.setHint("Search anime");
        query.setSingleLine(true);
        query.setFilters(new android.text.InputFilter[]{new android.text.InputFilter.LengthFilter(256)});
        query.setText(nativeAnimeQuery);
        android.widget.Spinner category = new android.widget.Spinner(this);
        category.setAdapter(new android.widget.ArrayAdapter<String>(this, android.R.layout.simple_spinner_dropdown_item, labels));
        for (int i=0;i<sections.length;i++) if (sections[i].equals(nativeAnimeSection)) category.setSelection(i);
        form.addView(query); form.addView(category);
        new android.app.AlertDialog.Builder(this).setTitle("Anime search and categories").setView(form)
                .setPositiveButton("Browse", (dialog,which) -> {
                    nativeAnimeQuery=query.getText().toString().trim();
                    nativeAnimeSection=sections[category.getSelectedItemPosition()];
                    requestNativeAnime("catalog","",1);
                }).setNegativeButton("Cancel",null).show();
    }

    public void requestNativeAnime(String action, String id, int page) {
        if (id==null || id.length()>64 || !("catalog".equals(action)||"watchlist".equals(action)
                ||"add".equals(action)||"remove".equals(action)||"watch".equals(action)
                ||"episodes".equals(action)||"play".equals(action)||"calendar-page".equals(action)
                ||"search".equals(action)||"refresh".equals(action)||"calendar".equals(action))) return;
        handler.post(() -> {
            if (isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=8) return;
            if ("search".equals(action)) {inputNativeAnime();return;}
            final String command="refresh".equals(action)?("calendar-page".equals(nativeAnimeView)?"calendar":nativeAnimeView):action;
            if ("catalog".equals(command)||"watchlist".equals(command)||"episodes".equals(command)||"calendar".equals(command)||"calendar-page".equals(command)) {
                if (("episodes".equals(command)||"calendar-page".equals(command))&&!id.isEmpty()) nativeAnimeSelection=id;
                nativeAnimeView=command;nativeAnimePage=Math.max(1,Math.min(1000,page));
            }
            final int generation=++nativeAnimeGeneration;
            final boolean privacy=settingsBridge==null||settingsBridge.inspectionPrivacyEnabled();
            final String request;
            try {
                JSONObject data=new JSONObject();data.put("id",("episodes".equals(command)||"calendar-page".equals(command))?nativeAnimeSelection:id);data.put("page",nativeAnimePage);
                data.put("section",nativeAnimeSection);data.put("query",nativeAnimeQuery);
                data.put("refresh","refresh".equals(action));request=data.toString();
            } catch (org.json.JSONException invalid) {return;}
            if ("catalog".equals(command)||"watchlist".equals(command)) nativeHome.updateAnime("{\"detail\":\"Loading anime…\"}");
            try {
                boolean readOnly="catalog".equals(command)||"episodes".equals(command);
                java.util.concurrent.ThreadPoolExecutor executor=readOnly?nativeAnimeReads:nativeActions;
                if(readOnly) nativeAnimeReads.getQueue().clear();
                executor.execute(() -> {
                    if(generation!=nativeAnimeGeneration||isDestroyed()) return;
                    String response=nativeAnimeCommand(command,request,privacy);
                    handler.post(() -> {
                        if (isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=8||generation!=nativeAnimeGeneration) return;
                        if (privacy!=(settingsBridge==null||settingsBridge.inspectionPrivacyEnabled())) {
                            // Re-read the completed calendar session, without marking releases seen twice.
                            try {
                                JSONObject completed=new JSONObject(response);
                                if (completed.optBoolean("ok")&&"calendar".equals(completed.optString("kind"))) {
                                    nativeAnimeView="calendar-page";nativeAnimeSelection=completed.optString("selection");
                                    nativeAnimePage=Math.max(1,completed.optInt("page",1));
                                }
                            } catch (org.json.JSONException invalid) { }
                            requestNativeAnime(nativeAnimeView,"",nativeAnimePage);return;
                        }
                        try {
                            JSONObject data=new JSONObject(response);
                            if (data.optBoolean("ok")&&("watch".equals(command)||"play".equals(command))) {
                                if(nativeHome!=null) {openNativeStreaming(data.optString("watchUrl"),data.optString("episode"));return;}
                                Uri launch=new Uri.Builder().scheme("rustdl").authority("stream")
                                        .appendQueryParameter("url",data.optString("watchUrl"))
                                        .appendQueryParameter("episode",data.optString("episode")).build();
                                handleModeSwitch(launch);return;
                            }
                            if (data.optBoolean("ok")&&("add".equals(command)||"remove".equals(command))) {
                                requestNativeAnime(nativeAnimeView,"",nativeAnimePage);return;
                            }
                            if (data.optBoolean("ok")&&data.has("page")) nativeAnimePage=Math.max(1,data.optInt("page",1));
                            if (data.optBoolean("ok")&&"calendar".equals(data.optString("kind"))) {
                                nativeAnimeSelection=data.optString("selection");nativeAnimeView="calendar-page";
                                org.json.JSONArray items=data.optJSONArray("items");
                                if (items!=null&&!privacy) for (int i=0;i<items.length();i++) {
                                    JSONObject item=items.optJSONObject(i);if (item==null) continue;
                                    long release=item.optLong("displayRelease",0);
                                    String label=release>0&&release<=Long.MAX_VALUE/1000L
                                            ? java.text.DateFormat.getDateTimeInstance(java.text.DateFormat.MEDIUM,java.text.DateFormat.SHORT)
                                                .format(new java.util.Date(release*1000L)) : "Release time unavailable";
                                    item.put("displayReleaseText",label);
                                }
                            }
                            String initial=NativeAnimePosters.load(this,data,privacy,updated->handler.post(()->{
                                if(!isDestroyed()&&nativeHome!=null&&nativeHomeVisible&&nativeHome.screen()==8&&generation==nativeAnimeGeneration&&!nativePlaybackPrivacy()) nativeHome.updateAnime(updated);
                            }));
                            nativeHome.updateAnime(initial);
                        } catch (org.json.JSONException invalid) {
                            nativeHome.updateAnime("{\"detail\":\"Could not load anime. Try again.\"}");
                        }
                    });
                });
            } catch (java.util.concurrent.RejectedExecutionException busy) {
                nativeHome.updateAnime("{\"detail\":\"Anime requests are busy. Try again.\"}");
            }
        });
    }

    public void openNativeReceiver() {
        handler.post(() -> {
            if (isDestroyed() || nativeHome == null) return;
            nativePeerItem = "";
            nativeHome.showPeers();
            requestNativePeer("snapshot", "{}");
        });
    }

    public void requestNativePeer(String action, String payload) {
        if (!("snapshot".equals(action)||"pair".equals(action)||"send".equals(action)
                ||"receive".equals(action)||"copy".equals(action))
                || payload == null || payload.length()>8192) return;
        handler.post(() -> {
            if (isDestroyed() || nativeHome == null || !nativeHomeVisible || nativeHome.screen()!=7) return;
            final int generation=++nativePeerGeneration;
            final boolean privacy=settingsBridge==null||settingsBridge.inspectionPrivacyEnabled();
            final String request;
            try {JSONObject data=new JSONObject(payload);data.put("id",nativePeerItem);request=data.toString();}
            catch (org.json.JSONException invalid) {return;}
            try {
                nativeActions.execute(() -> {
                    String response=nativePeerCommand(action,request,privacy);
                    handler.post(() -> {
                        if (isDestroyed()||nativeHome==null||!nativeHomeVisible||nativeHome.screen()!=7
                                || generation!=nativePeerGeneration) return;
                        if (privacy!=(settingsBridge==null||settingsBridge.inspectionPrivacyEnabled())) {
                            requestNativePeer("snapshot","{}");return;
                        }
                        if ("copy".equals(action)) {
                            try {
                                JSONObject copied = new JSONObject(response);
                                String text = copied.optString("copyText");
                                if (copied.optBoolean("ok") && !text.isEmpty()) {
                                    android.content.ClipboardManager clipboard = (android.content.ClipboardManager)
                                            getSystemService(CLIPBOARD_SERVICE);
                                    if (clipboard != null) clipboard.setPrimaryClip(android.content.ClipData.newPlainText("RustDL pairing", text));
                                    android.widget.Toast.makeText(this, "Receiver pairing copied", android.widget.Toast.LENGTH_SHORT).show();
                                } else android.widget.Toast.makeText(this, "Pairing expired. Renew receiving.", android.widget.Toast.LENGTH_SHORT).show();
                            } catch (org.json.JSONException invalid) { }
                            requestNativePeer("snapshot", "{}");
                            return;
                        }
                        nativeHome.updatePeers(response);
                        try {
                            JSONObject state = new JSONObject(response);
                            String phase=state.optString("phase");
                            boolean receiving=state.optBoolean("receiveEnabled");
                            if (receiving || phase.equals("hashing")||phase.equals("sending")||phase.equals("working")
                                    ||phase.equals("transferring")||phase.equals("verifying")) {
                                handler.postDelayed(() -> {
                                    if (generation==nativePeerGeneration&&activityResumed) requestNativePeer("snapshot","{}");
                                },receiving && phase.equals("idle") ? 10000L : 1000L);
                            }
                        } catch (org.json.JSONException invalid) { }
                    });
                });
            } catch (java.util.concurrent.RejectedExecutionException busy) {
                nativeHome.updatePeers("{\"ok\":false,\"detail\":\"Transfers are busy. Try again.\"}");
            }
        });
    }

    private native String nativeActivitySnapshot(String payload, boolean privacy);

    private native String nativeStorageCommand(String action, String payload, boolean privacy);

    private native String nativeAnimeCommand(String action, String payload, boolean privacy);

    private native String nativePeerCommand(String action, String payload, boolean privacy);

    private native String nativeDiscoveryCommand(String command, String payload, boolean privacy);

    private native String nativeLibraryData(int offset, String location, String search, boolean privacy, String resumeSelection);
    private native String nativeLibraryMedia(String id);
    private native String nativeLibrarySource(String id);
    private native String nativeLibraryThumbnail(String id, boolean privacy);
    private native boolean nativeLibraryDelete(String id);

    public String nativeDiagnosticsSnapshot() {
        lastNativeDiagnostics = diagnosticsBridge == null ? "{}" : diagnosticsBridge.diagnostics();
        return lastNativeDiagnostics;
    }

    public void copyNativeDiagnostics() {
        handler.post(() -> {
            if (isDestroyed() || diagnosticsBridge == null || lastNativeDiagnostics == null) return;
            boolean copied = diagnosticsBridge.copySnapshot(lastNativeDiagnostics);
            Toast.makeText(this, copied ? "Diagnostics copied" : "Could not copy diagnostics", Toast.LENGTH_SHORT).show();
        });
    }

    public String nativeQueueSnapshot(int offset) {
        nativeQueueOffset = Math.max(0, offset);
        nativeQueueVisible = true;
        return queueSnapshotData();
    }

    private String queueSnapshotData() {
        String data = nativeQueueData(nativeQueueOffset,
                settingsBridge == null || settingsBridge.inspectionPrivacyEnabled());
        try {
            JSONObject snapshot = new JSONObject(data);
            snapshot.put("networkState", settingsBridge == null ? 0 : settingsBridge.downloadNetworkState());
            return snapshot.toString();
        } catch (org.json.JSONException invalid) { return "{}"; }
    }

    public void approveNativeMobileDownloads() {
        handler.post(() -> {
            if (!isDestroyed() && settingsBridge != null)
                settingsBridge.requestMobileDownloadApproval();
        });
    }

    public void changeNativeQueueItem(String id, String action) {
        if (id == null || !id.matches("[a-f0-9]{32}") || action == null) return;
        if (!("pause".equals(action) || "resume".equals(action) || "cancel".equals(action)
                || "play".equals(action))) return;
        if ("play".equals(action)) {
            handler.post(() -> {
                if (isDestroyed() || nativeHome == null) return;
                String filename = nativeQueueMedia(id);
                if (filename == null || !filename.matches(MEDIA_NAME_PATTERN)) return;
                openNativePlayback(filename);
            });
            return;
        }
        try {
            nativeActions.execute(() -> {
                boolean ok = nativeQueueAction(id, action);
                handler.post(() -> {
                    if (isDestroyed() || nativeHome == null || !nativeHomeVisible || !nativeQueueVisible) return;
                    String data = queueSnapshotData();
                    if (!ok) {
                        try {
                            JSONObject result = new JSONObject(data);
                            result.put("ok", false);
                            result.put("detail", "Could not change this item. Its state may have changed.");
                            data = result.toString();
                        } catch (org.json.JSONException invalid) { return; }
                    }
                    nativeHome.updateQueue(data);
                });
            });
        } catch (java.util.concurrent.RejectedExecutionException busy) {
            handler.post(() -> Toast.makeText(this, "Queue is busy. Try again.", Toast.LENGTH_SHORT).show());
        }
    }

    private native String nativeQueueData(int offset, boolean privacy);
    private native boolean nativeQueueAction(String id, String action);
    private native String nativeQueueMedia(String id);

    /** Preference snapshots contain no downloaded-media metadata. */
    public String nativeSettingsSnapshot() {
        return settingsBridge == null ? "{}" : settingsBridge.settings();
    }

    public void changeNativeSetting(String key, String value) {
        if (!NativeSettingsRequest.valid(key, value)) return;
        handler.post(() -> {
            if (isFinishing() || isDestroyed() || settingsBridge == null || nativeHome == null) return;
            if ("reset".equals(key)) {
                new android.app.AlertDialog.Builder(this)
                        .setTitle("Restore default settings?")
                        .setNegativeButton("Cancel", null)
                        .setPositiveButton("Restore", (dialog, which) -> {
                            if (!isDestroyed() && nativeHome != null)
                                nativeHome.updateSettings(settingsBridge.reset());
                        })
                        .show();
                return;
            }
            if ("downloadFolder".equals(key)) {
                android.widget.EditText field = new android.widget.EditText(this);
                field.setSingleLine(true);
                field.setFilters(new android.text.InputFilter[]{new android.text.InputFilter.LengthFilter(48)});
                try { field.setText(new JSONObject(settingsBridge.settings()).getString("downloadFolder")); }
                catch (org.json.JSONException invalid) { return; }
                android.app.AlertDialog dialog = new android.app.AlertDialog.Builder(this)
                        .setTitle("Download folder")
                        .setMessage("Folder inside Downloads for newly exported files")
                        .setView(field).setNegativeButton("Cancel", null)
                        .setPositiveButton("Save", null).create();
                dialog.setOnShowListener(ignored -> dialog.getButton(android.app.AlertDialog.BUTTON_POSITIVE)
                        .setOnClickListener(view -> {
                            if (isDestroyed() || nativeHome == null) return;
                            String result = persistNativeSetting("downloadFolder", field.getText().toString());
                            try {
                                if (new JSONObject(result).optBoolean("ok")) dialog.dismiss();
                                else field.setError(new JSONObject(result).optString("detail", "Invalid folder"));
                            } catch (org.json.JSONException invalid) { field.setError("Could not save folder"); }
                            if (nativeHome != null) nativeHome.updateSettings(result);
                        }));
                dialog.show();
                return;
            }
            nativeHome.updateSettings(persistNativeSetting(key, value));
        });
    }

    private String persistNativeSetting(String key, String value) {
        try {
            JSONObject current = new JSONObject(settingsBridge.settings());
            if ("diagnosticsRefreshSeconds".equals(key)) current.put(key, Integer.parseInt(value));
            else if ("keepScreenAwake".equals(key) || "allowScreenshots".equals(key)
                    || "inspectionPrivacy".equals(key) || "spaceEffectEnabled".equals(key)
                    || "reduceMotion".equals(key)) current.put(key, Boolean.parseBoolean(value));
            else current.put(key, value);
            return settingsBridge.save(current.getString("downloadFolder"),
                    current.getBoolean("keepScreenAwake"), current.getInt("diagnosticsRefreshSeconds"),
                    current.getString("appearance"), current.getBoolean("spaceEffectEnabled"),
                    current.getString("backgroundTheme"), current.getBoolean("allowScreenshots"),
                    current.getBoolean("reduceMotion"), current.getString("mobileDownloadPolicy"),
                    current.getBoolean("inspectionPrivacy"));
        } catch (org.json.JSONException | NumberFormatException invalid) {
            return "{\"ok\":false,\"detail\":\"Could not save settings\"}";
        }
    }

    @Override public boolean dispatchKeyEvent(KeyEvent event) {
        if (nativeHomeVisible && nativeHome != null && nativeHome.key(event)) return true;
        return super.dispatchKeyEvent(event);
    }

    @Override
    public void onConfigurationChanged(Configuration newConfig) {
        super.onConfigurationChanged(newConfig);
        if (settingsBridge != null && "system".equals(settingsBridge.appearance())) {
            applyAppearance("system");
        }
    }

    void setPlaybackActive(boolean active, int width, int height) {
        handler.post(() -> {
            playbackActive = active;
            if (width > 0 && height > 0) {
                playbackWidth = width;
                playbackHeight = height;
            }
            applyPlaybackScreenPreferenceNow();
            if (android.os.Build.VERSION.SDK_INT >= 31 && supportsPictureInPicture()) {
                try {
                    PictureInPictureParams params = new PictureInPictureParams.Builder()
                            .setAspectRatio(new Rational(playbackWidth, playbackHeight))
                            .setAutoEnterEnabled(active)
                            .setSeamlessResizeEnabled(true)
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
        if (settingsBridge != null && settingsBridge.screenshotRedactionEnabled()) {
            getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
        }
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
        if (settingsBridge != null) {
            applyAppearance(settingsBridge.appearance());
        }
        applyScreenshotPreference();

        if (fullscreenCallback != null) {
            fullscreenCallback.onCustomViewHidden();
            fullscreenCallback = null;
        }
    }

    void applyPlaybackScreenPreference() {
        handler.post(this::applyPlaybackScreenPreferenceNow);
    }

    private void applyPlaybackScreenPreferenceNow() {
        boolean keepAwake = (playbackActive || (nativePlaybackPlaying&&nativeHomeVisible&&activityResumed&&nativeHome!=null&&NativePlaybackPolicy.playerScreen(nativeHome.screen()))) && settingsBridge != null
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
        showWebContent();
        resetNativeHistory = nativeHome != null;
        if (intent != null && Intent.ACTION_VIEW.equals(intent.getAction())) {
            Uri data = intent.getData();
            if (data != null && "rustdl".equals(data.getScheme())
                    && "pair".equals(data.getHost())) {
                String address = data.getQueryParameter("address");
                String key = data.getQueryParameter("key");
                if (address != null && key != null && nativeSetPeerPairing(address, key)) {
                    Toast.makeText(this, "RustDL devices paired", Toast.LENGTH_SHORT).show();
                    if (showNativeHome()) { openNativeReceiver(); return; }
                    webView.loadUrl(baseUrl + "peers/connected");
                } else {
                    Toast.makeText(this, "That pairing code is invalid", Toast.LENGTH_SHORT).show();
                    if (showNativeHome()) { openNativeReceiver(); return; }
                    webView.loadUrl(baseUrl + "peers/connected");
                }
                return;
            }
        }
        if(intent!=null&&OPEN_NATIVE_PLAYER_ACTION.equals(intent.getAction())) {
            if(showNativeHome()) {nativeHome.showPlayer();nativeHome.playerPrivacy(nativePlaybackPrivacy());nativeHome.resumePlayer();}
            return;
        }
        if (intent != null && OPEN_QUEUE_ACTION.equals(intent.getAction())) {
            if (showNativeHome()) {
                nativeHome.updateSettings(nativeSettingsSnapshot());
                nativeHome.updateQueue(nativeQueueSnapshot(0));
                nativeHome.showQueue();
                return;
            }
            webView.loadUrl(baseUrl + "queue");
            return;
        }
        String sharedUrls = extractSharedUrls(intent);
        if (sharedUrls == null) {
            if (showNativeHome()) return;
            webView.loadUrl(baseUrl);
            return;
        }
        Toast.makeText(this, "Adding shared video links…", Toast.LENGTH_SHORT).show();
        if (showNativeHome()) {
            nativeHome.showDiscovery();
            try {
                JSONObject payload=new JSONObject();payload.put("source",sharedUrls);
                requestNativeDiscovery("start",payload.toString());
            } catch(org.json.JSONException invalid) {
                nativeHome.updateDiscovery("{\"ok\":false,\"error\":\"Could not read shared links.\"}");
            }
            return;
        }
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
        final boolean updateEvent;
        try {
            JSONObject event = new JSONObject(eventJson);
            String type = event.optString("type", "");
            if (!("queue".equals(type) || "peer".equals(type)
                    || "update".equals(type) || "activity".equals(type)
                    || "sync".equals(type))
                    || event.optInt("version", -1) != 1) {
                return;
            }
            updateEvent="update".equals(type);
            encoded = JSONObject.quote(event.toString());
        } catch (Exception invalid) {
            return;
        }
        handler.post(() -> {
            if (updateEvent&&nativeHomeVisible&&nativeHome!=null&&nativeHome.screen()==11) requestNativeUpdates("snapshot");
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
            nativeTransferCount = Math.max(0, count);
            nativeDownloaded = Math.max(0L, downloaded);
            nativeTotal = Math.max(0L, total);
            if (nativeHome != null) {
                if(nativePlaybackPrivacy()) setNativePlaybackFullscreen(false);
                if(nativeHomeVisible&&nativeHome.screen()==13&&nativeStreamingPrivacy!=nativePlaybackPrivacy()&&!nativeStreamingToken.isEmpty()) requestNativeStreaming("page",nativeStreamingToken,nativeStreamingOffset);
                nativeHome.playerPrivacy(nativePlaybackPrivacy());
                if(nativePlaybackPrivacy()&&nativePlaybackMediaSession!=null) nativePlaybackMediaSession.redact();
                refreshNativePipActions();
                nativeHome.update(nativeDark, nativeTransferCount, nativeDownloaded, nativeTotal);
                if (nativeHomeVisible && nativeQueueVisible) {
                    nativeHome.updateQueue(queueSnapshotData());
                }
            }
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
        if(nativeHomeVisible&&nativeHome!=null&&nativeHome.screen()==15) {nativeHome.playerLayout(0);nativeHome.playerCommand("snapshot",0);return;}
        if(nativePlaybackFullscreen) {setNativePlaybackFullscreen(false);if(nativeHome!=null) nativeHome.playerCommand("snapshot",0);return;}
        if (fullscreenView != null) {
            hideFullscreenView();
            return;
        }
        if (nativeHomeVisible) {
            if (nativeHome != null && nativeHome.back()) {
                nativeHome.playerVisible(false);nativeHome.playerCommand("pause",0);resetNativePlaybackRotation();
                if(nativePlaybackMediaSession!=null) nativePlaybackMediaSession.deactivate();
                handler.removeCallbacks(nativePlaybackRefresh);
                nativeQueueVisible = false;
                nativeDiagnosticsVisible = false;
                nativeLibraryVisible = nativeHome.screen() == 5||nativeHome.screen()==15;
                handler.removeCallbacks(nativeDiagnosticsRefresh);
                return;
            }
            super.onBackPressed();
        } else if (resetNativeHistory && nativeHome != null) {
            webView.stopLoading();
            showNativeHome();
        } else if (webView.canGoBack()) {
            webView.goBack();
        } else if (showNativeHome()) {
            // Keep the existing WebView, its bridges, and its loaded page alive.
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
        if(nativeHomeVisible&&nativeHome!=null&&NativePlaybackPolicy.playerScreen(nativeHome.screen())) {
            if(!nativePlaybackPip) enterNativePlaybackPip();
            return;
        }
        if (playbackActive && android.os.Build.VERSION.SDK_INT < 31) {
            requestPictureInPicture(16, 9);
        }
    }

    @Override
    public void onPictureInPictureModeChanged(
            boolean isInPictureInPictureMode, Configuration newConfig) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig);
        if(nativePlaybackPip&&nativeHome!=null) {
            if(isInPictureInPictureMode) {
                nativeHome.view().setVisibility(View.GONE);handler.removeCallbacks(nativePipRefresh);handler.post(nativePipRefresh);
                handler.postDelayed(()->{
                    if(!isDestroyed()&&nativePlaybackPip&&isInPictureInPictureMode()&&nativeHome!=null) {
                        View decor=getWindow().getDecorView();nativeHome.playerSurface(0,0,decor.getWidth(),decor.getHeight());
                    }
                },100);
            } else {
                handler.removeCallbacks(nativePipRefresh);nativePlaybackPip=false;nativeHome.playerPip(false);nativeHome.view().setVisibility(View.VISIBLE);
                nativeHome.setActive(nativeHomeVisible&&activityResumed);nativeHome.playerVisible(activityResumed);
                if(!activityResumed) nativeHome.playerCommand("pause",0);
                applyScreenshotPreference();
            }
        }

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
        handler.post(this::refreshNativePreparation);
        activityResumed = true;
        handler.post(nativeActivityRefresh);
        if (nativeHome != null && nativeHomeVisible && nativeHome.screen() == 7) requestNativePeer("snapshot", "{}");
        if (nativeHome != null) nativeHome.setActive(nativeHomeVisible);
        if(nativeHome!=null&&nativeHomeVisible&&NativePlaybackPolicy.playerScreen(nativeHome.screen())) {
            nativeHome.playerVisible(true);nativeHome.playerCommand("snapshot",0);
        }
        if (nativeDiagnosticsVisible) {
            handler.removeCallbacks(nativeDiagnosticsRefresh);
            handler.post(nativeDiagnosticsRefresh);
        }
        applyScreenshotPreference();
        if (!inspectionMode) {
            updateRuntimeTuning();
        }
        if (updateManager != null) {
            updateManager.onResume();
        }
    }

    @Override protected void onPause() {
        activityResumed = false;
        if(!nativePlaybackPip&&nativePlaybackMediaSession!=null) nativePlaybackMediaSession.deactivate();
        handler.removeCallbacks(nativePlaybackRefresh);
        handler.removeCallbacks(nativeActivityRefresh);
        if (nativeHome != null) nativeHome.setActive(false);
        handler.removeCallbacks(nativeDiagnosticsRefresh);
        super.onPause();
    }

    @Override
    protected void onDestroy() {
        handler.removeCallbacks(nativeDiagnosticsRefresh);
        nativeAnimeReads.getQueue().clear();
        nativeAnimeReads.shutdown();
        nativeActions.shutdown();
        if (nativeHome != null) nativeHome.destroy();
        handler.removeCallbacks(nativePipRefresh);
        NativePipReceiver.clear(this);
        if(nativePlaybackMediaSession!=null) {nativePlaybackMediaSession.destroy();nativePlaybackMediaSession=null;}
        if (animeImportReceiverRegistered) unregisterReceiver(animeImportReceiver);
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
        handler.post(this::refreshGalleryIfVisible);
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
