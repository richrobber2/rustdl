package app.rustdl;

import android.app.Activity;
import org.json.JSONObject;
import dev.gpui.mobile.GpuiVideoPlayer;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.RejectedExecutionException;
import java.util.function.Consumer;
import java.util.concurrent.atomic.AtomicLong;

/** Decoder adapter. Only app user actions may open media; agent tools never invoke it. */
final class NativePlaybackSession {
    private final NativePlaybackPreview preview=new NativePlaybackPreview();
    private volatile java.lang.ref.WeakReference<Activity> activity;
    private final PlaybackBridge preferences;
    private final String origin;
    private final ThreadPoolExecutor worker=new ThreadPoolExecutor(1,1,0L,TimeUnit.MILLISECONDS,
            new ArrayBlockingQueue<Runnable>(8),new ThreadPoolExecutor.AbortPolicy());
    private volatile boolean privacy=true,visible,destroyed,active,pip;
    private final AtomicLong epoch=new AtomicLong();
    private volatile long preparedEpoch=-1;
    private volatile int player=-1;
    private String filename="",state="idle",detail="";
    private boolean growing,playIntent;
    private double lastPosition,lastDuration;
    private long lastPositionSavedAt;
    private double speed=1d,volume=1d,restoreVolume=1d;
    private final android.os.Handler timer=new android.os.Handler(android.os.Looper.getMainLooper());
    private volatile long sleepDeadline;
    private volatile Runnable sleepAction;
    private final AtomicLong sleepRevision=new AtomicLong();
    private volatile boolean sleepExpired;
    private android.media.AudioManager.OnAudioFocusChangeListener focusListener;
    private final android.media.AudioManager audio;
    private volatile android.view.View gestureOverlay;
    private int surfacePlayer=-1,surfaceX,surfaceY,surfaceWidth,surfaceHeight;
    NativePlaybackSession(Activity activity,PlaybackBridge preferences,String origin) {this((android.content.Context)activity,preferences,origin);}
    NativePlaybackSession(android.content.Context context,PlaybackBridge preferences,String origin) {
        this.activity=new java.lang.ref.WeakReference<>(context instanceof Activity?(Activity)context:null);
        this.preferences=preferences;this.origin=origin;
        audio=(android.media.AudioManager)context.getApplicationContext().getSystemService(android.content.Context.AUDIO_SERVICE);
    }
    void attachUi(Activity ui) {
        if(ui==null) hideForBackground();else setVisible(false);privacy=true;activity=new java.lang.ref.WeakReference<>(ui);
    }
    private Consumer<String> selectionListener=name->{};
    private String deliveredSelection="";
    void setSelectionListener(Consumer<String> listener) {selectionListener=listener;}
    void open(String filename,String source,Consumer<String> callback) { open(filename,source,Double.NaN,false,false,callback); }
    private long recoveryBytes,recoveryTime;
    private int recoveryAttempts;
    void recoverGrowing(Consumer<String> callback) {
        final long request=epoch.get();
        submit(()->{
            if(!growing||filename.isEmpty()||!active) return;
            String route=NativePlaybackService.nativePlaybackRoute(filename);
            long downloaded=NativePlaybackService.nativePlaybackDownloaded(filename);
            long now=android.os.SystemClock.elapsedRealtime();
            boolean complete="media".equals(route);
            boolean retry="stream".equals(route)&&NativePlaybackPolicy.growingRetryAllowed("error".equals(state),downloaded,recoveryBytes,recoveryAttempts,now-recoveryTime);
            if(!complete&&!retry) return;
            if(retry) {recoveryBytes=downloaded;recoveryAttempts++;recoveryTime=now;}
            double position=player>0?GpuiVideoPlayer.getPosition(player)/1000d:lastPosition;
            open(filename,origin+route+"/"+filename,position,playIntent,true,callback);
        },request,callback);
    }
    private String nextSelection() {
        String queued=preferences.nextQueued(filename);
        return queued.isEmpty()?NativePlaybackService.nativePlaybackNext(filename):queued;
    }
    void currentSelection(Consumer<String> callback) {
        final long request=epoch.get();
        submit(()->{String selected=filename;onUi(()->{if(!destroyed&&request==epoch.get()) callback.accept(selected);});},request,data->{});
    }
    void next(Consumer<String> callback) {
        final long request=epoch.get();
        submit(()->{
            String next=nextSelection();
            String route=next.isEmpty()?"":NativePlaybackService.nativePlaybackRoute(next);
            if(!"media".equals(route)&&!"stream".equals(route)) {deliver(request,callback,snapshot());return;}
            String source=origin+route+"/"+next;
            if(!NativePlaybackPolicy.validSource(next,source,origin)) return;
            preferences.changeQueued(next,false);
            open(next,source,Double.NaN,true,false,callback);
        },request,callback);
    }
    void reopen(String filename,String source,double position,boolean playing,Consumer<String> callback) {
        final long request=epoch.get();
        submit(()->{
            if(!this.filename.equals(filename)||!growing) return;
            double currentPosition=player>0?GpuiVideoPlayer.getPosition(player)/1000d:lastPosition;
            boolean currentPlaying=playIntent;
            open(filename,source,currentPosition,currentPlaying,true,callback);
        },request,callback);
    }
    private void open(String filename,String source,double resume,boolean autoplay,boolean preserveVolume,Consumer<String> callback) {
        final long request=epoch.incrementAndGet();
        if(!preserveVolume) clearSleep();
        visible=false;removeGestureOverlay();hideSurface();
        if (!NativePlaybackPolicy.validSource(filename,source,origin)) {deliver(request,callback,"{\"state\":\"error\",\"detail\":\"Invalid local playback source\"}");return;}
        submit(()->{
            savePosition();releasePlayer();this.filename=filename;playIntent=autoplay;if(!preserveVolume) {lastPosition=0;lastDuration=0;recoveryBytes=0;recoveryAttempts=0;recoveryTime=android.os.SystemClock.elapsedRealtime();}growing=source.startsWith(origin+"stream/");state="loading";detail="";
            player=GpuiVideoPlayer.create(null);
            if (player<=0||GpuiVideoPlayer.setUrl(null,player,source)==null) {
                state="error";releasePlayer();deliver(request,callback,snapshot());return;
            }
            double duration=GpuiVideoPlayer.getDuration(player)/1000d;
            double saved=Double.isFinite(resume)?resume:preferences.getPosition(filename);
            if ((Double.isFinite(resume)?saved>=0d:saved>=5d)&&saved<duration-.5d) GpuiVideoPlayer.seek(player,(long)(saved*1000d));
            speed=preferences.getPlaybackRate();if(!preserveVolume) {volume=1d;restoreVolume=1d;}
            GpuiVideoPlayer.setSpeed(player,(float)speed);GpuiVideoPlayer.setVolume(player,(float)volume);state="paused";
            if (destroyed||request!=epoch.get()) {releasePlayer();return;}
            preparedEpoch=request;
            if(NativePlaybackPolicy.autoplayAllowed(autoplay,active,destroyed)&&!sleepExpired&&acquireFocus(request,callback)) {GpuiVideoPlayer.play(player);state="playing";}
            deliver(request,callback,snapshot());
        },request,callback);
    }
    void command(String action,double value,Consumer<String> callback) {
        if("preview-hide".equals(action)) {preview.hide();return;}
        if("preview".equals(action)) {
            final long request=epoch.get();
            submit(()->{Activity ui=activity.get();if(ui==null||player<=0||filename.endsWith(".m4a")||!NativePlaybackPolicy.previewAvailable(value,GpuiVideoPlayer.getDuration(player)/1000d,GpuiVideoPlayer.getBufferedPercent(player),growing)) {preview.hide();return;}String source=origin+(growing?"stream/":"media/")+filename;
                preview.show(ui,filename,source,origin,value,()->request==epoch.get()&&!pip&&NativePlaybackPolicy.showFrames(privacy,visible,destroyed));
            },request,callback);return;
        }
        final long request=epoch.get();
        if (!("snapshot".equals(action)||"play".equals(action)||"pause".equals(action)||"seek".equals(action)
                ||"speed".equals(action)||"volume".equals(action)||"sleep".equals(action)||"stop".equals(action)||"toggle".equals(action)||"seek-by".equals(action))) return;
        if ("sleep".equals(action)) {
            if (!NativePlaybackPolicy.validSleepMinutes(value)) return;
            timer.post(()->{
                if (destroyed||request!=epoch.get()) return;
                clearSleep();
                if (value>0d) {
                    sleepDeadline=android.os.SystemClock.elapsedRealtime()+(long)(value*60000d);
                    final long sleepRequest=sleepRevision.get();
                    sleepAction=()->{if (!destroyed&&sleepRequest==sleepRevision.get()) {sleepDeadline=0;sleepExpired=true;command("pause",0,callback);}};
                    timer.postDelayed(sleepAction,(long)(value*60000d));
                }
                command("snapshot",0,callback);
            });return;
        }
        submit(()->{
            if("pause".equals(action)||"stop".equals(action)) playIntent=false;
            if ("stop".equals(action)) {savePosition();releasePlayer();state="idle";filename="";}
            else if (player>0) {
                boolean togglePause="toggle".equals(action)&&GpuiVideoPlayer.isPlaying(player);
                if ("play".equals(action)||("toggle".equals(action)&&!togglePause)) {
                    if (NativePlaybackPolicy.autoplayAllowed(true,active,destroyed)) {
                        if (acquireFocus(request,callback)&&NativePlaybackPolicy.autoplayAllowed(true,active,destroyed)) {GpuiVideoPlayer.play(player);state="playing";playIntent=true;sleepExpired=false;detail="";}
                        else {state="paused";playIntent=false;releaseFocus();detail=active?"Audio focus unavailable. Try Play again.":"";}
                    }
                }
                if ("pause".equals(action)||togglePause) {GpuiVideoPlayer.pause(player);state="paused";playIntent=false;releaseFocus();}
                if(("seek-by".equals(action)||"seek".equals(action))&&Double.isFinite(value)) {
                    double current=GpuiVideoPlayer.getPosition(player)/1000d,duration=GpuiVideoPlayer.getDuration(player)/1000d;
                    double requested="seek-by".equals(action)?current+value:value;
                    double target=NativePlaybackPolicy.growingSeekSeconds(requested,current,duration,GpuiVideoPlayer.getBufferedPercent(player),growing);
                    detail=target<NativePlaybackPolicy.seekSeconds(requested,duration)?"Waiting for download":"";
                    GpuiVideoPlayer.seek(player,(long)(target*1000d));
                }
                if ("speed".equals(action)&&Double.isFinite(value)&&value>=.5d&&value<=2d) {GpuiVideoPlayer.setSpeed(player,(float)value);preferences.savePlaybackRate(value);speed=value;}
                if ("volume".equals(action)&&Double.isFinite(value)) {volume=Math.max(0d,Math.min(1d,value));if(volume>0d) restoreVolume=volume;GpuiVideoPlayer.setVolume(player,(float)volume);}
                savePosition();
            }
            deliver(request,callback,snapshot());
        },request,callback);
    }
    void hideForBackground() {visible=false;removeGestureOverlay();hideSurface();}
    boolean inspectionPrivacyEnabled() {return privacy;}
    void setPip(boolean enabled) {pip=enabled;if(!enabled) surfacePlayer=-1;if(enabled) {preview.hide();active=true;visible=true;removeGestureOverlay();}}
    void setActive(boolean enabled) {active=enabled;}
    void setInspectionPrivacy(boolean enabled) {
        if(enabled) preview.hide();
        privacy=enabled;
        if(enabled) removeGestureOverlay();
        if (enabled) onUi(()->{Activity current=activity.get();if(player>0&&current!=null) GpuiVideoPlayer.hideSurface(current,player);surfacePlayer=-1;});
    }
    void setVisible(boolean enabled) {
        visible=enabled;
        if (!enabled) {clearSleep();removeGestureOverlay();}
        if (!enabled) hideSurface();
    }
    private void hideSurface() {preview.hide();onUi(()->{Activity current=activity.get();if(player>0&&current!=null) GpuiVideoPlayer.hideSurface(current,player);surfacePlayer=-1;});}
    private Activity liveActivity() {
        Activity current=activity.get();return current!=null&&!current.isDestroyed()?current:null;
    }
    private void onUi(Runnable action) {
        if(android.os.Looper.myLooper()==android.os.Looper.getMainLooper()) action.run();else timer.post(action);
    }
    void showSurface(int x,int y,int width,int height) {
        if (width<=0||height<=0) return;
        onUi(()->{
            Activity activity=liveActivity();
            if (activity!=null&&!filename.endsWith(".m4a")&&player>0&&preparedEpoch==epoch.get()&&NativePlaybackPolicy.showFrames(privacy,visible,destroyed)
                    &&(surfacePlayer!=player||surfaceX!=x||surfaceY!=y||surfaceWidth!=width||surfaceHeight!=height)) {
                GpuiVideoPlayer.showSurface(activity,player,x,y,width,height);
                if(!pip) installGestureOverlay(x,y,width,height);
                surfacePlayer=player;surfaceX=x;surfaceY=y;surfaceWidth=width;surfaceHeight=height;
            }
        });
    }
    private void installGestureOverlay(int x,int y,int width,int height) {
        Activity activity=liveActivity();if(activity==null) return;
        removeGestureOverlay();
        final long request=epoch.get();
        android.view.View overlay=new android.view.View(activity);
        android.view.GestureDetector detector=new android.view.GestureDetector(activity,new android.view.GestureDetector.SimpleOnGestureListener() {
            @Override public boolean onDown(android.view.MotionEvent event) {return true;}
            private boolean allowed() {return request==epoch.get()&&active&&NativePlaybackPolicy.showFrames(privacy,visible,destroyed);}
            @Override public boolean onSingleTapConfirmed(android.view.MotionEvent event) {if(allowed()) command("toggle",0,((MainActivity)activity)::nativePlaybackSnapshot);return true;}
            @Override public boolean onDoubleTap(android.view.MotionEvent event) {if(allowed()) command("seek-by",NativePlaybackPolicy.doubleTapSeek(event.getX(),width),((MainActivity)activity)::nativePlaybackSnapshot);return true;}
        });
        overlay.setOnTouchListener((view,event)->detector.onTouchEvent(event));
        android.widget.FrameLayout.LayoutParams bounds=new android.widget.FrameLayout.LayoutParams(width,height);bounds.leftMargin=x;bounds.topMargin=y;
        activity.addContentView(overlay,bounds);gestureOverlay=overlay;
    }
    private void removeGestureOverlay() {
        final android.view.View previous=gestureOverlay;
        if(previous==null) return;
        onUi(()->{if(previous.getParent() instanceof android.view.ViewGroup) ((android.view.ViewGroup)previous.getParent()).removeView(previous);if(gestureOverlay==previous) gestureOverlay=null;});
    }
    void destroy() {
        preview.destroy();
        if (destroyed) return;removeGestureOverlay();clearSleep();destroyed=true;visible=false;epoch.incrementAndGet();
        onUi(()->{Activity current=activity.get();if(player>0&&current!=null) GpuiVideoPlayer.hideSurface(current,player);surfacePlayer=-1;});
        worker.getQueue().clear();
        worker.execute(()->{try {savePosition();} finally {releasePlayer();}});worker.shutdown();
    }
    private void clearSleep() {
        sleepRevision.incrementAndGet();sleepDeadline=0;sleepExpired=false;
        final Runnable previous=sleepAction;sleepAction=null;
        if(previous!=null) {
            if(android.os.Looper.myLooper()==android.os.Looper.getMainLooper()) timer.removeCallbacks(previous);
            else timer.post(()->timer.removeCallbacks(previous));
        }
    }
    private boolean acquireFocus(long request,Consumer<String> callback) {
        if(audio==null) return false;
        if(focusListener!=null) return true;
        focusListener=change->{
            if(NativePlaybackPolicy.pauseForFocusChange(change)) {
                timer.post(()->{if(!destroyed&&request==epoch.get()) command("pause",0,callback);});
            }
        };
        if(audio.requestAudioFocus(focusListener,android.media.AudioManager.STREAM_MUSIC,android.media.AudioManager.AUDIOFOCUS_GAIN)==android.media.AudioManager.AUDIOFOCUS_REQUEST_GRANTED) return true;
        focusListener=null;return false;
    }
    private void releaseFocus() {
        android.media.AudioManager.OnAudioFocusChangeListener previous=focusListener;focusListener=null;
        if(audio!=null&&previous!=null) try {audio.abandonAudioFocus(previous);}catch(RuntimeException unavailable) {}
    }
    private void savePosition() {
        if (player>0&&!filename.isEmpty()) {preferences.savePosition(filename,GpuiVideoPlayer.getPosition(player)/1000d,GpuiVideoPlayer.getDuration(player)/1000d);lastPositionSavedAt=android.os.SystemClock.elapsedRealtime();}
    }
    private void releasePlayer() {removeGestureOverlay();releaseFocus();preparedEpoch=-1;int previous=player;player=-1;
        if(previous>0) try {GpuiVideoPlayer.dispose(previous);}catch(RuntimeException unavailable) {}
    }
    private String snapshot() {
        try {
            if(player>0&&GpuiVideoPlayer.hasError(player)) {state="error";detail="Playback unavailable";releasePlayer();}
            double position=player>0?GpuiVideoPlayer.getPosition(player)/1000d:lastPosition;
            double duration=player>0?GpuiVideoPlayer.getDuration(player)/1000d:lastDuration;
            if(Double.isFinite(position)&&position>=0) lastPosition=position;
            if(Double.isFinite(duration)&&duration>=0) lastDuration=duration;
            if(player>0&&!filename.isEmpty()&&android.os.SystemClock.elapsedRealtime()-lastPositionSavedAt>=2000L
                    &&Double.isFinite(position)&&position>=0d&&Double.isFinite(duration)&&duration>0d) {
                preferences.savePosition(filename,position,duration);lastPositionSavedAt=android.os.SystemClock.elapsedRealtime();
            }
            boolean playing=player>0&&GpuiVideoPlayer.isPlaying(player);
            if (NativePlaybackPolicy.completed("playing".equals(state),playing,position,duration,growing)) {
                preferences.markWatched(filename);state="completed";playIntent=false;releaseFocus();
            }
            JSONObject result=new JSONObject();result.put("videoWidth",player>0?GpuiVideoPlayer.getWidth(player):0);result.put("videoHeight",player>0?GpuiVideoPlayer.getHeight(player):0);result.put("growing",growing);result.put("playIntent",playIntent);result.put("bufferedPercent",player>0?GpuiVideoPlayer.getBufferedPercent(player):0);result.put("buffering",player>0&&GpuiVideoPlayer.isBuffering(player));result.put("sleepSeconds",Math.max(0L,(sleepDeadline-android.os.SystemClock.elapsedRealtime())/1000L));result.put("audioOnly",filename.endsWith(".m4a"));result.put("volume",volume);result.put("restoreVolume",restoreVolume);result.put("detail",detail);result.put("state",state);result.put("canClearQueue",!preferences.getPlaybackQueue().equals("[]"));result.put("upNextCount",preferences.queuedCount(filename));result.put("canNext",!filename.isEmpty()&&!nextSelection().isEmpty());result.put("speed",speed);result.put("muted",volume==0d);
            JSONObject transfer=new JSONObject(NativePlaybackService.nativePlaybackDetails(filename,privacy));
            result.put("downloaded",transfer.optLong("downloaded",0));result.put("total",transfer.optLong("total",0));result.put("quality",privacy?"":transfer.optString("quality",""));
            result.put("title",privacy?"Downloaded media":filename);
            result.put("audioTracks",player>0?GpuiVideoPlayer.getTrackCount(player,android.media.MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_AUDIO):0);
            result.put("captionTracks",player>0?GpuiVideoPlayer.getTrackCount(player,android.media.MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_SUBTITLE)+GpuiVideoPlayer.getTrackCount(player,android.media.MediaPlayer.TrackInfo.MEDIA_TRACK_TYPE_TIMEDTEXT):0);
            result.put("positionSeconds",position);
            result.put("durationSeconds",duration);
            result.put("playing",playing||(player>0&&GpuiVideoPlayer.isBuffering(player)&&"playing".equals(state)));
            result.put("framesHidden",!NativePlaybackPolicy.showFrames(privacy,visible,destroyed));return result.toString();
        } catch (Exception unavailable) {return "{\"state\":\"error\",\"detail\":\"Playback unavailable\"}";}
    }
    private void submit(Runnable action,long request,Consumer<String> callback) {
        if (destroyed) return;
        try {worker.execute(()->{if (!destroyed&&request==epoch.get()) try {action.run();} catch (RuntimeException unavailable) {state="error";releasePlayer();deliver(request,callback,snapshot());}});}catch(RejectedExecutionException closed){deliver(request,callback,"{\"state\":\"error\"}");}
    }
    private void deliver(long request,Consumer<String> callback,String data) {
        onUi(()->{
            if (destroyed||request!=epoch.get()) return;
            if(!filename.equals(deliveredSelection)) {deliveredSelection=filename;selectionListener.accept(filename);}
            String protectedData=data;
            if (privacy) try {
                JSONObject result=new JSONObject(data);result.put("title","Downloaded media");result.put("quality","");result.put("framesHidden",true);
                protectedData=result.toString();
            } catch (Exception invalid) {protectedData="{\"state\":\"error\",\"title\":\"Downloaded media\",\"framesHidden\":true}";}
            callback.accept(protectedData);
        });
    }
}
