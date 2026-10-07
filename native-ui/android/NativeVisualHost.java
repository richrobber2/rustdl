package app.rustdl;
import android.app.Activity;
/** Reuses the real GPUI Android surface with synthetic Rust fixtures only. */
final class NativeVisualHost extends NativeStreamingHost {
    NativeVisualHost(Activity activity,String screen) {
        super(activity);nativeVisual(screen==null?"home":screen);
    }
    @Override public void update(String screen) {nativeVisual(screen);}
    private static native void nativeVisual(String screen);
}
