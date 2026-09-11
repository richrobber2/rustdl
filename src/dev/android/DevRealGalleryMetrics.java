package app.rustdl;

import android.app.Activity;
import android.os.*;
import android.view.*;
import android.webkit.*;
import org.json.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Dev-only counters in the actual gallery. No screenshots, media reads, or content exports. */
public final class DevRealGalleryMetrics {
    private final Activity activity;
    private final WebView web;
    private final String runId;
    private final boolean previouslyKeptAwake;
    private final JSONArray results=new JSONArray();
    private final Object lock=new Object();
    private final ArrayList<Double> durations=new ArrayList<>();
    private final HandlerThread metrics=new HandlerThread("dev-real-gallery-metrics");
    private final Window.OnFrameMetricsAvailableListener listener;
    private final Handler main=new Handler(Looper.getMainLooper());
    private ServerSocket server;
    private volatile String report;
    private boolean started,measuring,closed;
    private long phaseStart;
    private int dropped,missed;
    private static final String[] FIELDS=("phase elapsedMs rafCallbacksPerSecond rafIntervalMedianMs rafIntervalP95Ms rafIntervalMaxMs actualCssPixelsPerSecond renderedCards thumbnailRequests thumbnailDurationMedianMs thumbnailDurationP95Ms thumbnailDurationMaxMs thumbnailTransferBytes thumbnailHttpErrors thumbnailStatusKnown loadEventsAfterObserver errorEventsAfterObserver imagesTotal imagesLoaded imagesPending imagesBroken longTasks longTaskTotalMs resourceBufferFull thumbnailHitsBefore thumbnailMissesBefore thumbnailPendingBefore thumbnailHitsDelta thumbnailMissesDelta thumbnailQueueDropsDelta thumbnailPending libraryItems rustRenderMicros").split(" ");
    public DevRealGalleryMetrics(Activity activity,WebView web)throws IOException {
        this.activity=activity;this.web=web;
        previouslyKeptAwake=(activity.getWindow().getAttributes().flags&WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)!=0;
        String id=activity.getIntent().getStringExtra("metrics_run_id");runId=id!=null&&id.matches("[a-f0-9]{32}")?id:"manual";
        update("ready",false);
        server=new ServerSocket();server.bind(new InetSocketAddress(InetAddress.getByName("127.0.0.1"),39091));
        metrics.start();
        listener=(window,frame,drop)->{synchronized(lock){
            if(!measuring||frame.getMetric(FrameMetrics.INTENDED_VSYNC_TIMESTAMP)<phaseStart)return;
            if(durations.size()<10000)durations.add(frame.getMetric(FrameMetrics.TOTAL_DURATION)/1e6);
            dropped+=drop;if(Build.VERSION.SDK_INT>=31){long deadline=frame.getMetric(FrameMetrics.DEADLINE);if(deadline>0&&frame.getMetric(FrameMetrics.TOTAL_DURATION)>=deadline)missed++;}
        }};
        activity.getWindow().addOnFrameMetricsAvailableListener(listener,new Handler(metrics.getLooper()));
        activity.getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        web.addJavascriptInterface(new Counters(),"DevRealGallery");
        new Thread(()->{while(!server.isClosed())try(Socket client=server.accept()){
            client.setSoTimeout(2000);InputStream in=client.getInputStream();for(int i=0;i<512;i++){int b=in.read();if(b<0||b=='\n')break;}
            byte[] data=report.getBytes(StandardCharsets.UTF_8);OutputStream out=client.getOutputStream();
            out.write(("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: "+data.length+"\r\nConnection: close\r\n\r\n").getBytes(StandardCharsets.US_ASCII));out.write(data);
        }catch(IOException ignored){}},"dev-real-gallery-report").start();
    }
    public void pageReady(String url){
        android.net.Uri uri=android.net.Uri.parse(url);
        if(started||closed||!"127.0.0.1".equals(uri.getHost())||!"/".equals(uri.getPath()))return;
        try(InputStream in=activity.getAssets().open("dev/real-gallery-metrics.js");ByteArrayOutputStream out=new ByteArrayOutputStream()){
            byte[] bytes=new byte[8192];int n;while((n=in.read(bytes))!=-1)out.write(bytes,0,n);
            final String script=out.toString("UTF-8");
            web.evaluateJavascript("Boolean(document.querySelector('.library'))",value->{if(!closed&&!started&&"true".equals(value)){started=true;web.evaluateJavascript(script,null);}});
        }catch(IOException error){update("setup_failed",true);}
    }
    private void update(String state,boolean aborted){synchronized(lock){try{
        report=new JSONObject().put("state",state).put("runId",runId).put("aborted",aborted).put("phases",results).toString();
    }catch(JSONException ignored){}}}
    public final class Counters {
        @JavascriptInterface public void begin(int phase){synchronized(lock){if(closed)return;durations.clear();missed=dropped=0;phaseStart=System.nanoTime();measuring=true;update("running",false);}}
        @JavascriptInterface public void record(String value){
            if(value==null||value.length()>16000)return;
            synchronized(lock){if(closed)return;measuring=false;try{
                JSONObject input=new JSONObject(value),clean=new JSONObject();
                for(String key:FIELDS){double number=input.optDouble(key,Double.NaN);if(!Double.isNaN(number)&&!Double.isInfinite(number))clean.put(key,number);}
                clean.put("aborted",input.optBoolean("aborted",true));Collections.sort(durations);
                clean.put("windowFrameSamples",durations.size()).put("windowFrameP95Ms",durations.isEmpty()?JSONObject.NULL:durations.get(Math.max(0,(int)Math.ceil(.95*durations.size())-1)))
                    .put("windowMissedDeadlines",Build.VERSION.SDK_INT>=31?missed:JSONObject.NULL).put("droppedMetricReports",dropped);
                results.put(clean);update("running",false);
            }catch(JSONException error){update("invalid_metrics",true);}}
        }
        @JavascriptInterface public void finish(boolean aborted){synchronized(lock){measuring=false;update("complete",aborted||results.length()!=3);main.post(()->{if(!previouslyKeptAwake)activity.getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);});}}
    }
    public void destroy(){
        synchronized(lock){closed=true;measuring=false;}
        main.removeCallbacksAndMessages(null);if(!previouslyKeptAwake)activity.getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);web.evaluateJavascript("window.stopRealGalleryMetrics?.()",null);web.removeJavascriptInterface("DevRealGallery");
        activity.getWindow().removeOnFrameMetricsAvailableListener(listener);metrics.quitSafely();try{server.close();}catch(IOException ignored){}
    }
}
