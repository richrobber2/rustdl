package app.rustdl;

import android.app.Activity;
import android.os.Bundle;
import android.os.Process;

/** Stops only the isolated inspection process, leaving the normal app untouched. */
public final class InspectionCloserActivity extends Activity {
    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        finishAndRemoveTask();
        Process.killProcess(Process.myPid());
    }
}
