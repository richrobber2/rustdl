package app.rustdl;

import android.app.Activity;
import android.os.Bundle;

/** Separate synthetic GPUI review process: no server, provider requests, stores or decoder. */
public final class NativeVisualActivity extends Activity {
    private NativeStreaming host;
    private String screen="home";
    private boolean frameReady;
    private final android.os.Handler handler=new android.os.Handler(android.os.Looper.getMainLooper());
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        NativeVisualStatusProvider.readyScreen="";
        String requested=getIntent().getStringExtra("screen");
        if("history".equals(requested)||"settings".equals(requested)||"anime".equals(requested))screen=requested;
        getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_SECURE);
        getWindow().getDecorView().setSystemUiVisibility(5894);
        try {
            host=(NativeStreaming)Class.forName("app.rustdl.NativeVisualHost")
                    .getDeclaredConstructor(Activity.class,String.class)
                    .newInstance(this,screen);
            setContentView(host.view());host.setActive(true);awaitFrame(100);
        } catch(ReflectiveOperationException|LinkageError unavailable) {
            android.widget.TextView text=new android.widget.TextView(this);
            text.setText("Native visual review unavailable");setContentView(text);
        }
    }
    @Override protected void onNewIntent(android.content.Intent intent) {
        super.onNewIntent(intent);setIntent(intent);
        String requested=intent.getStringExtra("screen");
        screen=("history".equals(requested)||"settings".equals(requested)||"anime".equals(requested))?requested:"home";
        frameReady=false;NativeVisualStatusProvider.readyScreen="";
        getWindow().addFlags(android.view.WindowManager.LayoutParams.FLAG_SECURE);
        if(host!=null) {host.update(screen);host.setActive(true);awaitFrame(100);}
    }
    @Override protected void onResume() {super.onResume();if(host!=null){host.setActive(true);awaitFrame(100);}}
    private void awaitFrame(int remaining) {
        if(isDestroyed()||host==null)return;
        if(host.ready()&&host.privacyReady()) {
            host.view().postOnAnimation(()->host.view().postOnAnimation(()->{
                if(!isDestroyed()&&host.privacyReady()) {getWindow().clearFlags(android.view.WindowManager.LayoutParams.FLAG_SECURE);frameReady=true;publishReady();}
            }));
        } else if(remaining>0)handler.postDelayed(()->awaitFrame(remaining-1),100);
    }
    private void publishReady() {
        NativeVisualStatusProvider.readyScreen=frameReady&&hasWindowFocus()?"private-synthetic:"+screen:"";
    }
    @Override public void onWindowFocusChanged(boolean focus) {super.onWindowFocusChanged(focus);publishReady();}
    public String nativeSettingsSnapshot(){return "{\"ok\":true,\"detail\":\"\",\"downloadFolder\":\"Synthetic\",\"keepScreenAwake\":false,\"allowScreenshots\":true,\"inspectionPrivacy\":true,\"diagnosticsRefreshSeconds\":15,\"appearance\":\"dark\",\"backgroundTheme\":\"plain\",\"spaceEffectEnabled\":false,\"reduceMotion\":true,\"mobileDownloadPolicy\":\"wifi\"}";}
    public void nativeScreenChanged(String screen) {}
    public void requestNativeAnime(String action,String id,int page) {}
    public void changeNativeSetting(String key,String value) {}
    @Override protected void onPause(){frameReady=false;NativeVisualStatusProvider.readyScreen="";if(host!=null)host.setActive(false);super.onPause();}
    @Override protected void onDestroy(){handler.removeCallbacksAndMessages(null);if(host!=null)host.setActive(false);super.onDestroy();}
}
