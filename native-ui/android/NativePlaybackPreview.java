package app.rustdl;

/** Runtime-only scrub frames. No frame data or media identity leaves Android. */
final class NativePlaybackPreview {
    private final android.os.Handler main=new android.os.Handler(android.os.Looper.getMainLooper());
    private final java.util.concurrent.ThreadPoolExecutor worker=new java.util.concurrent.ThreadPoolExecutor(1,1,0L,java.util.concurrent.TimeUnit.MILLISECONDS,new java.util.concurrent.ArrayBlockingQueue<Runnable>(1),new java.util.concurrent.ThreadPoolExecutor.DiscardOldestPolicy());
    private final java.util.concurrent.atomic.AtomicLong revision=new java.util.concurrent.atomic.AtomicLong();
    private android.view.View panel;
    private android.view.WindowManager manager;
    private android.graphics.Bitmap shown;
    private Runnable pending;
    private volatile boolean destroyed;
    void hide() {
        revision.incrementAndGet();
        Runnable old=pending;pending=null;if(old!=null) main.removeCallbacks(old);
        main.post(()->{if(panel!=null&&manager!=null) try {manager.removeViewImmediate(panel);}catch(RuntimeException gone) {}panel=null;manager=null;if(shown!=null) shown.recycle();shown=null;});
    }
    void show(android.app.Activity activity,String filename,String source,String origin,double seconds,java.util.function.BooleanSupplier allowed) {
        hide();final long request=revision.get();
        if(destroyed||!Double.isFinite(seconds)||seconds<0d||!NativePlaybackPolicy.validSource(filename,source,origin)||!allowed.getAsBoolean()) return;
        java.lang.ref.WeakReference<android.app.Activity> ui=new java.lang.ref.WeakReference<>(activity);
        pending=()->{
            if(destroyed||request!=revision.get()||!allowed.getAsBoolean()) return;
            try {worker.execute(()->{
                android.media.MediaMetadataRetriever retriever=new android.media.MediaMetadataRetriever();android.graphics.Bitmap frame=null;
                try {retriever.setDataSource(source,new java.util.HashMap<String,String>());frame=retriever.getScaledFrameAtTime((long)(seconds*1000000d),android.media.MediaMetadataRetriever.OPTION_CLOSEST_SYNC,240,135);}catch(RuntimeException unavailable) {}finally {try {retriever.release();}catch(Exception unavailable) {}}
                final android.graphics.Bitmap result=frame;
                main.post(()->{
                    android.app.Activity current=ui.get();
                    if(result==null) return;
                    if(destroyed||request!=revision.get()||current==null||current.isDestroyed()||!allowed.getAsBoolean()) {result.recycle();return;}
                    android.widget.LinearLayout content=new android.widget.LinearLayout(current);content.setOrientation(android.widget.LinearLayout.VERTICAL);content.setBackgroundColor(android.graphics.Color.BLACK);
                    android.widget.ImageView image=new android.widget.ImageView(current);image.setImageBitmap(result);image.setContentDescription("Scrub preview");content.addView(image,new android.widget.LinearLayout.LayoutParams(240,135));
                    android.widget.TextView time=new android.widget.TextView(current);time.setTextColor(android.graphics.Color.WHITE);long whole=(long)seconds;time.setText(String.format(java.util.Locale.ROOT,"%d:%02d",whole/60,whole%60));content.addView(time);

                    android.view.WindowManager.LayoutParams layout=new android.view.WindowManager.LayoutParams(240,android.view.WindowManager.LayoutParams.WRAP_CONTENT,android.view.WindowManager.LayoutParams.TYPE_APPLICATION_PANEL,android.view.WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE|android.view.WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE|android.view.WindowManager.LayoutParams.FLAG_SECURE,android.graphics.PixelFormat.TRANSLUCENT);
                    layout.token=current.getWindow().getDecorView().getWindowToken();layout.gravity=android.view.Gravity.TOP|android.view.Gravity.CENTER_HORIZONTAL;layout.y=100;
                    manager=current.getWindowManager();
                    try {manager.addView(content,layout);panel=content;shown=result;}catch(RuntimeException unavailable) {manager=null;result.recycle();}
                });
            });}catch(java.util.concurrent.RejectedExecutionException closed) {}
        };
        main.postDelayed(pending,150L);
    }
    void destroy() {destroyed=true;hide();worker.shutdownNow();}
}
