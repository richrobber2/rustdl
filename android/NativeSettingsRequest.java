package app.rustdl;

/** Closed preference command vocabulary shared by native controls and Android. */
final class NativeSettingsRequest {
    static boolean valid(String key, String value) {
        if (key == null || value == null) return false;
        switch (key) {
            case "appearance": return "system".equals(value) || "light".equals(value) || "dark".equals(value);
            case "backgroundTheme": return "space".equals(value) || "rainy-city".equals(value);
            case "mobileDownloadPolicy": return "allow".equals(value) || "ask".equals(value) || "block".equals(value);
            case "diagnosticsRefreshSeconds": return "3".equals(value) || "5".equals(value) || "10".equals(value) || "30".equals(value);
            case "keepScreenAwake": case "allowScreenshots": case "inspectionPrivacy":
            case "spaceEffectEnabled": case "reduceMotion":
                return "true".equals(value) || "false".equals(value);
            case "downloadFolder": case "reset": return "".equals(value);
            default: return false;
        }
    }
}
