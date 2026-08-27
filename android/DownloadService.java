package app.rustdl;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.os.Build;
import android.os.IBinder;

public final class DownloadService extends Service {
    static final String ACTION_UPDATE = "app.rustdl.action.UPDATE_DOWNLOADS";
    static final String EXTRA_COUNT = "count";
    static final String EXTRA_DOWNLOADED = "downloaded";
    static final String EXTRA_TOTAL = "total";
    private static final String CHANNEL_ID = "rustdl_downloads";
    private static final int NOTIFICATION_ID = 2401;

    @Override
    public void onCreate() {
        super.onCreate();
        if (Build.VERSION.SDK_INT >= 26) {
            NotificationChannel channel = new NotificationChannel(
                    CHANNEL_ID,
                    "Video downloads",
                    NotificationManager.IMPORTANCE_LOW);
            channel.setDescription("Active RustDL transfers");
            getSystemService(NotificationManager.class).createNotificationChannel(channel);
        }
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        int count = intent == null ? 0 : intent.getIntExtra(EXTRA_COUNT, 0);
        if (count <= 0) {
            stopForeground(true);
            stopSelf();
            return START_NOT_STICKY;
        }
        long downloaded = intent.getLongExtra(EXTRA_DOWNLOADED, 0L);
        long total = intent.getLongExtra(EXTRA_TOTAL, 0L);
        startForeground(NOTIFICATION_ID, buildNotification(count, downloaded, total));
        return START_NOT_STICKY;
    }

    private Notification buildNotification(int count, long downloaded, long total) {
        Intent open = new Intent(this, MainActivity.class);
        open.setAction(MainActivity.OPEN_QUEUE_ACTION);
        open.addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP | Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent content = PendingIntent.getActivity(
                this,
                0,
                open,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        String title = count == 1 ? "Downloading 1 video" : "Downloading " + count + " videos";
        String detail = total > 0
                ? formatBytes(downloaded) + " of " + formatBytes(total)
                : formatBytes(downloaded) + " saved";
        Notification.Builder builder = Build.VERSION.SDK_INT >= 26
                ? new Notification.Builder(this, CHANNEL_ID)
                : new Notification.Builder(this);
        builder.setSmallIcon(android.R.drawable.stat_sys_download)
                .setContentTitle(title)
                .setContentText(detail)
                .setContentIntent(content)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setCategory(Notification.CATEGORY_PROGRESS)
                .setVisibility(Notification.VISIBILITY_PRIVATE);
        if (total > 0) {
            int progress = (int) Math.min(100L, downloaded * 100L / total);
            builder.setProgress(100, progress, false);
        } else {
            builder.setProgress(0, 0, true);
        }
        return builder.build();
    }

    private static String formatBytes(long bytes) {
        if (bytes < 1024L) return bytes + " B";
        double value = bytes;
        String[] units = {"B", "KB", "MB", "GB"};
        int unit = 0;
        while (value >= 1024.0 && unit + 1 < units.length) {
            value /= 1024.0;
            unit++;
        }
        return String.format(java.util.Locale.US, "%.1f %s", value, units[unit]);
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
