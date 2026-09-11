package app.rustdl;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.Context;
import android.content.SharedPreferences;
import android.net.ConnectivityManager;
import android.net.Network;
import android.net.NetworkCapabilities;
import android.os.Handler;
import android.os.Looper;

/** Process-wide policy, kept alive while foreground download services run. */
final class DownloadNetworkPolicy {
    static final String KEY = "mobile-download-policy";
    private static DownloadNetworkPolicy instance;
    private final ConnectivityManager connectivity;
    private final SharedPreferences preferences;
    private final Handler main = new Handler(Looper.getMainLooper());
    private final SharedPreferences.OnSharedPreferenceChangeListener preferenceListener;
    private Network approvedNetwork;
    private int lastStatus = -1;
    private volatile int status = 1;
    private boolean dialogOpen;

    private static native void nativeSetDownloadNetworkState(int state);

    static synchronized DownloadNetworkPolicy get(Context context) {
        if (instance == null) instance = new DownloadNetworkPolicy(context.getApplicationContext());
        return instance;
    }

    private DownloadNetworkPolicy(Context context) {
        System.loadLibrary("rustdl");
        connectivity = context.getSystemService(ConnectivityManager.class);
        preferences = context.getSharedPreferences("rustdl-settings", Context.MODE_PRIVATE);
        preferenceListener = (prefs, key) -> {
            if (KEY.equals(key) || key == null) main.post(() -> {
                approvedNetwork = null;
                refresh();
            });
        };
        preferences.registerOnSharedPreferenceChangeListener(preferenceListener);
        connectivity.registerDefaultNetworkCallback(new ConnectivityManager.NetworkCallback() {
            @Override public void onAvailable(Network network) { refresh(); }
            @Override public void onLost(Network network) {
                if (network.equals(approvedNetwork)) approvedNetwork = null;
                refresh();
            }
            @Override public void onCapabilitiesChanged(Network network, NetworkCapabilities caps) { refresh(); }
        }, main);
        refresh();
    }

    static int decide(boolean connected, boolean unmetered, String policy, boolean approved) {
        if (!connected) return 1;
        if (unmetered || "allow".equals(policy)) return 0;
        if ("block".equals(policy)) return 2;
        return approved ? 0 : 3;
    }

    private void refresh() {
        Network network = connectivity.getActiveNetwork();
        NetworkCapabilities caps = network == null ? null : connectivity.getNetworkCapabilities(network);
        boolean connected = caps != null && caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED);
        boolean unmetered = caps != null && caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED);
        if (network == null || !network.equals(approvedNetwork) || unmetered) approvedNetwork = null;
        status = decide(connected, unmetered, preferences.getString(KEY, "ask"), approvedNetwork != null);
        if (status != lastStatus) {
            lastStatus = status;
            nativeSetDownloadNetworkState(status);
        }
    }

    int state() { return status; }

    void resetApproval() { main.post(() -> { approvedNetwork = null; refresh(); }); }

    static String reason(int state) {
        switch (state) {
            case 1: return "Waiting for an internet connection";
            case 2: return "Waiting for unmetered Wi-Fi";
            case 3: return "Waiting for mobile-data approval";
            default: return "Downloads allowed on this connection";
        }
    }

    void requestApproval(Activity activity) {
        main.post(() -> {
            refresh();
            if (status != 3 || dialogOpen || activity.isFinishing()) return;
            final Network requestedNetwork = connectivity.getActiveNetwork();
            dialogOpen = true;
            AlertDialog dialog = new AlertDialog.Builder(activity)
                    .setTitle("Download using mobile data?")
                    .setMessage("Allow queued and episode downloads on this metered connection? Approval ends when the connection changes or the app restarts.")
                    .setPositiveButton("Allow this connection", (d, which) -> {
                        if (requestedNetwork != null && requestedNetwork.equals(connectivity.getActiveNetwork())) {
                            approvedNetwork = requestedNetwork;
                            refresh();
                        }
                    })
                    .setNegativeButton("Wait for Wi-Fi", null).create();
            dialog.setOnDismissListener(d -> dialogOpen = false);
            dialog.show();
        });
    }
}
