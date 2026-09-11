package app.rustdl;

import android.app.Activity;
import android.content.pm.ActivityInfo;
import android.graphics.Bitmap;
import android.os.*;
import android.view.*;
import android.webkit.*;
import android.widget.*;
import org.json.*;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Development APK only. Renders bundled synthetic content, never the user's gallery. */
public final class GalleryBenchmarkActivity extends Activity {
    private WebView web;
    private JSONArray suiteCases=new JSONArray(), visualCases=new JSONArray();
    private JSONObject suiteCoverage=new JSONObject();
    private int caseIndex;
    private boolean suiteMode;
    private String runId="manual";
    private TextView status;
    private ServerSocket server;
    private volatile String report="{\"state\":\"ready\"}";
    private volatile byte[] screenshot;
    private final Map<String,byte[]> failureScreenshots=Collections.synchronizedMap(new LinkedHashMap<String,byte[]>());
    private volatile boolean measuring;
    private final Object frameLock=new Object();
    private final ArrayList<Double> frameTimes=new ArrayList<>();
    private int missedDeadlines, droppedReports;
    private HandlerThread metricsThread;
    private Window.OnFrameMetricsAvailableListener listener;
    private final JSONArray phases=new JSONArray();
    private final Handler main=new Handler(Looper.getMainLooper());

    @Override public void onCreate(Bundle state) {
        WebView.setDataDirectorySuffix("dev-benchmark");
        super.onCreate(state);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        LinearLayout root=new LinearLayout(this);root.setOrientation(LinearLayout.VERTICAL);
        status=new TextView(this);status.setTextColor(0xffeeeeee);status.setTextSize(14);status.setPadding(16,8,16,8);
        status.setText("dev:: visual gallery test · synthetic content only");
        root.addView(status,new LinearLayout.LayoutParams(-1,Math.round(64*getResources().getDisplayMetrics().density)));
        Button run=new Button(this);run.setText("Run visual scroll test");
        run.setOnClickListener(v->{getIntent().putExtra("auto",true);startRun();});root.addView(run);
        web=new WebView(this);root.addView(web,new LinearLayout.LayoutParams(-1,0,1));setContentView(root);WindowLayout.fitContent(this,root);
        WebSettings settings=web.getSettings();settings.setJavaScriptEnabled(true);settings.setDomStorageEnabled(true);
        settings.setUseWideViewPort(true);settings.setAllowFileAccess(false);settings.setAllowContentAccess(false);
        settings.setSupportZoom(false);settings.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        web.addJavascriptInterface(new ResultsBridge(),"RustDLDev");
        web.setWebViewClient(new WebViewClient(){
            @Override public boolean shouldOverrideUrlLoading(WebView view,WebResourceRequest request){return true;}
            @Override public WebResourceResponse shouldInterceptRequest(WebView view,WebResourceRequest request){
                String path=request.getUrl().getPath();
                if("/dev/rainy-city.webp".equals(path)||"/dev/rainy-city-light.webp".equals(path)){
                    try{return new WebResourceResponse("image/webp",null,getAssets().open("dev/"+path.substring(5)));}catch(IOException ignored){return response("text/plain","");}
                }
                if(path!=null&&path.startsWith("/thumbnail/")) {
                    int hue=Math.floorMod(path.hashCode(),360);
                    String svg="<svg xmlns='http://www.w3.org/2000/svg' width='480' height='270'><rect width='480' height='270' fill='hsl("+hue+",45%,28%)'/><circle cx='240' cy='135' r='70' fill='#ffffff22'/><text x='240' y='145' text-anchor='middle' fill='white' font-size='24'>SYNTHETIC</text></svg>";
                    return response("image/svg+xml",svg);
                }
                return response("application/json","{\"jobs\":[],\"active\":0}");
            }
            @Override public void onPageFinished(WebView view,String url){
                if(suiteMode&&caseIndex<suiteCases.length()){
                    try{web.evaluateJavascript(asset("visual-audit.js")+"\n"+asset(suiteCases.getJSONObject(caseIndex).optString("script","visual-smoke.js")),null);}catch(Exception error){status.setText("Missing visual smoke script");}
                    return;
                }
                status.setText("Ready · 1,000 synthetic cards · 36-second visual scroll test");
                if(getIntent().getBooleanExtra("auto",false))main.postDelayed(()->web.evaluateJavascript("runVisualBenchmark()",null),1500);
            }
        });
        metricsThread=new HandlerThread("dev-frame-metrics");metricsThread.start();
        listener=(window,frame,dropped)->{
            synchronized(frameLock){if(!measuring)return;
                if(frameTimes.size()<20000)frameTimes.add(frame.getMetric(FrameMetrics.TOTAL_DURATION)/1_000_000.0);
                droppedReports+=dropped;
                if(Build.VERSION.SDK_INT>=31){long deadline=frame.getMetric(FrameMetrics.DEADLINE);if(deadline>0&&frame.getMetric(FrameMetrics.TOTAL_DURATION)>=deadline)missedDeadlines++;}
            }
        };
        getWindow().addOnFrameMetricsAvailableListener(listener,new Handler(metricsThread.getLooper()));
        try {startReportServer();startRun();}
        catch(Exception error){status.setText("Benchmark setup failed: "+error.getClass().getSimpleName());report="{\"state\":\"setup_failed\"}";}
    }


    @Override protected void onNewIntent(android.content.Intent intent){super.onNewIntent(intent);setIntent(intent);startRun();}
    private void startRun(){
        try {
            runId=getIntent().getStringExtra("run_id");if(runId==null)runId="manual";
            suiteMode=getIntent().getBooleanExtra("suite",false);caseIndex=0;visualCases=new JSONArray();
            synchronized(phases){while(phases.length()>0)phases.remove(0);}
            suiteCoverage=new JSONObject(asset("suite.json"));suiteCases=suiteCoverage.getJSONArray("cases");
            if(getIntent().getBooleanExtra("batch500",false)){
                suiteMode=true;JSONArray selected=new JSONArray();
                for(int i=0;i<suiteCases.length();i++)if(suiteCases.getJSONObject(i).getString("name").contains("-500/"))selected.put(suiteCases.getJSONObject(i));
                suiteCases=selected;
            }
            screenshot=null;failureScreenshots.clear();publishProgress();loadCase();
        }catch(Exception error){status.setText("Visual suite setup failed: "+error.getClass().getSimpleName());}
    }
    private void loadCase()throws Exception {
        if(getIntent().getBooleanExtra("batch500",false)&&caseIndex>=suiteCases.length()){
            new ResultsBridge().complete("{\"aborted\":false,\"method\":\"500-entry synthetic WebView layout, interaction, and scroll test\"}");return;
        }
        boolean screen=suiteMode&&caseIndex<suiteCases.length();
        JSONObject item=screen?suiteCases.getJSONObject(caseIndex):new JSONObject();
        String profile=screen?item.getString("profile"):"native";
        boolean landscape=profile.contains("landscape");
        setRequestedOrientation(landscape?ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE:ActivityInfo.SCREEN_ORIENTATION_PORTRAIT);
        int widthDp=0;
        java.util.regex.Matcher widthMatch=java.util.regex.Pattern.compile("w(\\d+)").matcher(profile);
        if(widthMatch.find())try{widthDp=Integer.parseInt(widthMatch.group(1));}catch(NumberFormatException ignored){}
        boolean fixed=widthDp>0;
        boolean largeText=profile.contains("text150");
        LinearLayout.LayoutParams layout=(LinearLayout.LayoutParams)web.getLayoutParams();
        layout.width=fixed?Math.min(getResources().getDisplayMetrics().widthPixels,Math.round(widthDp*getResources().getDisplayMetrics().density)):-1;
        web.setLayoutParams(layout);
        web.getSettings().setTextZoom(largeText?150:100);
        status.setText(screen?"dev:: "+(caseIndex+1)+"/"+suiteCases.length()+" · "+item.getString("name"):"dev:: gallery scrolling");
        String html=screen?asset(item.getString("asset")):page();
        if(screen)html=html.replaceFirst("<html", "<html data-theme=\""+(profile.endsWith("light")||profile.equals("w393-text150")?"light":"dark")+"\" data-background=\""+(profile.contains("city")?"rainy-city":"space")+"\"");
        web.loadDataWithBaseURL("https://rustdl-benchmark.invalid/",html,"text/html","UTF-8",null);
    }
    private void publishProgress(){
        try{report=new JSONObject().put("state","suite_running").put("runId",runId).put("visualCases",visualCases).toString();}catch(JSONException ignored){}
    }
    private void addFrameMetrics(JSONObject result)throws JSONException {
        synchronized(frameLock){measuring=false;Collections.sort(frameTimes);
            result.put("windowFrameSamples",frameTimes.size()).put("windowFrameP95Ms",frameTimes.isEmpty()?JSONObject.NULL:frameTimes.get(Math.max(0,(int)Math.ceil(frameTimes.size()*.95)-1)))
                .put("windowMissedDeadlines",Build.VERSION.SDK_INT>=31?missedDeadlines:JSONObject.NULL).put("droppedMetricReports",droppedReports);
        }
    }

    private static WebResourceResponse response(String mime,String text){return new WebResourceResponse(mime,"UTF-8",new ByteArrayInputStream(text.getBytes(StandardCharsets.UTF_8)));}
    private String asset(String name)throws IOException {try(InputStream in=getAssets().open("dev/"+name);ByteArrayOutputStream out=new ByteArrayOutputStream()){byte[] b=new byte[8192];int n;while((n=in.read(b))!=-1)out.write(b,0,n);return out.toString("UTF-8");}}
    private String page()throws Exception {
        JSONArray entries=new JSONArray();
        for(int i=1;i<=1000;i++){String filename=i+"-1.mp4";entries.put(new JSONObject().put("href","/watch/"+filename).put("filename",filename).put("state","Ready").put("title","Synthetic video "+i).put("subtitle","Development fixture").put("kind","video").put("thumbnail",filename).put("transitionName","fixture-"+i));}
        String gallery=asset("gallery.html").replace("{collection_nav}","").replace("{library_heading}","Synthetic gallery").replace("{library_summary}","1,000 synthetic videos").replace("{gallery_json}",entries.toString()).replace("{initial_cards}","");
        return asset("index.html").replace("<!--INDEX_STYLESHEET-->","<style>"+asset("index.css")+asset("appearance.css")+"</style>").replace("<!--SAVED_VIDEOS-->",gallery).replace("<!--PLAYBACK_SCRIPT-->","<script>"+asset("playback.js")+"</script><script>"+asset("visual-scroll.js")+"</script>");
    }

    private final class ResultsBridge {
        @JavascriptInterface public void beginPhase(String name){
            if("1000 CSS px/s".equals(name)){synchronized(phases){while(phases.length()>0)phases.remove(0);}screenshot=null;}
            synchronized(frameLock){frameTimes.clear();missedDeadlines=0;droppedReports=0;measuring=true;}
            publishProgress();
            main.post(()->status.setText("Scrolling at "+name+" · keep this screen visible"));
        }
        @JavascriptInterface public void endPhase(String json){
            if(json==null||json.length()>100000)return;
            try {JSONObject result=new JSONObject(json);
                addFrameMetrics(result);
                synchronized(phases){phases.put(result);}
            }catch(JSONException ignored){}
        }
        @JavascriptInterface public void caseComplete(String json){
            if(json==null||json.length()>100000)return;
            main.post(()->{
                try{JSONObject result=new JSONObject(json);JSONObject item=suiteCases.getJSONObject(caseIndex);
                    result.put("name",item.getString("name")).put("coverage",item.getString("coverage"));addFrameMetrics(result);
                    Runnable next=()->{visualCases.put(result);caseIndex++;publishProgress();main.postDelayed(()->{try{loadCase();}catch(Exception error){status.setText("Could not load next visual case");}},100);};
                    if(!result.optBoolean("pass")&&failureScreenshots.size()<12)captureFailure(result,next);else next.run();
                }catch(Exception error){status.setText("Invalid visual case result");}
            });
        }
        @JavascriptInterface public void complete(String json){
            if(json==null||json.length()>60000)return;
            try {JSONObject result=new JSONObject(json);synchronized(phases){result.put("results",new JSONArray(phases.toString()));while(phases.length()>0)phases.remove(0);}
                result.put("state","complete").put("runId",runId).put("visualCases",visualCases).put("missingCoverage",suiteCoverage.optJSONArray("missing")).put("androidApi",Build.VERSION.SDK_INT).put("deviceModel",Build.MODEL)
                    .put("webViewVersion",WebView.getCurrentWebViewPackage()==null?"unknown":WebView.getCurrentWebViewPackage().versionName);
                report=result.toString(2);
                main.post(()->{status.setText("Complete · results at 127.0.0.1:39090/report.json");capture();});
            }catch(JSONException ignored){}
        }
    }

    private void captureFailure(JSONObject reportCase,Runnable next){
        try {
            Bitmap bitmap=Bitmap.createBitmap(getWindow().getDecorView().getWidth(),getWindow().getDecorView().getHeight(),Bitmap.Config.ARGB_8888);
            final String key="/failure-"+caseIndex+".png";
            PixelCopy.request(getWindow(),bitmap,result->{
                try {
                    if(result==PixelCopy.SUCCESS){ByteArrayOutputStream out=new ByteArrayOutputStream();bitmap.compress(Bitmap.CompressFormat.PNG,100,out);failureScreenshots.put(key,out.toByteArray());reportCase.put("screenshot",key);}
                    else reportCase.put("screenshotError",result);
                }catch(Exception ignored){}finally{bitmap.recycle();next.run();}
            },main);
        }catch(Exception error){next.run();}
    }
    private void capture(){
        main.postDelayed(()->{
            Bitmap bitmap=Bitmap.createBitmap(getWindow().getDecorView().getWidth(),getWindow().getDecorView().getHeight(),Bitmap.Config.ARGB_8888);
            PixelCopy.request(getWindow(),bitmap,result->{
                if(result==PixelCopy.SUCCESS){ByteArrayOutputStream out=new ByteArrayOutputStream();bitmap.compress(Bitmap.CompressFormat.PNG,100,out);screenshot=out.toByteArray();}bitmap.recycle();
            },main);
        },500);
    }
    private void startReportServer()throws IOException {
        server=new ServerSocket();server.bind(new InetSocketAddress(InetAddress.getByName("127.0.0.1"),39090));
        new Thread(()->{while(!server.isClosed()){
            try(Socket client=server.accept()){
                client.setSoTimeout(2000);String line=new BufferedReader(new InputStreamReader(client.getInputStream(),StandardCharsets.UTF_8)).readLine();
                String requestPath=line!=null&&line.startsWith("GET ")?line.split(" ")[1]:"";
                boolean shot=requestPath.equals("/screenshot.png")||requestPath.matches("/failure-[0-9]+\\.png");
                byte[] data=shot?(requestPath.equals("/screenshot.png")?screenshot:failureScreenshots.get(requestPath)):report.getBytes(StandardCharsets.UTF_8);if(data==null)data=new byte[0];
                OutputStream out=client.getOutputStream();out.write(("HTTP/1.1 200 OK\r\nContent-Type: "+(shot?"image/png":"application/json")+"\r\nCache-Control: no-store\r\nContent-Length: "+data.length+"\r\nConnection: close\r\n\r\n").getBytes(StandardCharsets.US_ASCII));out.write(data);
            }catch(IOException ignored){}
        }},"dev-benchmark-results").start();
    }
    @Override public void onPause(){super.onPause();if(web!=null)web.evaluateJavascript("window.stopVisualBenchmark?.()",null);}
    @Override public void onDestroy(){
        if(listener!=null)getWindow().removeOnFrameMetricsAvailableListener(listener);
        if(metricsThread!=null)metricsThread.quitSafely();
        if(server!=null)try{server.close();}catch(IOException ignored){}
        main.removeCallbacksAndMessages(null);if(web!=null)web.destroy();super.onDestroy();
    }
}
