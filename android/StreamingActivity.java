package app.rustdl;

import android.app.Activity;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.drawable.ColorDrawable;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.webkit.CookieManager;
import android.webkit.SafeBrowsingResponse;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Button;
import android.widget.BaseAdapter;
import android.widget.AdapterView;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.Spinner;
import android.widget.TextView;
import android.widget.Toast;

import org.json.JSONArray;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Locale;
import java.util.Map;

/**
 * Streaming-only player kept in a dedicated process with no RustDL JavaScript bridges.
 * Rust supplies an episode-scoped source manifest; this activity never opens the full watch page.
 */
@SuppressWarnings("deprecation")
public final class StreamingActivity extends Activity {
    static final String EXTRA_URL = "app.rustdl.extra.STREAM_URL";
    static final String EXTRA_MANIFEST_URL = "app.rustdl.extra.STREAM_MANIFEST_URL";
    private static final String PROVIDER_REFERER = "https://aniwaves.ru/";
    private static final int MAX_MANIFEST_BYTES = 2_000_000;
    private static boolean dataDirectoryConfigured;

    private final Handler handler = new Handler(Looper.getMainLooper());
    private final ArrayList<Episode> episodes = new ArrayList<>();
    private final ArrayList<Source> sources = new ArrayList<>();
    private final HashSet<Integer> attemptedSources = new HashSet<>();
    private final HashSet<String> allowedSourceHosts = new HashSet<>();

    private FrameLayout root;
    private LinearLayout shell;
    private LinearLayout filterButtons;
    private LinearLayout sourceButtons;
    private WebView webView;
    private ProgressBar progressBar;
    private TextView titleView;
    private TextView statusView;
    private Spinner episodeSpinner;
    private EpisodeAdapter episodeAdapter;
    private Button previousButton;
    private Button nextButton;
    private View fullscreenView;
    private WebChromeClient.CustomViewCallback fullscreenCallback;
    private int previousSystemUiVisibility;
    private String watchUrl;
    private String manifestUrl;
    private String currentEpisode;
    private int currentSource = -1;
    private int manifestGeneration;
    private int sourceGeneration;
    private boolean failoverScheduled;
    private boolean pageFinished;
    private boolean bindingEpisodeSelection;
    private String sourceFilter = "all";
    private String streamTitle = "AniWaves";
    private String posterUrl = "";
    private String watchlistToken = "";
    private boolean watchlisted;

    static boolean isAllowedUrl(String value) {
        if (value == null || value.length() > 500) return false;
        Uri uri = Uri.parse(value);
        String host = uri.getHost();
        String path = uri.getPath();
        return "https".equalsIgnoreCase(uri.getScheme())
                && host != null
                && "aniwaves.ru".equalsIgnoreCase(host)
                && path != null
                && path.startsWith("/watch/")
                && uri.getQuery() == null
                && uri.getFragment() == null
                && uri.getUserInfo() == null;
    }

    private static boolean isAllowedManifestUrl(String value) {
        if (value == null || value.length() > 2_000) return false;
        Uri uri = Uri.parse(value);
        return "http".equalsIgnoreCase(uri.getScheme())
                && "127.0.0.1".equals(uri.getHost())
                && uri.getPort() == 37_658
                && "/__app/stream-manifest.json".equals(uri.getPath())
                && uri.getQueryParameter("url") != null;
    }

    @Override
    protected void onCreate(Bundle state) {
        configureWebViewDirectory();
        super.onCreate(state);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
        getWindow().setStatusBarColor(Color.rgb(9, 10, 15));
        getWindow().setNavigationBarColor(Color.rgb(9, 10, 15));

        watchUrl = getIntent().getStringExtra(EXTRA_URL);
        manifestUrl = getIntent().getStringExtra(EXTRA_MANIFEST_URL);
        if (!isAllowedUrl(watchUrl) || !isAllowedManifestUrl(manifestUrl)) {
            Toast.makeText(this, "That stream manifest is not supported", Toast.LENGTH_LONG).show();
            finish();
            return;
        }

        buildPlayerUi();
        configureStreamingWebView();
        loadManifest(null, false);
        Toast.makeText(this, "Protected player · automatic backup streams enabled",
                Toast.LENGTH_LONG).show();
    }

    private static synchronized void configureWebViewDirectory() {
        if (!dataDirectoryConfigured && Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            WebView.setDataDirectorySuffix("streaming");
            dataDirectoryConfigured = true;
        }
    }

    private void buildPlayerUi() {
        root = new FrameLayout(this);
        shell = new LinearLayout(this);
        shell.setOrientation(LinearLayout.VERTICAL);
        shell.setBackgroundColor(Color.rgb(9, 10, 15));
        root.addView(shell, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT));

        LinearLayout header = new LinearLayout(this);
        header.setGravity(Gravity.CENTER_VERTICAL);
        header.setPadding(dp(8), dp(7), dp(8), dp(5));
        Button close = playerButton("←");
        close.setContentDescription("Back to RustDL");
        close.setOnClickListener(view -> finish());
        header.addView(close, new LinearLayout.LayoutParams(dp(48), dp(42)));

        titleView = new TextView(this);
        titleView.setText("Finding streams…");
        titleView.setTextColor(Color.rgb(245, 247, 250));
        titleView.setTextSize(14);
        titleView.setMaxLines(2);
        titleView.setPadding(dp(9), 0, dp(9), 0);
        header.addView(titleView, new LinearLayout.LayoutParams(0, dp(48), 1));

        episodeSpinner = new Spinner(this, Spinner.MODE_DROPDOWN);
        episodeSpinner.setContentDescription("Select episode");
        episodeSpinner.setEnabled(false);
        episodeSpinner.setPopupBackgroundDrawable(new ColorDrawable(Color.rgb(18, 21, 30)));
        episodeSpinner.setDropDownWidth(dp(300));
        episodeSpinner.setDropDownVerticalOffset(dp(4));
        episodeSpinner.setBackgroundTintList(ColorStateList.valueOf(Color.rgb(45, 50, 64)));
        episodeAdapter = new EpisodeAdapter();
        episodeSpinner.setAdapter(episodeAdapter);
        episodeSpinner.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener() {
            @Override
            public void onItemSelected(AdapterView<?> parent, View view, int position, long id) {
                if (bindingEpisodeSelection || position < 0 || position >= episodes.size()) return;
                Episode selected = episodes.get(position);
                if (!selected.number.equals(currentEpisode)) selectEpisode(selected.number);
            }

            @Override
            public void onNothingSelected(AdapterView<?> parent) {
            }
        });
        header.addView(episodeSpinner, new LinearLayout.LayoutParams(dp(92), dp(42)));

        previousButton = playerButton("‹");
        previousButton.setContentDescription("Previous episode");
        previousButton.setOnClickListener(view -> changeEpisode(-1));
        header.addView(previousButton, new LinearLayout.LayoutParams(dp(48), dp(42)));
        nextButton = playerButton("›");
        nextButton.setContentDescription("Next episode");
        nextButton.setOnClickListener(view -> changeEpisode(1));
        header.addView(nextButton, new LinearLayout.LayoutParams(dp(48), dp(42)));
        shell.addView(header, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(58)));

        HorizontalScrollView filterScroll = new HorizontalScrollView(this);
        filterScroll.setHorizontalScrollBarEnabled(false);
        filterScroll.setFillViewport(true);
        filterButtons = new LinearLayout(this);
        filterButtons.setGravity(Gravity.CENTER_VERTICAL);
        filterButtons.setPadding(dp(8), 0, dp(8), dp(4));
        filterScroll.addView(filterButtons, new HorizontalScrollView.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.MATCH_PARENT));
        shell.addView(filterScroll, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(43)));

        HorizontalScrollView sourceScroll = new HorizontalScrollView(this);
        sourceScroll.setHorizontalScrollBarEnabled(false);
        sourceScroll.setFillViewport(true);
        sourceButtons = new LinearLayout(this);
        sourceButtons.setGravity(Gravity.CENTER_VERTICAL);
        sourceButtons.setPadding(dp(8), 0, dp(8), dp(7));
        sourceScroll.addView(sourceButtons, new HorizontalScrollView.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT,
                ViewGroup.LayoutParams.MATCH_PARENT));
        shell.addView(sourceScroll, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(49)));
        renderFilterButtons();

        FrameLayout playerFrame = new FrameLayout(this);
        webView = new WebView(this);
        webView.setBackgroundColor(Color.BLACK);
        webView.setKeepScreenOn(true);
        playerFrame.addView(webView, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT));

        statusView = new TextView(this);
        statusView.setGravity(Gravity.CENTER);
        statusView.setTextColor(Color.rgb(190, 197, 210));
        statusView.setTextSize(14);
        statusView.setPadding(dp(24), dp(18), dp(24), dp(18));
        statusView.setBackgroundColor(Color.rgb(9, 10, 15));
        playerFrame.addView(statusView, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT));

        progressBar = new ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal);
        progressBar.setMax(100);
        FrameLayout.LayoutParams progressLayout = new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, dp(3));
        progressLayout.gravity = Gravity.TOP;
        playerFrame.addView(progressBar, progressLayout);
        shell.addView(playerFrame, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, 0, 1));
        setContentView(root);
    }

    private Button playerButton(String text) {
        Button button = new Button(this);
        button.setText(text);
        button.setTextColor(Color.rgb(223, 229, 239));
        button.setTextSize(16);
        button.setAllCaps(false);
        button.setPadding(dp(6), 0, dp(6), 0);
        button.setBackgroundTintList(ColorStateList.valueOf(Color.rgb(25, 28, 38)));
        return button;
    }

    private void configureStreamingWebView() {
        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setAllowFileAccess(false);
        settings.setAllowContentAccess(false);
        settings.setAllowFileAccessFromFileURLs(false);
        settings.setAllowUniversalAccessFromFileURLs(false);
        settings.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        settings.setMediaPlaybackRequiresUserGesture(false);
        settings.setJavaScriptCanOpenWindowsAutomatically(false);
        settings.setSupportMultipleWindows(false);
        settings.setBuiltInZoomControls(false);
        settings.setDisplayZoomControls(false);
        settings.setSaveFormData(false);
        settings.setSafeBrowsingEnabled(true);

        CookieManager cookies = CookieManager.getInstance();
        cookies.setAcceptCookie(true);
        cookies.setAcceptThirdPartyCookies(webView, false);
        webView.setDownloadListener((url, userAgent, disposition, type, length) ->
                Toast.makeText(this, "Downloads are disabled in streaming mode",
                        Toast.LENGTH_SHORT).show());

        webView.setWebChromeClient(new WebChromeClient() {
            @Override
            public void onProgressChanged(WebView view, int progress) {
                progressBar.setProgress(progress);
                progressBar.setVisibility(progress >= 100 ? View.GONE : View.VISIBLE);
            }

            @Override
            public boolean onCreateWindow(
                    WebView view, boolean dialog, boolean userGesture, android.os.Message result) {
                Toast.makeText(StreamingActivity.this, "Popup blocked",
                        Toast.LENGTH_SHORT).show();
                return false;
            }

            @Override
            public void onPermissionRequest(android.webkit.PermissionRequest request) {
                request.deny();
            }

            @Override
            public void onShowCustomView(View view, CustomViewCallback callback) {
                showFullscreen(view, callback);
            }

            @Override
            public void onHideCustomView() {
                hideFullscreen();
            }
        });
        webView.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
                if (!request.isForMainFrame()) return false;
                Uri uri = request.getUrl();
                if (isSafePlayerUri(uri)
                        && allowedSourceHosts.contains(normalizeHost(uri.getHost()))) {
                    return false;
                }
                String label = currentSource >= 0 && currentSource < sources.size()
                        ? sources.get(currentSource).label
                        : "this source";
                Toast.makeText(StreamingActivity.this,
                        "External redirect blocked for " + label,
                        Toast.LENGTH_SHORT).show();
                return true;
            }

            @Override
            public void onReceivedError(
                    WebView view,
                    WebResourceRequest request,
                    WebResourceError error) {
                if (request.isForMainFrame()) queueFailover("Stream failed");
            }

            @Override
            public void onReceivedHttpError(
                    WebView view,
                    WebResourceRequest request,
                    WebResourceResponse response) {
                if (request.isForMainFrame() && response.getStatusCode() >= 400) {
                    queueFailover("Server returned " + response.getStatusCode());
                }
            }

            @Override
            public void onPageFinished(WebView view, String url) {
                pageFinished = true;
                progressBar.setVisibility(View.GONE);
                statusView.setVisibility(View.GONE);
                view.clearHistory();
            }

            @Override
            public void onSafeBrowsingHit(
                    WebView view,
                    WebResourceRequest request,
                    int threatType,
                    SafeBrowsingResponse callback) {
                callback.backToSafety(true);
                queueFailover("Unsafe server blocked");
            }
        });
    }

    private void loadManifest(String episode, boolean refresh) {
        int generation = ++manifestGeneration;
        showStatus("Checking primary and backup streams…");
        sourceButtons.removeAllViews();
        episodeSpinner.setEnabled(false);
        previousButton.setEnabled(false);
        nextButton.setEnabled(false);
        new Thread(() -> {
            try {
                Uri.Builder builder = Uri.parse(manifestUrl).buildUpon();
                if (episode != null) builder.appendQueryParameter("episode", episode);
                if (refresh) builder.appendQueryParameter("refresh", "1");
                JSONObject manifest = requestManifest(builder.build().toString());
                runOnUiThread(() -> {
                    if (generation == manifestGeneration) applyManifest(manifest);
                });
            } catch (Exception error) {
                runOnUiThread(() -> {
                    if (generation == manifestGeneration) showManifestError(error.getMessage());
                });
            }
        }, "rustdl-stream-manifest").start();
    }

    private JSONObject requestManifest(String url) throws Exception {
        HttpURLConnection connection = (HttpURLConnection) new URL(url).openConnection();
        connection.setConnectTimeout(10_000);
        connection.setReadTimeout(35_000);
        connection.setUseCaches(false);
        connection.setRequestProperty("Accept", "application/json");
        try {
            int status = connection.getResponseCode();
            InputStream input = status >= 400
                    ? connection.getErrorStream()
                    : connection.getInputStream();
            String body = readLimited(input);
            JSONObject value = new JSONObject(body);
            if (status != 200) {
                throw new IOException(value.optString("error", "Could not resolve streams"));
            }
            return value;
        } finally {
            connection.disconnect();
        }
    }

    private String readLimited(InputStream input) throws IOException {
        if (input == null) throw new IOException("Empty stream manifest response");
        try (InputStream source = input; ByteArrayOutputStream output = new ByteArrayOutputStream()) {
            byte[] buffer = new byte[8_192];
            int total = 0;
            int read;
            while ((read = source.read(buffer)) != -1) {
                total += read;
                if (total > MAX_MANIFEST_BYTES) {
                    throw new IOException("Stream manifest is unexpectedly large");
                }
                output.write(buffer, 0, read);
            }
            return new String(output.toByteArray(), StandardCharsets.UTF_8);
        }
    }

    private void applyManifest(JSONObject manifest) {
        try {
            currentEpisode = manifest.getString("episode");
            streamTitle = manifest.optString("title", "AniWaves");
            posterUrl = manifest.optString("posterUrl", "");
            watchlistToken = manifest.optString("watchlistToken", "");
            watchlisted = manifest.optBoolean("watchlisted", false);
            titleView.setText(streamTitle);
            episodes.clear();
            JSONArray episodeValues = manifest.getJSONArray("episodes");
            for (int index = 0; index < episodeValues.length(); index++) {
                JSONObject item = episodeValues.getJSONObject(index);
                episodes.add(new Episode(item.getString("number"), item.getString("title")));
            }
            sources.clear();
            JSONArray sourceValues = manifest.getJSONArray("sources");
            for (int index = 0; index < sourceValues.length(); index++) {
                JSONObject item = sourceValues.getJSONObject(index);
                String url = item.getString("url");
                Uri playerUri = Uri.parse(url);
                if (!isSafePlayerUri(playerUri)) continue;
                HashSet<String> allowedHosts = new HashSet<>();
                allowedHosts.add(normalizeHost(playerUri.getHost()));
                JSONArray hostValues = item.optJSONArray("allowedHosts");
                if (hostValues != null) {
                    for (int hostIndex = 0; hostIndex < hostValues.length(); hostIndex++) {
                        String host = normalizeHost(hostValues.optString(hostIndex, ""));
                        if (!host.isEmpty()) allowedHosts.add(host);
                    }
                }
                sources.add(new Source(
                        item.getString("label"),
                        item.getString("language"),
                        url,
                        item.optBoolean("available", false),
                        item.optBoolean("redirected", false),
                        allowedHosts,
                        item.optString("issue", "")));
            }
            if (sources.isEmpty()) throw new IOException("No valid player sources were returned");
            updateEpisodeButtons();
            attemptedSources.clear();
            currentSource = -1;
            int first = firstFilteredSource();
            if (first < 0) {
                sourceFilter = "all";
                first = firstFilteredSource();
            }
            renderFilterButtons();
            renderSourceButtons();
            loadSource(first, false);
        } catch (Exception error) {
            showManifestError(error.getMessage());
        }
    }

    private void renderFilterButtons() {
        if (filterButtons == null) return;
        filterButtons.removeAllViews();
        if (!watchlistToken.isEmpty()) {
            Button save = playerButton(watchlisted ? "✓ Watchlist" : "＋ Watchlist");
            save.setTextSize(12);
            save.setContentDescription((watchlisted ? "Remove " : "Add ")
                    + streamTitle + (watchlisted ? " from watchlist" : " to watchlist"));
            save.setBackgroundTintList(ColorStateList.valueOf(watchlisted
                    ? Color.rgb(112, 223, 201)
                    : Color.rgb(25, 28, 38)));
            save.setTextColor(watchlisted
                    ? Color.rgb(7, 17, 15)
                    : Color.rgb(223, 229, 239));
            save.setOnClickListener(this::toggleWatchlist);
            LinearLayout.LayoutParams saveLayout = new LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT, dp(36));
            saveLayout.setMarginEnd(dp(12));
            filterButtons.addView(save, saveLayout);
        }
        for (String filter : new String[] {"all", "sub", "dub", "ready", "issues"}) {
            int count = sourceCount(filter);
            Button button = playerButton(filterLabel(filter) + (sources.isEmpty() ? "" : " " + count));
            boolean selected = sourceFilter.equals(filter);
            button.setTextSize(12);
            button.setEnabled(filter.equals("all") || count > 0);
            button.setAlpha(button.isEnabled() ? 1f : .35f);
            button.setBackgroundTintList(ColorStateList.valueOf(selected
                    ? Color.rgb(112, 223, 201)
                    : Color.rgb(25, 28, 38)));
            button.setTextColor(selected
                    ? Color.rgb(7, 17, 15)
                    : Color.rgb(223, 229, 239));
            button.setOnClickListener(view -> setSourceFilter(filter));
            LinearLayout.LayoutParams layout = new LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT, dp(36));
            layout.setMarginEnd(dp(7));
            filterButtons.addView(button, layout);
        }
    }

    private void toggleWatchlist(View view) {
        Button button = (Button) view;
        button.setEnabled(false);
        String action = watchlisted ? "remove" : "add";
        new Thread(() -> {
            HttpURLConnection connection = null;
            try {
                Uri endpoint = Uri.parse(manifestUrl).buildUpon()
                        .path("/__app/watchlist")
                        .clearQuery()
                        .build();
                String body = formPart("token", watchlistToken)
                        + "&" + formPart("action", action)
                        + "&" + formPart("url", watchUrl)
                        + "&" + formPart("title", streamTitle)
                        + "&" + formPart("poster", posterUrl)
                        + "&response=json";
                byte[] encoded = body.getBytes(StandardCharsets.UTF_8);
                connection = (HttpURLConnection) new URL(endpoint.toString()).openConnection();
                connection.setConnectTimeout(10_000);
                connection.setReadTimeout(15_000);
                connection.setUseCaches(false);
                connection.setRequestMethod("POST");
                connection.setDoOutput(true);
                connection.setFixedLengthStreamingMode(encoded.length);
                connection.setRequestProperty(
                        "Content-Type", "application/x-www-form-urlencoded; charset=utf-8");
                connection.setRequestProperty("Accept", "application/json");
                try (OutputStream output = connection.getOutputStream()) {
                    output.write(encoded);
                }
                int status = connection.getResponseCode();
                InputStream input = status >= 400
                        ? connection.getErrorStream()
                        : connection.getInputStream();
                JSONObject response = new JSONObject(readLimited(input));
                if (status != 200 || !response.optBoolean("ok", false)) {
                    throw new IOException(response.optString("error", "Watchlist update failed"));
                }
                boolean saved = response.optBoolean("watchlisted", false);
                runOnUiThread(() -> {
                    watchlisted = saved;
                    renderFilterButtons();
                    Toast.makeText(this, saved ? "Saved to watchlist" : "Removed from watchlist",
                            Toast.LENGTH_SHORT).show();
                });
            } catch (Exception error) {
                runOnUiThread(() -> {
                    button.setEnabled(true);
                    Toast.makeText(this, "Could not update watchlist", Toast.LENGTH_SHORT).show();
                });
            } finally {
                if (connection != null) connection.disconnect();
            }
        }, "rustdl-watchlist").start();
    }

    private static String formPart(String name, String value) throws Exception {
        return URLEncoder.encode(name, StandardCharsets.UTF_8.name()) + "="
                + URLEncoder.encode(value == null ? "" : value, StandardCharsets.UTF_8.name());
    }

    private String filterLabel(String filter) {
        if ("sub".equals(filter)) return "SUB";
        if ("dub".equals(filter)) return "DUB";
        if ("ready".equals(filter)) return "Ready";
        if ("issues".equals(filter)) return "Issues";
        return "All";
    }

    private int sourceCount(String filter) {
        int count = 0;
        for (Source source : sources) {
            if (matchesFilter(source, filter)) count++;
        }
        return count;
    }

    private boolean matchesFilter(Source source, String filter) {
        if ("sub".equals(filter) || "dub".equals(filter)) {
            return filter.equalsIgnoreCase(source.language);
        }
        if ("ready".equals(filter)) return source.available;
        if ("issues".equals(filter)) return !source.available;
        return true;
    }

    private void setSourceFilter(String filter) {
        if (sourceCount(filter) == 0 && !"all".equals(filter)) return;
        sourceFilter = filter;
        attemptedSources.clear();
        renderFilterButtons();
        renderSourceButtons();
        if (currentSource < 0
                || !matchesFilter(sources.get(currentSource), sourceFilter)
                || (!"issues".equals(filter) && !sources.get(currentSource).available)) {
            loadSource(firstFilteredSource(), false);
        }
    }

    private int firstFilteredSource() {
        for (int index = 0; index < sources.size(); index++) {
            Source source = sources.get(index);
            if (source.available && matchesFilter(source, sourceFilter)) return index;
        }
        for (int index = 0; index < sources.size(); index++) {
            if (matchesFilter(sources.get(index), sourceFilter)) return index;
        }
        return -1;
    }

    private void renderSourceButtons() {
        sourceButtons.removeAllViews();
        for (int index = 0; index < sources.size(); index++) {
            final int sourceIndex = index;
            Source source = sources.get(index);
            if (!matchesFilter(source, sourceFilter)) continue;
            Button button = playerButton(source.language.toUpperCase(Locale.ROOT)
                    + " · " + source.label
                    + (source.available
                    ? (source.redirected ? " · redirect" : "")
                    : " · " + (source.issue.isEmpty() ? "issue" : source.issue)));
            button.setTextSize(12);
            button.setTag(Integer.valueOf(index));
            button.setContentDescription(source.language.toUpperCase(Locale.ROOT)
                    + " " + source.label + ", "
                    + (source.available ? "ready" : "issue " + source.issue)
                    + (source.redirected ? ", provider redirect checked" : ""));
            button.setOnClickListener(view -> loadSource(sourceIndex, true));
            LinearLayout.LayoutParams layout = new LinearLayout.LayoutParams(
                    ViewGroup.LayoutParams.WRAP_CONTENT, dp(40));
            layout.setMarginEnd(dp(7));
            sourceButtons.addView(button, layout);
        }
        updateSelectedSourceButton();
    }

    private void loadSource(int index, boolean userSelected) {
        if (index < 0 || index >= sources.size()) return;
        if (userSelected) attemptedSources.clear();
        currentSource = index;
        attemptedSources.add(index);
        sourceGeneration++;
        failoverScheduled = false;
        pageFinished = false;
        Source source = sources.get(index);
        Uri uri = Uri.parse(source.url);
        allowedSourceHosts.clear();
        allowedSourceHosts.addAll(source.allowedHosts);
        allowedSourceHosts.add(normalizeHost(uri.getHost()));
        String state = source.available
                ? (source.redirected ? " · checked redirect" : "")
                : (source.issue.isEmpty() ? " · retrying issue" : " · retrying " + source.issue);
        showStatus("Opening " + source.label + state + "…");
        progressBar.setVisibility(View.VISIBLE);
        updateSelectedSourceButton();
        Map<String, String> headers = new HashMap<>();
        headers.put("Referer", PROVIDER_REFERER);
        webView.stopLoading();
        webView.loadUrl(source.url, headers);
        int generation = sourceGeneration;
        handler.postDelayed(() -> {
            if (generation == sourceGeneration && !pageFinished) {
                queueFailover("Server timed out");
            }
        }, 15_000);
    }

    private void updateSelectedSourceButton() {
        for (int index = 0; index < sourceButtons.getChildCount(); index++) {
            View child = sourceButtons.getChildAt(index);
            if (!(child instanceof Button)) continue;
            boolean selected = Integer.valueOf(currentSource).equals(child.getTag());
            child.setBackgroundTintList(ColorStateList.valueOf(selected
                    ? Color.rgb(112, 223, 201)
                    : Color.rgb(25, 28, 38)));
            ((Button) child).setTextColor(selected
                    ? Color.rgb(7, 17, 15)
                    : Color.rgb(223, 229, 239));
        }
    }

    private void queueFailover(String reason) {
        if (failoverScheduled || sources.isEmpty()) return;
        failoverScheduled = true;
        int generation = sourceGeneration;
        handler.postDelayed(() -> {
            if (generation != sourceGeneration) return;
            failoverScheduled = false;
            if (currentSource >= 0 && currentSource < sources.size()) {
                Source failed = sources.get(currentSource);
                failed.available = false;
                failed.issue = reason;
                renderFilterButtons();
                renderSourceButtons();
            }
            int next = nextUnattemptedSource();
            if (next >= 0) {
                Toast.makeText(this, reason + " · trying " + sources.get(next).label,
                        Toast.LENGTH_SHORT).show();
                loadSource(next, false);
            } else {
                showStatus("No more " + filterLabel(sourceFilter)
                        + " streams are ready. Change the filter or tap a source to retry.");
            }
        }, 450);
    }

    private int nextUnattemptedSource() {
        for (int offset = 1; offset <= sources.size(); offset++) {
            int index = (currentSource + offset) % sources.size();
            if (!attemptedSources.contains(index)
                    && matchesFilter(sources.get(index), sourceFilter)) return index;
        }
        return -1;
    }

    private void changeEpisode(int direction) {
        int current = -1;
        for (int index = 0; index < episodes.size(); index++) {
            if (episodes.get(index).number.equals(currentEpisode)) {
                current = index;
                break;
            }
        }
        int target = current + direction;
        if (target < 0 || target >= episodes.size()) return;
        selectEpisode(episodes.get(target).number);
    }

    private void selectEpisode(String number) {
        if (number == null || number.equals(currentEpisode)) return;
        webView.stopLoading();
        loadManifest(number, false);
    }

    private void updateEpisodeButtons() {
        int current = -1;
        for (int index = 0; index < episodes.size(); index++) {
            if (episodes.get(index).number.equals(currentEpisode)) current = index;
        }
        previousButton.setEnabled(current > 0);
        nextButton.setEnabled(current >= 0 && current + 1 < episodes.size());
        previousButton.setAlpha(previousButton.isEnabled() ? 1f : .35f);
        nextButton.setAlpha(nextButton.isEnabled() ? 1f : .35f);
        bindingEpisodeSelection = true;
        episodeAdapter.notifyDataSetChanged();
        if (current >= 0) episodeSpinner.setSelection(current, false);
        episodeSpinner.setEnabled(current >= 0 && !episodes.isEmpty());
        episodeSpinner.setAlpha(episodeSpinner.isEnabled() ? 1f : .35f);
        episodeSpinner.setContentDescription(current >= 0
                ? "Select episode, currently episode " + currentEpisode
                : "Select episode");
        bindingEpisodeSelection = false;
    }

    private void showManifestError(String detail) {
        String safeDetail = detail == null || detail.trim().isEmpty()
                ? "Could not resolve player sources"
                : detail;
        showStatus(safeDetail);
        episodeSpinner.setEnabled(!episodes.isEmpty());
        episodeSpinner.setAlpha(episodeSpinner.isEnabled() ? 1f : .35f);
        sourceButtons.removeAllViews();
        Button retry = playerButton("Retry sources");
        retry.setOnClickListener(view -> loadManifest(currentEpisode, true));
        sourceButtons.addView(retry, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, dp(40)));
    }

    private void showStatus(String text) {
        statusView.setText(text);
        statusView.setVisibility(View.VISIBLE);
        progressBar.setVisibility(View.VISIBLE);
    }

    private static boolean isSafePlayerUri(Uri uri) {
        String host = uri.getHost();
        return "https".equalsIgnoreCase(uri.getScheme())
                && host != null
                && !host.isEmpty()
                && !"localhost".equalsIgnoreCase(host)
                && uri.getUserInfo() == null;
    }

    private static String normalizeHost(String host) {
        return host == null ? "" : host.toLowerCase(Locale.ROOT);
    }

    private void showFullscreen(View view, WebChromeClient.CustomViewCallback callback) {
        if (fullscreenView != null) {
            callback.onCustomViewHidden();
            return;
        }
        fullscreenView = view;
        fullscreenCallback = callback;
        previousSystemUiVisibility = getWindow().getDecorView().getSystemUiVisibility();
        shell.setVisibility(View.GONE);
        root.addView(view, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT));
        getWindow().getDecorView().setSystemUiVisibility(
                View.SYSTEM_UI_FLAG_FULLSCREEN
                        | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY);
    }

    private void hideFullscreen() {
        if (fullscreenView == null) return;
        root.removeView(fullscreenView);
        fullscreenView = null;
        shell.setVisibility(View.VISIBLE);
        getWindow().getDecorView().setSystemUiVisibility(previousSystemUiVisibility);
        if (fullscreenCallback != null) {
            fullscreenCallback.onCustomViewHidden();
            fullscreenCallback = null;
        }
    }

    private int dp(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }

    @Override
    public void onBackPressed() {
        if (fullscreenView != null) {
            hideFullscreen();
        } else {
            finish();
        }
    }

    @Override
    protected void onDestroy() {
        manifestGeneration++;
        sourceGeneration++;
        handler.removeCallbacksAndMessages(null);
        if (webView != null) {
            webView.stopLoading();
            webView.destroy();
        }
        super.onDestroy();
    }

    private static final class Episode {
        final String number;
        final String title;

        Episode(String number, String title) {
            this.number = number;
            this.title = title;
        }
    }

    private final class EpisodeAdapter extends BaseAdapter {
        @Override
        public int getCount() {
            return episodes.size();
        }

        @Override
        public Episode getItem(int position) {
            return episodes.get(position);
        }

        @Override
        public long getItemId(int position) {
            return position;
        }

        @Override
        public View getView(int position, View convertView, ViewGroup parent) {
            TextView view = episodeView(convertView, false);
            view.setText("EP " + getItem(position).number);
            return view;
        }

        @Override
        public View getDropDownView(int position, View convertView, ViewGroup parent) {
            TextView view = episodeView(convertView, true);
            Episode episode = getItem(position);
            String fallback = "Episode " + episode.number;
            view.setText(episode.title.isEmpty() || fallback.equalsIgnoreCase(episode.title)
                    ? "EP " + episode.number
                    : "EP " + episode.number + "  ·  " + episode.title);
            return view;
        }

        private TextView episodeView(View convertView, boolean dropdown) {
            TextView view = convertView instanceof TextView
                    ? (TextView) convertView
                    : new TextView(StreamingActivity.this);
            view.setGravity(dropdown ? Gravity.CENTER_VERTICAL : Gravity.CENTER);
            view.setTextColor(Color.rgb(229, 234, 242));
            view.setTextSize(dropdown ? 14 : 12);
            view.setMaxLines(dropdown ? 2 : 1);
            view.setPadding(dp(dropdown ? 16 : 7), dp(dropdown ? 12 : 0),
                    dp(dropdown ? 16 : 7), dp(dropdown ? 12 : 0));
            view.setBackgroundColor(dropdown
                    ? Color.rgb(18, 21, 30)
                    : Color.TRANSPARENT);
            return view;
        }
    }

    private static final class Source {
        final String label;
        final String language;
        final String url;
        boolean available;
        final boolean redirected;
        final HashSet<String> allowedHosts;
        String issue;

        Source(
                String label,
                String language,
                String url,
                boolean available,
                boolean redirected,
                HashSet<String> allowedHosts,
                String issue) {
            this.label = label;
            this.language = language;
            this.url = url;
            this.available = available;
            this.redirected = redirected;
            this.allowedHosts = new HashSet<>(allowedHosts);
            this.issue = issue == null ? "" : issue;
        }
    }
}
