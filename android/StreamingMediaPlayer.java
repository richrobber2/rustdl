package app.rustdl;
import android.app.Activity;
import android.media.MediaPlayer;
import android.media.AudioManager;
import android.os.Handler;
import android.os.HandlerThread;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
import org.json.JSONObject;
import java.util.Map;

/** RustDL-owned stream decoder. Provider pages only resolve media requests. */
final class StreamingMediaPlayer implements SurfaceHolder.Callback {
    final SurfaceView view;
    private final Activity activity;
    private final HandlerThread thread=new HandlerThread("anime-decoder");
    private final Handler actor;
    private final AudioManager audio;
    private MediaPlayer player;
    private volatile boolean privacy=true,active,destroyed,ready,playing;
    private volatile String state="resolving";
    private volatile double position,duration;
    private volatile int generation,videoWidth,videoHeight;
    private android.view.Surface surface;
    private final AudioManager.OnAudioFocusChangeListener focus=change->{if(change<0)command("pause");};
    StreamingMediaPlayer(Activity activity) {
        this.activity=activity;view=new SurfaceView(activity);view.setKeepScreenOn(true);view.setVisibility(View.INVISIBLE);view.getHolder().addCallback(this);
        audio=(AudioManager)activity.getSystemService(Activity.AUDIO_SERVICE);thread.start();actor=new Handler(thread.getLooper());
    }
    void privacy(boolean value){privacy=value;view.setVisibility(value||!ready||!active?View.INVISIBLE:View.VISIBLE);actor.post(this::attachSurface);}
    void active(boolean value){active=value;if(!value){view.setVisibility(View.INVISIBLE);command("pause");}actor.post(this::attachSurface);}
    void reset(int next){generation=next;view.setVisibility(View.INVISIBLE);state="resolving";ready=false;playing=false;actor.post(this::release);}
    void open(String url,Map<String,String> headers,int expected) {
        if(!StreamingMediaPolicy.playable(url)||destroyed||expected!=generation)return;
        actor.post(()->{
            if(destroyed||expected!=generation)return;release();state="preparing";
            try {
                MediaPlayer candidate=new MediaPlayer();player=candidate;
                candidate.setAudioAttributes(new android.media.AudioAttributes.Builder().setUsage(android.media.AudioAttributes.USAGE_MEDIA).setContentType(android.media.AudioAttributes.CONTENT_TYPE_MOVIE).build());
                candidate.setOnPreparedListener(mp->{if(player!=mp||destroyed||expected!=generation){try{mp.release();}catch(RuntimeException ignored){}return;}ready=true;duration=Math.max(0,mp.getDuration()/1000d);state="paused";attachSurface();activity.runOnUiThread(()->{if(!destroyed&&expected==generation)view.setVisibility(privacy||!active?View.INVISIBLE:View.VISIBLE);});if(active)play();});
                candidate.setOnVideoSizeChangedListener((mp,width,height)->{if(player==mp&&expected==generation){videoWidth=width;videoHeight=height;activity.runOnUiThread(this::fitSurface);}});
                candidate.setOnCompletionListener(mp->{if(player==mp){playing=false;state="completed";releaseFocus();}});
                candidate.setOnErrorListener((mp,what,extra)->{if(player==mp){state="error";release();}return true;});
                candidate.setDataSource(activity,android.net.Uri.parse(url),headers);attachSurface();candidate.prepareAsync();
                actor.postDelayed(()->{if(player==candidate&&!ready&&expected==generation){state="error";release();}},20000);
            }catch(Exception failure){state="error";release();}
        });
    }
    private void attachSurface(){try{if(player!=null)player.setSurface(privacy||!active?null:surface);}catch(RuntimeException failure){state="error";release();}}
    private void play(){if(!active||destroyed||!ready||player==null)return;try{if(audio.requestAudioFocus(focus,AudioManager.STREAM_MUSIC,AudioManager.AUDIOFOCUS_GAIN)!=AudioManager.AUDIOFOCUS_REQUEST_GRANTED){state="focus-unavailable";return;}if(!active||destroyed){releaseFocus();return;}player.start();playing=true;state="playing";}catch(RuntimeException failure){state="error";release();}}
    void command(String action){int expected=generation;actor.post(()->{if(destroyed||expected!=generation||player==null||!ready)return;try{
        switch(action){case "play":play();break;case "pause":player.pause();playing=false;state="paused";releaseFocus();break;
        case "seek-back":case "seek-forward":long next=player.getCurrentPosition()+("seek-back".equals(action)?-10000L:10000L);player.seekTo((int)Math.max(0,Math.min(next,Math.max(0,player.getDuration()))));break;
        default:return;}
    }catch(RuntimeException failure){state="error";release();}});}
    boolean isReady(){return ready;}
    boolean resolving(){return "resolving".equals(state)||"preparing".equals(state);}
    boolean failed(){return "error".equals(state);}
    void append(JSONObject page) throws org.json.JSONException {
        page.put("playbackState",state);page.put("playing",playing);page.put("mediaReady",ready);page.put("positionSeconds",position);page.put("durationSeconds",duration);
        actor.post(()->{if(player!=null&&ready)try{position=Math.max(0,player.getCurrentPosition()/1000d);duration=Math.max(0,player.getDuration()/1000d);}catch(RuntimeException failure){state="error";release();}});
    }
    private void releaseFocus(){try{audio.abandonAudioFocus(focus);}catch(RuntimeException ignored){}}
    private void release(){MediaPlayer previous=player;player=null;ready=false;playing=false;position=0;duration=0;releaseFocus();if(previous!=null)try{previous.release();}catch(RuntimeException ignored){}}
    private void fitSurface(){
        if(destroyed||videoWidth<=0||videoHeight<=0||!(view.getParent() instanceof android.view.View))return;
        android.view.View parent=(android.view.View)view.getParent();int width=parent.getWidth(),height=parent.getHeight();if(width<=0||height<=0)return;
        double scale=Math.min(width/(double)videoWidth,height/(double)videoHeight);int targetWidth=Math.max(1,(int)(videoWidth*scale)),targetHeight=Math.max(1,(int)(videoHeight*scale));
        android.view.ViewGroup.LayoutParams old=view.getLayoutParams();if(old!=null&&old.width==targetWidth&&old.height==targetHeight)return;
        android.widget.FrameLayout.LayoutParams layout=new android.widget.FrameLayout.LayoutParams(targetWidth,targetHeight);layout.gravity=android.view.Gravity.CENTER;view.setLayoutParams(layout);
    }
    void destroy(){destroyed=true;generation++;view.setVisibility(View.INVISIBLE);actor.post(()->{release();thread.quitSafely();});}
    public void surfaceCreated(SurfaceHolder holder){actor.post(()->{surface=holder.getSurface();attachSurface();});}
    public void surfaceChanged(SurfaceHolder holder,int format,int width,int height){surfaceCreated(holder);fitSurface();}
    public void surfaceDestroyed(SurfaceHolder holder){actor.post(()->{surface=null;attachSurface();});}
}
