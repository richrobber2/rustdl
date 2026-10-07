package app.rustdl;

import android.media.MediaMetadata;
import android.media.session.MediaSession;
import android.media.session.PlaybackState;
import org.json.JSONObject;

/** Native transport controls. Cached artwork is published only outside inspection privacy. */
final class NativePlaybackMediaSession {
    private final MediaSession session;
    private final android.os.Handler handler=new android.os.Handler(android.os.Looper.getMainLooper());
    private final java.util.concurrent.ThreadPoolExecutor artworkWorker=new java.util.concurrent.ThreadPoolExecutor(
            1,1,0L,java.util.concurrent.TimeUnit.MILLISECONDS,new java.util.concurrent.ArrayBlockingQueue<>(1),
            new java.util.concurrent.ThreadPoolExecutor.DiscardOldestPolicy());
    private String selection="";
    private volatile long artworkGeneration;
    private long artworkAttemptAt;
    private boolean artworkPending;
    private volatile boolean privateMetadata=true,destroyed,active;
    private android.graphics.Bitmap artwork;
    private JSONObject latest=new JSONObject();
    private java.util.function.Function<String,String> artworkResolver=name->"";
    void artworkResolver(java.util.function.Function<String,String> resolver) {artworkResolver=resolver;}
    void selection(String filename) {
        if(selection.equals(filename)) return;
        selection=filename;clearArtwork();latest=new JSONObject();redactMetadata();
    }
    private void clearArtwork() {artworkGeneration++;artwork=null;artworkAttemptAt=0;artworkPending=false;artworkWorker.getQueue().clear();}
    private void requestArtwork() {
        if(destroyed||privateMetadata||!active||selection.isEmpty()||artwork!=null||artworkPending) return;
        long now=android.os.SystemClock.elapsedRealtime();
        if(artworkAttemptAt!=0&&now-artworkAttemptAt<5000) return;
        artworkAttemptAt=now;artworkPending=true;final long request=artworkGeneration;final String name=selection;
        try {artworkWorker.execute(()->{
            if(!NativePlaybackPolicy.artworkResultAllowed(request,artworkGeneration,privateMetadata,active,destroyed)) return;
            android.graphics.Bitmap decoded=null;
            try {
                String path=artworkResolver.apply(name);
                if(!destroyed&&!privateMetadata&&active&&path!=null&&!path.isEmpty()) try(java.io.FileInputStream input=new java.io.FileInputStream(path)) {
                    if(input.getChannel().size()<=8*1024*1024) {
                        android.graphics.BitmapFactory.Options options=new android.graphics.BitmapFactory.Options();options.inJustDecodeBounds=true;
                        android.graphics.BitmapFactory.decodeFileDescriptor(input.getFD(),null,options);
                        int sample=NativePlaybackPolicy.artworkSample(options.outWidth,options.outHeight);
                        if(sample>0) {
                            options.inSampleSize=sample;options.inJustDecodeBounds=false;input.getChannel().position(0);
                            decoded=android.graphics.BitmapFactory.decodeFileDescriptor(input.getFD(),null,options);
                        }
                    }
                }
            }catch(Exception unavailable) {}catch(OutOfMemoryError unavailable) {}
            final android.graphics.Bitmap result=decoded;
            handler.post(()->{
                if(!NativePlaybackPolicy.artworkResultAllowed(request,artworkGeneration,privateMetadata,active,destroyed)||!selection.equals(name)) {
                    if(result!=null) result.recycle();return;
                }
                artworkPending=false;artwork=result;if(result!=null) publishMetadata();
            });
        });}catch(java.util.concurrent.RejectedExecutionException closed) {artworkPending=false;}
    }
    private void publishMetadata() {
        MediaMetadata.Builder metadata=new MediaMetadata.Builder()
                .putString(MediaMetadata.METADATA_KEY_TITLE,privateMetadata?"Downloaded media":latest.optString("title","Downloaded media"))
                .putString(MediaMetadata.METADATA_KEY_ARTIST,"RustDL")
                .putLong(MediaMetadata.METADATA_KEY_DURATION,milliseconds(latest.optDouble("durationSeconds",0)));
        if(!privateMetadata&&artwork!=null) metadata.putBitmap(MediaMetadata.METADATA_KEY_ART,artwork)
                .putBitmap(MediaMetadata.METADATA_KEY_ALBUM_ART,artwork);
        session.setMetadata(metadata.build());
    }
    private void redactMetadata() {session.setMetadata(new MediaMetadata.Builder()
            .putString(MediaMetadata.METADATA_KEY_TITLE,"Downloaded media")
            .putString(MediaMetadata.METADATA_KEY_ARTIST,"RustDL").build());}
    NativePlaybackMediaSession(MainActivity activity) {this(activity,activity::requestNativePlayer);}
    NativePlaybackMediaSession(android.content.Context context,java.util.function.BiConsumer<String,Double> controls) {
        session=new MediaSession(context,"RustDL native playback");
        session.setCallback(new MediaSession.Callback() {
            private void command(String action,double value) {controls.accept(action,value);}
            @Override public void onPlay() {command("play",0);}
            @Override public void onPause() {command("pause",0);}
            @Override public void onStop() {command("stop",0);}
            @Override public void onSeekTo(long position) {if(position>=0) command("seek",position/1000d);}
            @Override public void onSkipToNext() {command("next",0);}
            @Override public void onSkipToPrevious() {command("seek",0);}
            @Override public void onFastForward() {command("seek-by",10);}
            @Override public void onRewind() {command("seek-by",-10);}
        },new android.os.Handler(android.os.Looper.getMainLooper()));
        session.setFlags(MediaSession.FLAG_HANDLES_MEDIA_BUTTONS|MediaSession.FLAG_HANDLES_TRANSPORT_CONTROLS);
    }
    void update(JSONObject data,boolean active,boolean privacy) {
        if(destroyed) return;
        if(privateMetadata!=privacy) {privateMetadata=privacy;clearArtwork();}
        if(this.active&&!active) clearArtwork();
        this.active=active;latest=data;
        double position=data.optDouble("positionSeconds",0),duration=data.optDouble("durationSeconds",0),speed=data.optDouble("speed",1);
        long positionMs=milliseconds(position),durationMs=milliseconds(duration);
        boolean playing=data.optBoolean("playing");
        int state="error".equals(data.optString("state"))?PlaybackState.STATE_ERROR:
                data.optBoolean("buffering")?PlaybackState.STATE_BUFFERING:
                "completed".equals(data.optString("state"))?PlaybackState.STATE_STOPPED:
                playing?PlaybackState.STATE_PLAYING:PlaybackState.STATE_PAUSED;
        long actions=PlaybackState.ACTION_PLAY|PlaybackState.ACTION_PAUSE|PlaybackState.ACTION_PLAY_PAUSE|
                PlaybackState.ACTION_STOP|PlaybackState.ACTION_SEEK_TO|PlaybackState.ACTION_SKIP_TO_PREVIOUS|
                PlaybackState.ACTION_FAST_FORWARD|PlaybackState.ACTION_REWIND;
        if(data.optBoolean("canNext")) actions|=PlaybackState.ACTION_SKIP_TO_NEXT;
        publishMetadata();requestArtwork();
        session.setPlaybackState(new PlaybackState.Builder().setActions(actions)
                .setState(state,positionMs,playing&&Double.isFinite(speed)?(float)speed:0f).build());
        session.setActive(active);
    }
    private static long milliseconds(double seconds) {
        return Double.isFinite(seconds)&&seconds>0? (long)Math.min(seconds*1000d,Long.MAX_VALUE):0L;
    }
    void redact() {privateMetadata=true;clearArtwork();redactMetadata();}
    void deactivate() {active=false;clearArtwork();redactMetadata();session.setActive(false);}
    void destroy() {destroyed=true;active=false;clearArtwork();artworkWorker.shutdownNow();session.setActive(false);session.release();}
}
