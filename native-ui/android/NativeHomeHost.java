package app.rustdl;

import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;

/** A process-lived GPUI render thread behind the existing, retained WebView. */
final class NativeHomeHost extends SurfaceView implements NativeHome, SurfaceHolder.Callback {
    static { System.loadLibrary("rustdl_ui"); }
    private boolean active;
    private final NativeAnimeScroll animeScroll;
    private final NativeAccessibility accessibility;
    private final MainActivity activity;
    private NativePlaybackSession player;
    private NativePlaybackService.LocalBinder playbackOwner;
    private boolean playbackBound, destroyed, pendingResume;
    private String pendingFile="",pendingSource="";
    private java.util.function.Consumer<String> playbackCallback=data->{};
    private final android.content.ServiceConnection playbackConnection=new android.content.ServiceConnection() {
        @Override public void onServiceConnected(android.content.ComponentName name,android.os.IBinder service) {
            if(destroyed) return;
            playbackOwner=(NativePlaybackService.LocalBinder)service;
            if(!playbackOwner.hasPlayer()&&pendingFile.isEmpty()) {
                boolean requested=pendingResume;pendingResume=false;
                if(playbackBound) {activity.unbindService(playbackConnection);playbackBound=false;}playbackOwner=null;
                if(requested) activity.nativePlaybackSnapshot("{\"state\":\"error\",\"detail\":\"Playback session unavailable\"}");return;
            }
            player=playbackOwner.player(activity.nativePlaybackOrigin());playbackOwner.attach(activity);playbackCallback=playbackOwner.callback();
            player.setActive(true);player.setInspectionPrivacy(activity.nativePlaybackPrivacy());player.setVisible(active&&NativePlaybackPolicy.playerScreen(screen()));
            if(!active||!NativePlaybackPolicy.playerScreen(screen())) {pendingFile="";pendingSource="";}
            if(pendingResume) {pendingResume=false;player.command("snapshot",0,playbackCallback);}
            if(!pendingFile.isEmpty()) {String file=pendingFile,source=pendingSource;pendingFile="";pendingSource="";player.open(file,source,playbackCallback);}
        }
        @Override public void onServiceDisconnected(android.content.ComponentName name) {playbackOwner=null;player=null;}
    };

    NativeHomeHost(MainActivity activity) {
        super(activity);
        this.activity=activity;
        nativeCreate(activity);
        accessibility=new NativeAccessibility(this);
        animeScroll=new NativeAnimeScroll(this);
        getHolder().addCallback(this);
        setFocusable(true);
        setFocusableInTouchMode(true);
    }

    @Override protected void onAttachedToWindow() {super.onAttachedToWindow();accessibility.attached();}
    @Override protected void onDetachedFromWindow() {animeScroll.stop();accessibility.detached();super.onDetachedFromWindow();}
    @Override public android.view.accessibility.AccessibilityNodeProvider getAccessibilityNodeProvider() {return accessibility;}
    @Override public boolean dispatchHoverEvent(MotionEvent event) {return accessibility.hover(event)||super.dispatchHoverEvent(event);}
    @Override public View view() { return this; }
    @Override public void updateLibrary(String library) { nativeLibrary(library); }
    @Override public void updateDiscovery(String discovery) { nativeDiscovery(discovery); }
    @Override public void updateDiagnostics(String diagnostics) { nativeDiagnostics(diagnostics); }
    @Override public void showQueue() { nativeShowQueue(); }
    @Override public void showDiscovery() { nativeShowDiscovery(); }
    @Override public void showPeers() { nativeShowPeers(); }
    @Override public void updatePeers(String peers) { nativePeers(peers); }
    @Override public void updateAnime(String anime) { nativeAnime(anime); }
    @Override public void updateStorage(String storage) { nativeStorage(storage); }
    @Override public void updateActivity(String activity) { nativeActivity(activity); }
    @Override public void updateUpdates(String updates) { nativeUpdates(updates); }
    @Override public void updateStreaming(String data) {nativeStreaming(data);}
    @Override public void showStreaming() {nativeShowStreaming();}
    @Override public void updatePlayer(String data) {nativePlayer(data);}
    @Override public void showPlayer() {nativeShowPlayer();}
    @Override public void playerLayout(int layout) {nativePlayerLayout(layout);}
    @Override public void resumePlayer() {
        if(destroyed) return;
        pendingFile="";pendingSource="";
        if(playbackOwner!=null&&player!=null) {playbackOwner.attach(activity);player.setInspectionPrivacy(activity.nativePlaybackPrivacy());player.setVisible(active);player.command("snapshot",0,playbackCallback);return;}
        pendingResume=true;
        if(playbackBound) return;
        try {playbackBound=activity.bindService(new android.content.Intent(activity,NativePlaybackService.class),playbackConnection,android.content.Context.BIND_AUTO_CREATE);}
        catch(RuntimeException unavailable) {playbackBound=false;}
        if(!playbackBound) {pendingResume=false;activity.nativePlaybackSnapshot("{\"state\":\"error\",\"detail\":\"Playback session unavailable\"}");}
    }
    @Override public void openPlayer(String filename,String source) {
        if(destroyed) return;
        if(player!=null) {
            try {activity.startForegroundService(new android.content.Intent(activity,NativePlaybackService.class).putExtra("control","begin"));}
            catch(RuntimeException unavailable) {activity.nativePlaybackSnapshot("{\"state\":\"error\",\"detail\":\"Playback service unavailable\"}");return;}
            player.setActive(true);player.setInspectionPrivacy(activity.nativePlaybackPrivacy());player.setVisible(active);player.open(filename,source,playbackCallback);return;
        }
        pendingFile=filename;pendingSource=source;
        if(playbackBound) return;
        android.content.Intent intent=new android.content.Intent(activity,NativePlaybackService.class).putExtra("control","begin");
        try {
            activity.startForegroundService(intent);
            playbackBound=activity.bindService(intent,playbackConnection,android.content.Context.BIND_AUTO_CREATE);
            if(!playbackBound) activity.nativePlaybackSnapshot("{\"state\":\"error\",\"detail\":\"Playback service unavailable\"}");
        }catch(RuntimeException unavailable) {activity.nativePlaybackSnapshot("{\"state\":\"error\",\"detail\":\"Playback service unavailable\"}");}
    }
    @Override public void reopenPlayer(String filename,String source,double position,boolean playing) {
        if(player!=null) {player.setInspectionPrivacy(activity.nativePlaybackPrivacy());player.reopen(filename,source,position,playing,playbackCallback);}
    }
    @Override public void playerCommand(String action,double value) {if(playbackOwner!=null) playbackOwner.command(action,value);}
    @Override public void playerPip(boolean enabled) {if(player!=null) player.setPip(enabled);}
    @Override public void playerPrivacy(boolean enabled) {if(playbackOwner!=null) playbackOwner.privacy(enabled);else if(player!=null) player.setInspectionPrivacy(enabled);}
    @Override public void playerVisible(boolean enabled) {if(!enabled) {pendingFile="";pendingSource="";pendingResume=false;}if(player!=null) player.setVisible(enabled);}
    @Override public void playerSurface(int x,int y,int w,int h) {if(player!=null) player.showSurface(x,y,w,h);}
    @Override public void updateQueue(String queue) { nativeQueue(queue); }
    @Override public void updateSettings(String settings) { accessibility.invalidate(); nativeSettings(settings); }
    @Override public boolean privacyReady() { return nativePrivacyReady(); }
    @Override public int screen() { return nativeScreen(); }
    @Override public boolean back() { return nativeBack(); }
    @Override public boolean ready() { return nativeReady(); }

    @Override public void setActive(boolean next) {
        if (active == next) return;
        if (!next) animeScroll.stop();
        active = next;
        accessibility.visible(next);
        if(!next) {pendingFile="";pendingSource="";pendingResume=false;}
        if(player!=null) player.setActive(true);
        if (!next&&player!=null&&!activity.nativePlaybackInPip()) {activity.resetNativePlaybackRotation();player.hideForBackground();}
        nativeActive(next);
        if (next) requestFocus();
    }

    @Override public void update(boolean dark, int count, long downloaded, long total) {
        nativeUpdate(dark, Math.max(0, count), Math.max(0L, downloaded), Math.max(0L, total));
    }

    @Override public void surfaceCreated(SurfaceHolder holder) { attach(holder); }
    @Override public void surfaceChanged(SurfaceHolder holder, int format, int w, int h) {
        attach(holder);
    }
    private void attach(SurfaceHolder holder) {
        nativeSurface(holder.getSurface(), getResources().getDisplayMetrics().density);
        nativeActive(active);
    }
    @Override public void surfaceDestroyed(SurfaceHolder holder) { animeScroll.stop(); nativeReleaseSurface(); }

    @Override public boolean onTouchEvent(MotionEvent event) {
        if (!active) return false;
        if (animeScroll.touch(event)) return true;
        int action = event.getActionMasked();
        int selected = event.getActionIndex();
        for (int i = 0; i < event.getPointerCount(); i++) {
            int mapped = action;
            if (action == MotionEvent.ACTION_POINTER_DOWN || action == MotionEvent.ACTION_POINTER_UP) {
                if (i != selected) continue;
                mapped = action == MotionEvent.ACTION_POINTER_DOWN
                        ? MotionEvent.ACTION_DOWN : MotionEvent.ACTION_UP;
            }
            nativeTouch(mapped, event.getPointerId(i), event.getX(i), event.getY(i));
        }
        return true;
    }

    @Override public boolean key(KeyEvent event) {
        if (!active || event.getKeyCode() == KeyEvent.KEYCODE_BACK) return false;
        // Preserve system volume, media, and other non-navigation keys.
        switch (event.getKeyCode()) {
            case KeyEvent.KEYCODE_TAB:
            case KeyEvent.KEYCODE_ENTER:
            case KeyEvent.KEYCODE_SPACE:
            case KeyEvent.KEYCODE_DPAD_CENTER:
            case KeyEvent.KEYCODE_DPAD_UP:
            case KeyEvent.KEYCODE_DPAD_DOWN:
            case KeyEvent.KEYCODE_DPAD_LEFT:
            case KeyEvent.KEYCODE_DPAD_RIGHT:
                nativeKey(event.getKeyCode(), event.getAction(), event.getMetaState());
                return true;
            default: return false;
        }
    }

    @Override public void destroy() {
        destroyed=true;pendingFile="";pendingSource="";
        if(playbackOwner!=null) playbackOwner.detach(activity);
        if(playbackBound) {activity.unbindService(playbackConnection);playbackBound=false;}
        player=null;playbackOwner=null;
        setActive(false);
        // The SurfaceHolder owns release timing; do not release a still-live surface.
    }

    static native void nativeAccessibilityActive(boolean active);
    static native String nativeAccessibilitySnapshot();
    static native boolean nativeAccessibilityAction(String id,int action,double value,long revision,int screen);
    static native void nativeCreate(android.app.Activity activity);
    static native void nativeSelectStreamingDecoder();
    static native void nativeStreamingDecoder(String presentation);
    static native boolean nativeReady();
    private static native boolean nativeBack();
    private static native int nativeScreen();
    static native boolean nativePrivacyReady();
    private static native void nativeSettings(String settings);
    private static native void nativeQueue(String queue);
    private static native void nativeShowQueue();
    private static native void nativeShowDiscovery();
    private static native void nativeShowPeers();
    private static native void nativePeers(String peers);
    private static native void nativeAnime(String anime);
    private static native void nativeStorage(String storage);
    private static native void nativeActivity(String activity);
    private static native void nativeUpdates(String updates);
    private static native void nativeStreaming(String data);
    private static native void nativeShowStreaming();
    private static native void nativePlayer(String player);
    private static native void nativeShowPlayer();
    private static native void nativePlayerLayout(int layout);
    private static native void nativeDiagnostics(String diagnostics);
    private static native void nativeLibrary(String library);
    private static native void nativeDiscovery(String discovery);
    static native void nativeSurface(Surface surface, float scale);
    static native void nativeReleaseSurface();
    static native void nativeActive(boolean active);
    static native long nativeAnimeScrollEpoch();
    static native void nativeAnimeScroll(long epoch, float delta);
    static native void nativeStopAnimeScroll(long epoch);
    static native void nativeTouch(int action, int pointer, float x, float y);
    private static native void nativeKey(int key, int action, int modifiers);
    private static native void nativeUpdate(boolean dark, int count, long downloaded, long total);
}
