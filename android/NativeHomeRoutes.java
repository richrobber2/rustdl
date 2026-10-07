package app.rustdl;

/** Native commands resolve to fixed local destinations, never arbitrary URLs. */
final class NativeHomeRoutes {
    static String path(String destination) {
        if (destination == null) return null;
        switch (destination) {
            case "library": return "";
            case "anime": return "streaming";
            case "queue": return "queue";
            case "settings": return "settings";
            case "diagnostics": return "diagnostics";
            case "changelog": return "changelog";
            default: return null;
        }
    }
}
