package app.rustdl;
import android.app.Activity;
import android.view.View;
/** Optional GPUI controls in the isolated streaming process. */
interface NativeStreaming {
    View view();
    boolean ready();
    boolean privacyReady();
    void update(String presentation);
    void setActive(boolean active);
    static NativeStreaming create(Activity activity) {
        android.view.accessibility.AccessibilityManager accessibility=
                (android.view.accessibility.AccessibilityManager)activity.getSystemService(android.content.Context.ACCESSIBILITY_SERVICE);
        if(accessibility!=null&&accessibility.isTouchExplorationEnabled()) return null;
        try { return (NativeStreaming)Class.forName("app.rustdl.NativeStreamingHost").getDeclaredConstructor(Activity.class).newInstance(activity); }
        catch(ReflectiveOperationException | LinkageError unavailable) { return null; }
    }
}
