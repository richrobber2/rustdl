package app.rustdl;

import android.app.ActivityManager;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.os.BatteryManager;
import android.os.Process;
import android.os.StatFs;
import android.os.SystemClock;
import android.webkit.JavascriptInterface;

import java.io.BufferedReader;
import java.io.FileReader;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** Privacy-scoped telemetry collected entirely inside the installed APK. */
final class DiagnosticsBridge {
    private static final Pattern LOAD_AVERAGE = Pattern.compile(
            "^([0-9.]+)\\s+([0-9.]+)\\s+([0-9.]+)");
    private final MainActivity activity;

    DiagnosticsBridge(MainActivity activity) {
        this.activity = activity;
    }

    @JavascriptInterface
    public boolean copySnapshot(String snapshot) {
        if (snapshot == null || snapshot.length() > 65_536) return false;
        try {
            ClipboardManager clipboard = (ClipboardManager) activity.getSystemService(
                    Context.CLIPBOARD_SERVICE);
            if (clipboard == null) return false;
            clipboard.setPrimaryClip(ClipData.newPlainText("RustDL diagnostics", snapshot));
            return true;
        } catch (RuntimeException unavailable) {
            return false;
        }
    }

    @JavascriptInterface
    public String diagnostics() {
        double[] load = loadAverage();
        long[] memory = memoryState();
        long[] storage = storageState();
        BatteryState battery = batteryState();
        int thermalStatus = thermalStatus();
        int available = 0;
        if (load[0] >= 0) available++;
        if (memory[0] > 0) available++;
        if (storage[0] > 0) available++;
        if (battery.level >= 0 || battery.temperature >= 0) available++;
        if (thermalStatus >= 0) available++;

        String data = "{\"timestamp\":" + System.currentTimeMillis()
                + ",\"uptimeSeconds\":" + (SystemClock.elapsedRealtime() / 1000.0)
                + ",\"processors\":" + Runtime.getRuntime().availableProcessors()
                + ",\"load1\":" + number(load[0])
                + ",\"load5\":" + number(load[1])
                + ",\"load15\":" + number(load[2])
                + ",\"memoryTotalBytes\":" + memory[0]
                + ",\"memoryAvailableBytes\":" + memory[1]
                + ",\"storageTotalBytes\":" + storage[0]
                + ",\"storageAvailableBytes\":" + storage[1]
                + ",\"batteryLevel\":" + battery.level
                + ",\"batteryStatus\":\"" + battery.status + "\""
                + ",\"batteryTemperatureC\":" + number(battery.temperature)
                + ",\"thermalStatus\":" + thermalStatus
                + ",\"availableSources\":" + available
                + ",\"totalSources\":5"
                + ",\"rustdlPid\":" + Process.myPid() + "}";
        return "{\"ok\":true,\"detail\":\"" + available
                + "/5 sources available\",\"data\":" + data + "}";
    }

    private long[] memoryState() {
        try {
            ActivityManager manager = (ActivityManager) activity.getSystemService(
                    Context.ACTIVITY_SERVICE);
            if (manager == null) return new long[]{-1, -1};
            ActivityManager.MemoryInfo memory = new ActivityManager.MemoryInfo();
            manager.getMemoryInfo(memory);
            return new long[]{memory.totalMem, memory.availMem};
        } catch (RuntimeException unavailable) {
            return new long[]{-1, -1};
        }
    }

    private long[] storageState() {
        try {
            StatFs storage = new StatFs(activity.getFilesDir().getAbsolutePath());
            return new long[]{storage.getTotalBytes(), storage.getAvailableBytes()};
        } catch (RuntimeException unavailable) {
            return new long[]{-1, -1};
        }
    }

    private BatteryState batteryState() {
        try {
            Intent battery = activity.registerReceiver(
                    null, new IntentFilter(Intent.ACTION_BATTERY_CHANGED));
            int rawTemperature = battery == null
                    ? -1 : battery.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, -1);
            return new BatteryState(
                    scaledBatteryLevel(battery),
                    rawTemperature < 0 ? -1 : rawTemperature / 10.0,
                    batteryStatus(battery));
        } catch (RuntimeException unavailable) {
            return new BatteryState(-1, -1, "Unavailable");
        }
    }

    private int thermalStatus() {
        try {
            return activity.currentThermalStatus();
        } catch (RuntimeException unavailable) {
            return -1;
        }
    }

    private static double[] loadAverage() {
        try (BufferedReader reader = new BufferedReader(new FileReader("/proc/loadavg"))) {
            String line = reader.readLine();
            Matcher matcher = LOAD_AVERAGE.matcher(line == null ? "" : line);
            if (matcher.find()) {
                return new double[]{
                        Double.parseDouble(matcher.group(1)),
                        Double.parseDouble(matcher.group(2)),
                        Double.parseDouble(matcher.group(3))
                };
            }
        } catch (Exception unavailable) {
            // Android may restrict this proc entry on a future release.
        }
        return new double[]{-1, -1, -1};
    }

    private static int scaledBatteryLevel(Intent battery) {
        if (battery == null) return -1;
        int level = battery.getIntExtra(BatteryManager.EXTRA_LEVEL, -1);
        int scale = battery.getIntExtra(BatteryManager.EXTRA_SCALE, -1);
        return level < 0 || scale <= 0 ? -1 : Math.round(level * 100f / scale);
    }

    private static String batteryStatus(Intent battery) {
        int status = battery == null ? -1
                : battery.getIntExtra(BatteryManager.EXTRA_STATUS, -1);
        switch (status) {
            case BatteryManager.BATTERY_STATUS_CHARGING: return "Charging";
            case BatteryManager.BATTERY_STATUS_DISCHARGING: return "Discharging";
            case BatteryManager.BATTERY_STATUS_FULL: return "Full";
            case BatteryManager.BATTERY_STATUS_NOT_CHARGING: return "Not charging";
            default: return "Unknown";
        }
    }

    private static String number(double value) {
        return Double.isFinite(value) ? Double.toString(value) : "-1";
    }

    private static final class BatteryState {
        final int level;
        final double temperature;
        final String status;

        BatteryState(int level, double temperature, String status) {
            this.level = level;
            this.temperature = temperature;
            this.status = status;
        }
    }
}
