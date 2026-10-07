package app.rustdl;
import android.app.Activity;
import android.view.MotionEvent;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
/** Controls-only GPUI surface; the restricted decoder remains owned by its activity. */
class NativeStreamingHost extends SurfaceView implements NativeStreaming, SurfaceHolder.Callback {
    private boolean active;
    private final NativeAnimeScroll animeScroll;
    private boolean semanticPrivacy=true;
    private int semanticGeneration=-1;
    private final NativeAccessibility accessibility;
    NativeStreamingHost(Activity activity) {
        super(activity);
        NativeHomeHost.nativeSelectStreamingDecoder();
        NativeHomeHost.nativeCreate(activity);
        accessibility=new NativeAccessibility(this);
        animeScroll=new NativeAnimeScroll(this);
        getHolder().addCallback(this);
        setFocusable(true); setFocusableInTouchMode(true);
    }
    @Override protected void onAttachedToWindow() {super.onAttachedToWindow();accessibility.attached();}
    @Override protected void onDetachedFromWindow() {animeScroll.stop();accessibility.detached();super.onDetachedFromWindow();}
    @Override public android.view.accessibility.AccessibilityNodeProvider getAccessibilityNodeProvider() {return accessibility;}
    @Override public boolean dispatchHoverEvent(MotionEvent event) {return accessibility.hover(event)||super.dispatchHoverEvent(event);}
    public View view() { return this; }
    public boolean ready() { return NativeHomeHost.nativeReady(); }
    public boolean privacyReady() { return NativeHomeHost.nativePrivacyReady(); }
    public void update(String data) {
        boolean privacy=true;int generation=-1;
        try {org.json.JSONObject page=new org.json.JSONObject(data);privacy=page.optBoolean("privacy",true);generation=page.optInt("generation",-1);}
        catch(org.json.JSONException invalid) {}
        if(semanticPrivacy!=privacy||semanticGeneration!=generation) accessibility.invalidate();
        semanticPrivacy=privacy;semanticGeneration=generation;
        NativeHomeHost.nativeStreamingDecoder(data);
    }
    public void setActive(boolean next) { if(!next) animeScroll.stop(); active=next; accessibility.visible(next); NativeHomeHost.nativeActive(next); }
    public void surfaceCreated(SurfaceHolder holder) { attach(holder); }
    public void surfaceChanged(SurfaceHolder holder,int format,int width,int height) { attach(holder); }
    private void attach(SurfaceHolder holder) { NativeHomeHost.nativeSurface(holder.getSurface(),getResources().getDisplayMetrics().density); NativeHomeHost.nativeActive(active); }
    public void surfaceDestroyed(SurfaceHolder holder) { animeScroll.stop(); NativeHomeHost.nativeReleaseSurface(); }
    public boolean onTouchEvent(MotionEvent event) {
        if(!active) return false;
        if(animeScroll.touch(event)) return true;
        int action=event.getActionMasked(), selected=event.getActionIndex();
        for(int index=0;index<event.getPointerCount();index++) {
            int mapped=action;
            if(action==MotionEvent.ACTION_POINTER_DOWN||action==MotionEvent.ACTION_POINTER_UP) {
                if(index!=selected) continue;
                mapped=action==MotionEvent.ACTION_POINTER_DOWN?MotionEvent.ACTION_DOWN:MotionEvent.ACTION_UP;
            }
            NativeHomeHost.nativeTouch(mapped,event.getPointerId(index),event.getX(index),event.getY(index));
        }
        return true;
    }
}
