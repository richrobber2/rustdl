"""Instrument staged MainActivity only, never the production source tree."""
from pathlib import Path
import sys
p=Path(sys.argv[1]);text=p.read_text()
def replace(old,new):
 global text
 if text.count(old)!=1:raise SystemExit('Dev instrumentation marker mismatch: '+old)
 text=text.replace(old,new,1)
replace('public class MainActivity extends Activity {','public class MainActivity extends Activity {\n    private DevRealGalleryMetrics devRealMetrics;\n    private void configureDevMetrics(){\n        if(devRealMetrics!=null){devRealMetrics.destroy();devRealMetrics=null;}\n        if(!inspectionMode&&getIntent().getBooleanExtra("dev_metrics",false)){\n            try{devRealMetrics=new DevRealGalleryMetrics(this,webView);}catch(java.io.IOException error){throw new IllegalStateException("Could not start dev metrics",error);}\n        }\n    }')
replace('        configureWebView();','        configureWebView();\n        configureDevMetrics();')
replace('                if (!inspectionMode) {\n                    dispatchRustEvent','                if(devRealMetrics!=null)devRealMetrics.pageReady(url);\n                if (!inspectionMode) {\n                    dispatchRustEvent')
replace('        setIntent(intent);\n        loadInitialScreen(intent);','        setIntent(intent);\n        configureDevMetrics();\n        loadInitialScreen(intent);')
replace('    protected void onDestroy() {','    protected void onDestroy() {\n        if(devRealMetrics!=null)devRealMetrics.destroy();')
p.write_text(text)
