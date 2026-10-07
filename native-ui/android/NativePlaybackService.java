package app.rustdl;

import android.app.Service;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Intent;
import android.os.Binder;
import android.os.IBinder;

/** Foreground decoder owner. Export is disabled; notifications contain no media identity. */
public final class NativePlaybackService extends Service {
    static native String nativePlaybackArtwork(String filename,boolean privacy);
    static native String nativePlaybackRoute(String filename);
    static native String nativePlaybackNext(String filename);
    static native long nativePlaybackDownloaded(String filename);
    static native String nativePlaybackDetails(String filename,boolean privacy);
    private static final String CHANNEL="rustdl_native_playback";
    private static final int NOTIFICATION=2403;
    private NativePlaybackSession player;
    private String origin="";
    private boolean foreground;
    private NativePlaybackMediaSession media;
    private java.lang.ref.WeakReference<MainActivity> ui=new java.lang.ref.WeakReference<>(null);
    private final android.os.Handler handler=new android.os.Handler(android.os.Looper.getMainLooper());
    private final Runnable refresh=new Runnable() {public void run() {if(player!=null) {player.command("snapshot",0,NativePlaybackService.this::snapshot);handler.postDelayed(this,1000);}}};
    void snapshot(String data) {
        if(player==null) return;
        try {org.json.JSONObject value=new org.json.JSONObject(data);media.update(value,foreground&&!"idle".equals(value.optString("state")),player.inspectionPrivacyEnabled());}catch(Exception invalid) {}
        MainActivity current=ui.get();if(current!=null&&!current.isDestroyed()) current.nativePlaybackSnapshot(data);
        player.recoverGrowing(this::snapshot);
    }
    private void command(String action,double value) {
        if(player==null) return;
        if("stop".equals(action)) {foreground=false;handler.removeCallbacks(refresh);player.command("stop",0,this::snapshot);media.deactivate();stopForeground(true);stopSelf();return;}
        player.setActive(true);
        if("next".equals(action)) {player.next(this::snapshot);return;}
        player.command(action,value,this::snapshot);
    }
    private final LocalBinder binder=new LocalBinder();
    public final class LocalBinder extends Binder {
        void command(String action,double value) {NativePlaybackService.this.command(action,value);}
        void privacy(boolean enabled) {if(player!=null) player.setInspectionPrivacy(enabled);if(enabled&&media!=null) media.redact();}
        boolean hasPlayer() {return player!=null;}
        void attach(MainActivity activity) {ui=new java.lang.ref.WeakReference<>(activity);if(player!=null) {player.attachUi(activity);player.currentSelection(activity::nativePlaybackSelection);}}
        void detach(MainActivity activity) {if(ui.get()==activity) {ui.clear();if(player!=null) {player.hideForBackground();player.attachUi(null);}}}
        java.util.function.Consumer<String> callback() {return NativePlaybackService.this::snapshot;}
        NativePlaybackSession player(String requestedOrigin) {
            if(!validOrigin(requestedOrigin)) throw new IllegalArgumentException("Invalid playback origin");
            if(player!=null&&!origin.equals(requestedOrigin)) {player.destroy();player=null;}
            if(player==null) {origin=requestedOrigin;player=new NativePlaybackSession(NativePlaybackService.this,new PlaybackBridge(NativePlaybackService.this),origin);player.setSelectionListener(name->{media.selection(name);MainActivity current=ui.get();if(current!=null&&!current.isDestroyed()) current.nativePlaybackSelection(name);});}
            handler.removeCallbacks(refresh);handler.post(refresh);
            return player;
        }
    }
    private static boolean validOrigin(String origin) {
        if(origin==null) return false;
        try {java.net.URI uri=new java.net.URI(origin);return "http".equals(uri.getScheme())&&"127.0.0.1".equals(uri.getHost())&&uri.getPort()>0&&uri.getPort()<=65535&&"/".equals(uri.getPath())&&uri.getRawUserInfo()==null&&uri.getRawQuery()==null&&uri.getRawFragment()==null;}
        catch(Exception invalid) {return false;}
    }
    @Override public void onCreate() {
        super.onCreate();media=new NativePlaybackMediaSession(this,(action,value)->{
            if(player!=null&&"stop".equals(action)) {player.command("pause",0,this::snapshot);player.command("seek",0,this::snapshot);}
            else command(action,value);
        });
        media.artworkResolver(name->nativePlaybackArtwork(name,false));
        NotificationManager manager=(NotificationManager)getSystemService(NOTIFICATION_SERVICE);
        manager.createNotificationChannel(new NotificationChannel(CHANNEL,"Playback",NotificationManager.IMPORTANCE_LOW));
    }
    @Override public int onStartCommand(Intent intent,int flags,int startId) {
        String action=intent==null?"":intent.getStringExtra("control");
        if("stop".equals(action)) {if(player!=null) command("stop",0);else {stopForeground(true);stopSelf();}return START_NOT_STICKY;}
        if(!"begin".equals(action)&&!"play".equals(action)&&!"pause".equals(action)) return START_NOT_STICKY;
        startForeground(NOTIFICATION,notification());foreground=true;
        if(player!=null) {handler.removeCallbacks(refresh);handler.post(refresh);}
        if(player!=null&&("play".equals(action)||"pause".equals(action))) {
            command(action,0);
        }
        return START_NOT_STICKY;
    }
    private Notification notification() {
        Notification.Builder builder=new Notification.Builder(this,CHANNEL)
                .setSmallIcon(android.R.drawable.ic_media_play).setContentTitle("RustDL playback")
                .setContentText("Native media playback").setOngoing(true).setVisibility(Notification.VISIBILITY_SECRET);
        Intent open=new Intent(this,MainActivity.class).setAction(MainActivity.OPEN_NATIVE_PLAYER_ACTION).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP|Intent.FLAG_ACTIVITY_CLEAR_TOP);
        builder.setContentIntent(PendingIntent.getActivity(this,906,open,PendingIntent.FLAG_UPDATE_CURRENT|PendingIntent.FLAG_IMMUTABLE));
        for(String action:new String[]{"play","pause","stop"}) {
            Intent intent=new Intent(this,NativePlaybackService.class).setAction(getPackageName()+".NATIVE_PLAYBACK_"+action).putExtra("control",action);
            PendingIntent command=PendingIntent.getService(this,910+action.length(),intent,PendingIntent.FLAG_UPDATE_CURRENT|PendingIntent.FLAG_IMMUTABLE);
            builder.addAction("play".equals(action)?android.R.drawable.ic_media_play:"pause".equals(action)?android.R.drawable.ic_media_pause:android.R.drawable.ic_menu_close_clear_cancel,action,command);
        }
        return builder.build();
    }
    @Override public IBinder onBind(Intent intent) {return binder;}
    @Override public void onDestroy() {handler.removeCallbacks(refresh);if(media!=null) media.destroy();ui.clear();if(player!=null) {player.destroy();player=null;}stopForeground(true);super.onDestroy();}
}
