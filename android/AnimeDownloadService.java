package app.rustdl;

import android.app.*;
import android.content.*;
import android.database.Cursor;
import android.media.*;
import android.net.Uri;
import android.os.*;
import android.provider.MediaStore;
import android.widget.Toast;
import java.io.*;
import java.net.*;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.*;

/** Explicit, user-started episode downloads. Remote players have no Java bridge. */
public final class AnimeDownloadService extends Service {
    static final String ACTION_SAVED = "app.rustdl.action.ANIME_SAVED";
    private static final String CHANNEL = "anime-downloads";
    private static final int NOTICE = 402;
    private volatile boolean busy;
    private volatile boolean cancelled;
    private String cookie, cookieHost, referer, userAgent;
    private String downloadTitle = "Anime download";
    private long downloaded;
    private long lastNotice;
    private NotificationManager notifications;
    private DownloadNetworkPolicy networkPolicy;
    private final Handler main = new Handler(Looper.getMainLooper());

    @Override public IBinder onBind(Intent intent) { return null; }

    @Override public void onCreate() {
        super.onCreate();
        networkPolicy = DownloadNetworkPolicy.get(this);
        notifications = getSystemService(NotificationManager.class);
        notifications.createNotificationChannel(new NotificationChannel(CHANNEL, "Anime downloads", NotificationManager.IMPORTANCE_LOW));
    }

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent == null) { stopSelf(startId); return START_NOT_STICKY; }
        if ("cancel".equals(intent.getAction())) { cancelled = true; if (!busy) stopSelf(); return START_NOT_STICKY; }
        if (busy) { toast("Another episode is downloading. Wait for it to finish."); return START_NOT_STICKY; }
        busy = true; cancelled = false;
        String requestedTitle = intent.getStringExtra("title");
        downloadTitle = requestedTitle == null ? "Anime download" : requestedTitle.substring(0, Math.min(150, requestedTitle.length()));
        startForeground(NOTICE, notification("Preparing episode…", true));
        new Thread(() -> {
            String message;
            try { saveEpisode(intent); message = "Episode saved to your gallery"; }
            catch (Exception error) { message = cancelled ? "Episode download cancelled" : "Download failed: " + safeError(error); }
            final String result = message;
            main.post(() -> {
                stopForeground(STOP_FOREGROUND_DETACH);
                notifications.notify(NOTICE, notification(result, false));
                toast(result); busy = false; stopSelf();
            });
        }, "rustdl-anime-download").start();
        return START_NOT_STICKY;
    }

    private Notification notification(String text, boolean active) {
        Intent gallery = new Intent(this, MainActivity.class);
        gallery.setAction(MainActivity.OPEN_QUEUE_ACTION);
        PendingIntent open = PendingIntent.getActivity(this, NOTICE, gallery, PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        Notification.Builder builder = new Notification.Builder(this, CHANNEL)
                .setSmallIcon(android.R.drawable.stat_sys_download).setContentTitle(downloadTitle)
                .setContentText(text).setStyle(new Notification.BigTextStyle().bigText(text))
                .setContentIntent(open).setOnlyAlertOnce(true).setOngoing(active).setAutoCancel(!active);
        if (active) {
            builder.setProgress(0, 0, true);
            PendingIntent cancel = PendingIntent.getService(this, NOTICE, new Intent(this, AnimeDownloadService.class).setAction("cancel"), PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
            builder.addAction(new Notification.Action.Builder(null, "Cancel", cancel).build());
        }
        return builder.build();
    }

    private void progress(String text) {
        if (SystemClock.elapsedRealtime() - lastNotice < 500) return;
        lastNotice = SystemClock.elapsedRealtime();
        notifications.notify(NOTICE, notification(text, true));
    }

    private void checkCancelled() throws IOException {
        if (cancelled) throw new IOException("Cancelled");
    }

    private static final class NetworkRestricted extends IOException {}

    private void checkNetwork() throws IOException {
        checkCancelled();
        if (networkPolicy.state() != 0) throw new NetworkRestricted();
    }

    private void awaitNetwork() throws IOException {
        while (networkPolicy.state() != 0) {
            checkCancelled();
            progress(DownloadNetworkPolicy.reason(networkPolicy.state()) + " · open queue for options");
            try { Thread.sleep(250); }
            catch (InterruptedException error) { Thread.currentThread().interrupt(); throw new IOException("Interrupted", error); }
        }
        checkCancelled();
    }

    private void saveEpisode(Intent intent) throws Exception {
        URL url = new URL(intent.getStringExtra("url"));
        cookie = intent.getStringExtra("cookie"); cookieHost = url.getHost();
        referer = intent.getStringExtra("referer"); userAgent = intent.getStringExtra("userAgent");
        String identity = intent.getStringExtra("identity");
        if (identity == null || identity.length() > 4096) throw new IOException("Invalid episode");
        byte[] hash = MessageDigest.getInstance("SHA-256").digest(identity.getBytes(StandardCharsets.UTF_8));
        StringBuilder name = new StringBuilder("anime-");
        for (int i = 0; i < 12; i++) name.append(String.format(Locale.ROOT, "%02x", hash[i] & 255));
        name.append(".mp4");
        File library = new File(getFilesDir(), "videos");
        if (!library.isDirectory() && !library.mkdirs()) throw new IOException("Could not open your gallery");
        File output = new File(library, name.toString());
        if (output.isFile()) { publish(output); sendBroadcast(new Intent(ACTION_SAVED).setPackage(getPackageName())); return; }
        File work = new File(getCacheDir(), "anime-" + UUID.randomUUID());
        if (!work.mkdirs()) throw new IOException("Could not create download workspace");
        downloaded = 0;
        try {
            File video = new File(work, "video.source");
            File audio = new File(work, "audio.source");
            File ready = new File(work, "ready.mp4");
            if (intent.getBooleanExtra("hls", false)) {
                AnimePlaylist playlist = playlist(url, 0);
                saveSegments(playlist.segments, video, "video");
                if (playlist.audio != null) saveSegments(playlist(playlist.audio, 0).segments, audio, "audio");
                progress("Preparing offline playback…");
                AnimeMedia.remux(video, audio.isFile() ? audio : video, ready, this::checkCancelled);
            } else {
                try (OutputStream out = new FileOutputStream(video)) { copy(url, out); }
                AnimeMedia.remux(video, video, ready, this::checkCancelled);
            }
            checkCancelled();
            if (ready.length() == 0) throw new IOException("The source contained no playable video");
            if (!ready.renameTo(output)) throw new IOException("Could not save the episode to your gallery");
            publish(output);
            sendBroadcast(new Intent(ACTION_SAVED).setPackage(getPackageName()));
        } finally {
            File[] files = work.listFiles();
            if (files != null) for (File file : files) file.delete();
            work.delete();
        }
    }

    private AnimePlaylist playlist(URL url, int depth) throws IOException {
        if (depth > 4) throw new IOException("Too many nested playlists");
        for (;;) {
        awaitNetwork();
        HttpURLConnection connection = null;
        try {
        connection = connect(url, 0);
        byte[] bytes;
        URL resolved = connection.getURL();
        try (InputStream in = connection.getInputStream(); ByteArrayOutputStream out = new ByteArrayOutputStream()) {
            byte[] buffer = new byte[8192]; int count;
            while (true) {
                checkNetwork();
                count = in.read(buffer);
                if (count == -1) break;
                checkCancelled(); if (out.size() + count > 2 * 1024 * 1024) throw new IOException("Playlist is too large");
                out.write(buffer, 0, count);
            }
            bytes = out.toByteArray();
        } finally { connection.disconnect(); }
        AnimePlaylist result = AnimePlaylist.parse(resolved, new String(bytes, StandardCharsets.UTF_8));
        if (result.variant != null) {
            AnimePlaylist media = playlist(result.variant, depth + 1);
            if (result.audio != null) media.audio = result.audio;
            return media;
        }
        return result;
        } catch (IOException error) {
            if (!(error instanceof NetworkRestricted) && networkPolicy.state() == 0) throw error;
        } finally { if (connection != null) connection.disconnect(); }
        }
    }

    private void saveSegments(List<URL> segments, File file, String track) throws IOException {
        try (OutputStream out = new BufferedOutputStream(new FileOutputStream(file))) {
            int index = 0;
            for (URL segment : segments) { copy(segment, out); progress("Saving " + track + " · " + (++index) + "/" + segments.size() + " segments"); }
        }
    }

    private HttpURLConnection connect(URL initial, long offset) throws IOException {
        URL url = initial;
        for (int redirect = 0; redirect < 6; redirect++) {
            checkNetwork();
            if (!"https".equalsIgnoreCase(url.getProtocol()) || url.getUserInfo() != null || (url.getPort() != -1 && url.getPort() != 443))
                throw new IOException("Unsupported media address");
            for (InetAddress address : InetAddress.getAllByName(url.getHost())) {
                if (address.isAnyLocalAddress() || address.isLoopbackAddress() || address.isLinkLocalAddress() || address.isSiteLocalAddress() || address.isMulticastAddress())
                    throw new IOException("This source is not a public media server");
            }
            HttpURLConnection connection = (HttpURLConnection) url.openConnection();
            connection.setConnectTimeout(15000); connection.setReadTimeout(30000);
            connection.setInstanceFollowRedirects(false); connection.setUseCaches(false);
            connection.setRequestProperty("Accept-Encoding", "identity");
            if (offset > 0) connection.setRequestProperty("Range", "bytes=" + offset + "-");
            if (userAgent != null) connection.setRequestProperty("User-Agent", userAgent);
            if (referer != null) connection.setRequestProperty("Referer", referer);
            if (cookie != null && cookieHost.equalsIgnoreCase(url.getHost())) connection.setRequestProperty("Cookie", cookie);
            int status;
            try { status = connection.getResponseCode(); }
            catch (IOException error) { connection.disconnect(); throw error; }
            if (status >= 300 && status < 400) {
                String next = connection.getHeaderField("Location"); connection.disconnect();
                if (next == null) throw new IOException("Source redirect is missing its destination");
                url = new URL(url, next); continue;
            }
            if (offset > 0) {
                String range = connection.getHeaderField("Content-Range");
                if (status != 206 || range == null || !range.startsWith("bytes " + offset + "-")) {
                    connection.disconnect(); throw new IOException("This source cannot resume after a network change. Retry on Wi-Fi.");
                }
            } else if (status != 200) { connection.disconnect(); throw new IOException("Source returned HTTP " + status + ". Start playback again and retry."); }
            return connection;
        }
        throw new IOException("Too many source redirects");
    }

    private void copy(URL url, OutputStream out) throws IOException {
        long offset = 0;
        for (;;) {
            awaitNetwork();
            HttpURLConnection connection = null;
            try {
                connection = connect(url, offset);
                long expected = connection.getContentLengthLong();
                long startedAt = offset;
                try (InputStream in = connection.getInputStream()) {
                    byte[] buffer = new byte[65536];
                    while (true) {
                        checkNetwork();
                        int count = in.read(buffer);
                        if (count == -1) {
                            if (expected >= 0 && offset - startedAt != expected) throw new IOException("Incomplete episode response");
                            return;
                        }
                        checkCancelled();
                        if (downloaded + count > 4L * 1024 * 1024 * 1024) throw new IOException("Episode exceeds the 4 GB download limit");
                        if (getCacheDir().getUsableSpace() < 64L * 1024 * 1024) throw new IOException("Not enough free storage");
                        out.write(buffer, 0, count);
                        offset += count;
                        downloaded += count;
                        progress("Saving episode · " + downloaded / (1024 * 1024) + " MB");
                        if (expected >= 0 && offset - startedAt == expected) return;
                    }
                }
            } catch (IOException error) {
                if (!(error instanceof NetworkRestricted) && networkPolicy.state() == 0) throw error;
            } finally {
                if (connection != null) connection.disconnect();
            }
        }
    }

    private void publish(File file) throws IOException {
        String folder = getSharedPreferences("rustdl-settings", MODE_PRIVATE).getString("download-folder", SettingsBridge.DEFAULT_DOWNLOAD_FOLDER);
        if (folder == null || !folder.matches("[A-Za-z0-9 ._-]{1,48}") || folder.equals(".") || folder.equals("..")) folder = SettingsBridge.DEFAULT_DOWNLOAD_FOLDER;
        String relative = Environment.DIRECTORY_DOWNLOADS + "/" + folder + "/";
        ContentResolver resolver = getContentResolver(); Uri downloads = MediaStore.Downloads.EXTERNAL_CONTENT_URI;
        try (Cursor cursor = resolver.query(downloads, new String[]{MediaStore.Downloads._ID}, "_display_name=? AND relative_path=?", new String[]{file.getName(), relative}, null)) {
            if (cursor != null && cursor.moveToFirst()) return;
        }
        ContentValues values = new ContentValues(); values.put(MediaStore.MediaColumns.DISPLAY_NAME, file.getName());
        values.put(MediaStore.MediaColumns.RELATIVE_PATH, relative); values.put(MediaStore.MediaColumns.MIME_TYPE, "video/mp4"); values.put(MediaStore.MediaColumns.IS_PENDING, 1);
        Uri destination = resolver.insert(downloads, values); if (destination == null) throw new IOException("Could not publish to Downloads");
        try {
            try (InputStream in = new FileInputStream(file); OutputStream out = resolver.openOutputStream(destination)) {
                if (out == null) throw new IOException("Could not open Downloads"); byte[] buffer = new byte[65536]; int count;
                while ((count = in.read(buffer)) != -1) out.write(buffer, 0, count);
            }
            values.clear(); values.put(MediaStore.MediaColumns.IS_PENDING, 0); resolver.update(destination, values, null, null);
        } catch (IOException error) { resolver.delete(destination, null, null); throw error; }
    }

    @Override public void onDestroy() { cancelled = true; super.onDestroy(); }

    private static String safeError(Exception error) {
        return error instanceof IOException && error.getMessage() != null ? error.getMessage() : "This source could not be saved. Try another source.";
    }
    private void toast(String text) { main.post(() -> Toast.makeText(this, text, Toast.LENGTH_LONG).show()); }
}
