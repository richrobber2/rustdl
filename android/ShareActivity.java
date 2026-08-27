package app.rustdl;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;

public final class ShareActivity extends Activity {
    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Intent forwarded = new Intent(getIntent());
        forwarded.setClass(this, MainActivity.class);
        startActivity(forwarded);
        finish();
    }
}
