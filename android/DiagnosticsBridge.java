package app.rustdl;

import android.app.ActivityManager;
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
    public String diagnostics() {
        try {
            double[] load = loadAverage();
            ActivityManager manager = (ActivityManager) activity.getSystemService(
                    Context.ACTIVITY_SERVICE);
            ActivityManager.MemoryInfo memory = new ActivityManager.MemoryInfo();
            if (manager != null) manager.getMemoryInfo(memory);

            StatFs storage = new StatFs(activity.getFilesDir().getAbsolutePath());
            Intent battery = activity.registerReceiver(
                    null, new IntentFilter(Intent.ACTION_BATTERY_CHANGED));
            int batteryLevel = scaledBatteryLevel(battery);
            int rawTemperature = battery == null
                    ? -1 : battery.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, -1);
            double batteryTemperature = rawTemperature < 0 ? -1 : rawTemperature / 10.0;

            String data = "{\"timestamp\":" + System.currentTimeMillis()
                    + ",\"uptimeSeconds\":" + (SystemClock.elapsedRealtime() / 1000.0)
                    + ",\"processors\":" + Runtime.getRuntime().availableProcessors()
                    + ",\"load1\":" + number(load[0])
                    + ",\"load5\":" + number(load[1])
                    + ",\"load15\":" + number(load[2])
                    + ",\"memoryTotalBytes\":" + memory.totalMem
                    + ",\"memoryAvailableBytes\":" + memory.availMem
                    + ",\"storageTotalBytes\":" + storage.getTotalBytes()
                    + ",\"storageAvailableBytes\":" + storage.getAvailableBytes()
                    + ",\"batteryLevel\":" + batteryLevel
                    + ",\"batteryStatus\":\"" + batteryStatus(battery) + "\""
                    + ",\"batteryTemperatureC\":" + number(batteryTemperature)
                    + ",\"thermalStatus\":" + activity.currentThermalStatus()
                    + ",\"rustdlPid\":" + Process.myPid() + "}";
            return "{\"ok\":true,\"detail\":\"APK-local\",\"data\":" + data + "}";
        } catch (RuntimeException unavailable) {
            return "{\"ok\":false,\"detail\":\"Android telemetry unavailable\",\"data\":null}";
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
}
