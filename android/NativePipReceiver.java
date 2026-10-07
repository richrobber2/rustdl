package app.rustdl;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import java.lang.ref.WeakReference;
/** Explicit, non-exported PiP actions for the currently living player activity. */
public final class NativePipReceiver extends BroadcastReceiver {
    private static WeakReference<MainActivity> active=new WeakReference<>(null);
    static void bind(MainActivity activity) {active=new WeakReference<>(activity);}
    static void clear(MainActivity activity) {if(active.get()==activity) active.clear();}
    @Override public void onReceive(Context context,Intent intent) {
        MainActivity activity=active.get();if(activity!=null) activity.handleNativePipCommand(intent);
    }
}
