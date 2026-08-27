package app.rustdl;

import android.app.Activity;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.content.SharedPreferences;
import android.content.pm.PackageInfo;
import android.content.pm.PackageInstaller;
import android.content.pm.PackageManager;
import android.content.pm.Signature;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.provider.Settings;
import android.view.Gravity;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.widget.Toast;

import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Locale;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

final class UpdateManager {
    private static final String INSTALL_STATUS_ACTION = "app.rustdl.action.UPDATE_STATUS";
    private static final String PREFERENCES = "updates";
    private static final String LAST_CHECK = "last-check";
    private static final long CHECK_INTERVAL_MS = 6L * 60L * 60L * 1000L;
    private static final long MAX_MANIFEST_BYTES = 64L * 1024L;
    private static final long MAX_APK_BYTES = 250L * 1024L * 1024L;
    private static final int CONNECT_TIMEOUT_MS = 12_000;
    private static final int READ_TIMEOUT_MS = 30_000;
    private static final int MAX_REDIRECTS = 5;

    private final Activity activity;
    private final FrameLayout root;
    private final String manifestUrl;
    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private final ExecutorService executor = Executors.newSingleThreadExecutor();
    private final SharedPreferences preferences;
    private final BroadcastReceiver installReceiver;

    private ReadyUpdate readyUpdate;
    private LinearLayout updateBanner;
    private Button updateButton;
    private boolean waitingForInstallPermission;
    private boolean destroyed;

    UpdateManager(Activity activity, FrameLayout root, String manifestUrl) {
        this.activity = activity;
        this.root = root;
        this.manifestUrl = manifestUrl == null ? "" : manifestUrl.trim();
        preferences = activity.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE);
        installReceiver = new BroadcastReceiver() {
            @Override
            public void onReceive(Context context, Intent intent) {
                handleInstallStatus(intent);
            }
        };
        IntentFilter filter = new IntentFilter(INSTALL_STATUS_ACTION);
        if (Build.VERSION.SDK_INT >= 33) {
            activity.registerReceiver(installReceiver, filter, Context.RECEIVER_NOT_EXPORTED);
        } else {
            activity.registerReceiver(installReceiver, filter);
        }
    }

    void start() {
        if (!isHttps(manifestUrl)) {
            return;
        }
        long now = System.currentTimeMillis();
        if (now - preferences.getLong(LAST_CHECK, 0L) < CHECK_INTERVAL_MS) {
            return;
        }
        preferences.edit().putLong(LAST_CHECK, now).apply();
        executor.execute(() -> {
            try {
                Release release = fetchRelease();
                long installedVersion = installedPackage().getLongVersionCode();
                if (release.versionCode <= installedVersion) {
                    return;
                }
                File apk = downloadRelease(release);
                validateApk(apk, release);
                mainHandler.post(() -> showReadyUpdate(new ReadyUpdate(release, apk)));
            } catch (Exception error) {
                // Update checks are intentionally quiet. Normal app use must never be blocked.
            }
        });
    }

    void onResume() {
        if (!waitingForInstallPermission || readyUpdate == null) {
            return;
        }
        if (activity.getPackageManager().canRequestPackageInstalls()) {
            waitingForInstallPermission = false;
            beginInstall();
        } else {
            setButtonState(true, "Allow update");
        }
    }

    void destroy() {
        destroyed = true;
        executor.shutdownNow();
        try {
            activity.unregisterReceiver(installReceiver);
        } catch (IllegalArgumentException ignored) {
        }
    }

    private Release fetchRelease() throws Exception {
        HttpURLConnection connection = openHttps(manifestUrl);
        try {
            requireSuccess(connection);
            byte[] body = readLimited(connection.getInputStream(), MAX_MANIFEST_BYTES);
            JSONObject json = new JSONObject(new String(body, "UTF-8"));
            long versionCode = json.getLong("version_code");
            String versionName = json.optString("version_name", Long.toString(versionCode));
            String apkUrl = json.getString("apk_url").trim();
            String sha256 = json.getString("sha256").trim().toLowerCase(Locale.ROOT);
            long size = json.optLong("size_bytes", -1L);
            if (versionCode <= 0L || !isHttps(apkUrl) || !isSha256(sha256)) {
                throw new IOException("Invalid update manifest");
            }
            if (size > MAX_APK_BYTES) {
                throw new IOException("Update is too large");
            }
            return new Release(versionCode, versionName, apkUrl, sha256, size);
        } finally {
            connection.disconnect();
        }
    }

    private File downloadRelease(Release release) throws Exception {
        File directory = new File(activity.getCacheDir(), "updates");
        if (!directory.isDirectory() && !directory.mkdirs()) {
            throw new IOException("Could not create update cache");
        }
        File destination = new File(directory, "rustdl-" + release.versionCode + ".apk");
        if (destination.isFile()
                && destination.length() <= MAX_APK_BYTES
                && digest(destination).equals(release.sha256)) {
            return destination;
        }
        File pending = new File(directory, destination.getName() + ".part");
        if (pending.exists() && !pending.delete()) {
            throw new IOException("Could not clear partial update");
        }

        HttpURLConnection connection = openHttps(release.apkUrl);
        try {
            requireSuccess(connection);
            long declaredLength = connection.getContentLengthLong();
            if (declaredLength > MAX_APK_BYTES
                    || (release.size >= 0L && declaredLength >= 0L
                    && declaredLength != release.size)) {
                throw new IOException("Unexpected update size");
            }
            MessageDigest digest = MessageDigest.getInstance("SHA-256");
            long total = 0L;
            try (InputStream input = connection.getInputStream();
                 FileOutputStream output = new FileOutputStream(pending)) {
                byte[] buffer = new byte[64 * 1024];
                int count;
                while ((count = input.read(buffer)) != -1) {
                    total += count;
                    if (total > MAX_APK_BYTES) {
                        throw new IOException("Update exceeded size limit");
                    }
                    output.write(buffer, 0, count);
                    digest.update(buffer, 0, count);
                }
                output.flush();
                output.getFD().sync();
            }
            if ((release.size >= 0L && total != release.size)
                    || !hex(digest.digest()).equals(release.sha256)) {
                throw new IOException("Update checksum mismatch");
            }
            if (destination.exists() && !destination.delete()) {
                throw new IOException("Could not replace cached update");
            }
            if (!pending.renameTo(destination)) {
                throw new IOException("Could not publish cached update");
            }
            return destination;
        } finally {
            connection.disconnect();
            if (pending.exists()) {
                pending.delete();
            }
        }
    }

    private void validateApk(File apk, Release release) throws Exception {
        PackageManager manager = activity.getPackageManager();
        PackageInfo candidate = manager.getPackageArchiveInfo(
                apk.getAbsolutePath(), PackageManager.GET_SIGNING_CERTIFICATES);
        PackageInfo installed = installedPackage();
        if (candidate == null
                || !activity.getPackageName().equals(candidate.packageName)
                || candidate.getLongVersionCode() != release.versionCode
                || candidate.getLongVersionCode() <= installed.getLongVersionCode()) {
            throw new IOException("Update package identity mismatch");
        }
        if (!signingCertificateMatches(installed, candidate)) {
            throw new SecurityException("Update signing certificate mismatch");
        }
        if (!digest(apk).equals(release.sha256)) {
            throw new SecurityException("Update checksum changed during validation");
        }
    }

    private PackageInfo installedPackage() throws PackageManager.NameNotFoundException {
        return activity.getPackageManager().getPackageInfo(
                activity.getPackageName(), PackageManager.GET_SIGNING_CERTIFICATES);
    }

    private boolean signingCertificateMatches(PackageInfo installed, PackageInfo candidate)
            throws NoSuchAlgorithmException {
        if (installed.signingInfo == null || candidate.signingInfo == null) {
            return false;
        }
        Signature[] current = installed.signingInfo.getApkContentsSigners();
        Signature[] accepted = candidate.signingInfo.hasPastSigningCertificates()
                ? candidate.signingInfo.getSigningCertificateHistory()
                : candidate.signingInfo.getApkContentsSigners();
        for (Signature currentSignature : current) {
            byte[] currentDigest = certificateDigest(currentSignature);
            for (Signature acceptedSignature : accepted) {
                if (MessageDigest.isEqual(currentDigest, certificateDigest(acceptedSignature))) {
                    return true;
                }
            }
        }
        return false;
    }

    private void showReadyUpdate(ReadyUpdate update) {
        if (destroyed) {
            return;
        }
        readyUpdate = update;
        if (updateBanner != null) {
            root.removeView(updateBanner);
        }
        updateBanner = new LinearLayout(activity);
        updateBanner.setOrientation(LinearLayout.HORIZONTAL);
        updateBanner.setGravity(Gravity.CENTER_VERTICAL);
        updateBanner.setPadding(dp(16), dp(12), dp(12), dp(12));
        GradientDrawable background = new GradientDrawable();
        background.setColor(Color.rgb(24, 29, 39));
        background.setStroke(dp(1), Color.rgb(72, 116, 109));
        background.setCornerRadius(dp(18));
        updateBanner.setBackground(background);
        updateBanner.setElevation(dp(10));

        TextView message = new TextView(activity);
        message.setText("RustDL " + update.release.versionName + " is ready");
        message.setTextColor(Color.WHITE);
        message.setTextSize(14f);
        message.setGravity(Gravity.CENTER_VERTICAL);
        updateBanner.addView(message, new LinearLayout.LayoutParams(
                0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));

        updateButton = new Button(activity);
        updateButton.setAllCaps(false);
        updateButton.setText("Update");
        updateButton.setOnClickListener(view -> requestInstall());
        updateBanner.addView(updateButton, new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        FrameLayout.LayoutParams layout = new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT,
                Gravity.BOTTOM);
        layout.setMargins(dp(12), dp(12), dp(12), dp(16));
        root.addView(updateBanner, layout);
    }

    private void requestInstall() {
        if (readyUpdate == null) {
            return;
        }
        if (!activity.getPackageManager().canRequestPackageInstalls()) {
            waitingForInstallPermission = true;
            setButtonState(false, "Waiting…");
            Intent settings = new Intent(
                    Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                    Uri.parse("package:" + activity.getPackageName()));
            activity.startActivity(settings);
            return;
        }
        beginInstall();
    }

    private void beginInstall() {
        ReadyUpdate update = readyUpdate;
        if (update == null) {
            return;
        }
        setButtonState(false, "Installing…");
        executor.execute(() -> {
            try {
                validateApk(update.apk, update.release);
                stagePackage(update.apk);
            } catch (Exception error) {
                mainHandler.post(() -> {
                    setButtonState(true, "Retry");
                    Toast.makeText(activity, "Update could not be installed", Toast.LENGTH_LONG)
                            .show();
                });
            }
        });
    }

    private void stagePackage(File apk) throws Exception {
        PackageInstaller installer = activity.getPackageManager().getPackageInstaller();
        PackageInstaller.SessionParams params = new PackageInstaller.SessionParams(
                PackageInstaller.SessionParams.MODE_FULL_INSTALL);
        params.setAppPackageName(activity.getPackageName());
        if (Build.VERSION.SDK_INT >= 31) {
            params.setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED);
        }
        int sessionId = installer.createSession(params);
        try (PackageInstaller.Session session = installer.openSession(sessionId)) {
            try (InputStream input = new FileInputStream(apk);
                 OutputStream output = session.openWrite("base.apk", 0L, apk.length())) {
                byte[] buffer = new byte[64 * 1024];
                int count;
                while ((count = input.read(buffer)) != -1) {
                    output.write(buffer, 0, count);
                }
                session.fsync(output);
            }
            Intent status = new Intent(INSTALL_STATUS_ACTION).setPackage(activity.getPackageName());
            PendingIntent callback = PendingIntent.getBroadcast(
                    activity,
                    sessionId,
                    status,
                    PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_MUTABLE);
            session.commit(callback.getIntentSender());
        } catch (Exception error) {
            installer.abandonSession(sessionId);
            throw error;
        }
    }

    private void handleInstallStatus(Intent intent) {
        int status = intent.getIntExtra(
                PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE);
        if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
            Intent confirmation = intent.getParcelableExtra(Intent.EXTRA_INTENT);
            if (confirmation != null) {
                activity.startActivity(confirmation);
                return;
            }
        }
        if (status != PackageInstaller.STATUS_SUCCESS) {
            setButtonState(true, "Retry");
            Toast.makeText(activity, "Android did not install the update", Toast.LENGTH_LONG)
                    .show();
        }
    }

    private HttpURLConnection openHttps(String rawUrl) throws IOException {
        URL current = new URL(rawUrl);
        for (int redirect = 0; redirect <= MAX_REDIRECTS; redirect++) {
            if (!"https".equalsIgnoreCase(current.getProtocol())) {
                throw new IOException("Updates require HTTPS");
            }
            HttpURLConnection connection = (HttpURLConnection) current.openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setConnectTimeout(CONNECT_TIMEOUT_MS);
            connection.setReadTimeout(READ_TIMEOUT_MS);
            connection.setRequestProperty("Accept", "application/json, application/vnd.android.package-archive");
            connection.setRequestProperty("User-Agent", "RustDL Android updater");
            int status = connection.getResponseCode();
            if (status < 300 || status >= 400) {
                return connection;
            }
            String location = connection.getHeaderField("Location");
            connection.disconnect();
            if (location == null) {
                throw new IOException("Update redirect has no destination");
            }
            current = new URL(current, location);
        }
        throw new IOException("Too many update redirects");
    }

    private static void requireSuccess(HttpURLConnection connection) throws IOException {
        int status = connection.getResponseCode();
        if (status < 200 || status >= 300) {
            throw new IOException("Update server returned HTTP " + status);
        }
    }

    private static byte[] readLimited(InputStream input, long limit) throws IOException {
        try (InputStream source = input; ByteArrayOutputStream output = new ByteArrayOutputStream()) {
            byte[] buffer = new byte[8 * 1024];
            long total = 0L;
            int count;
            while ((count = source.read(buffer)) != -1) {
                total += count;
                if (total > limit) {
                    throw new IOException("Update response exceeded size limit");
                }
                output.write(buffer, 0, count);
            }
            return output.toByteArray();
        }
    }

    private static String digest(File file) throws IOException, NoSuchAlgorithmException {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (InputStream input = new FileInputStream(file)) {
            byte[] buffer = new byte[64 * 1024];
            int count;
            while ((count = input.read(buffer)) != -1) {
                digest.update(buffer, 0, count);
            }
        }
        return hex(digest.digest());
    }

    private static byte[] certificateDigest(Signature signature)
            throws NoSuchAlgorithmException {
        return MessageDigest.getInstance("SHA-256").digest(signature.toByteArray());
    }

    private static String hex(byte[] bytes) {
        StringBuilder value = new StringBuilder(bytes.length * 2);
        for (byte item : bytes) {
            value.append(String.format(Locale.ROOT, "%02x", item & 0xff));
        }
        return value.toString();
    }

    private static boolean isHttps(String value) {
        try {
            return "https".equalsIgnoreCase(new URL(value).getProtocol());
        } catch (Exception error) {
            return false;
        }
    }

    private static boolean isSha256(String value) {
        if (value.length() != 64) {
            return false;
        }
        for (int index = 0; index < value.length(); index++) {
            char item = value.charAt(index);
            if (!((item >= '0' && item <= '9') || (item >= 'a' && item <= 'f'))) {
                return false;
            }
        }
        return true;
    }

    private void setButtonState(boolean enabled, String text) {
        if (updateButton == null) {
            return;
        }
        updateButton.setEnabled(enabled);
        updateButton.setText(text);
    }

    private int dp(int value) {
        return Math.round(value * activity.getResources().getDisplayMetrics().density);
    }

    private static final class Release {
        final long versionCode;
        final String versionName;
        final String apkUrl;
        final String sha256;
        final long size;

        Release(long versionCode, String versionName, String apkUrl, String sha256, long size) {
            this.versionCode = versionCode;
            this.versionName = versionName;
            this.apkUrl = apkUrl;
            this.sha256 = sha256;
            this.size = size;
        }
    }

    private static final class ReadyUpdate {
        final Release release;
        final File apk;

        ReadyUpdate(Release release, File apk) {
            this.release = release;
            this.apk = apk;
        }
    }
}
