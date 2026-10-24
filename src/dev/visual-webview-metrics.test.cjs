const {test,after}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {JSDOM}=require('jsdom');
const root=path.resolve(__dirname,'../..');
const fixtureDir=fs.mkdtempSync(path.join(require('node:os').tmpdir(),'rustdl-webview-test-'));
after(()=>fs.rmSync(fixtureDir,{recursive:true,force:true}));
const generated=require('node:child_process').spawnSync('python3',[path.join(__dirname,'prepare-visual.py'),fixtureDir],{encoding:'utf8'});
assert.equal(generated.status,0,generated.stderr);
const fixture=path.join(fixtureDir,'webview-metrics-100.html');
async function run({abort=false,broken=false,observer=false,count=100}={}){
  const dom=new JSDOM(fs.readFileSync(fixture.replace('-100.html',`-${count}.html`),'utf8'),{runScripts:'outside-only',pretendToBeVisual:true,url:'https://rustdl-benchmark.invalid/'});
  const w=dom.window;w.scrollTo=()=>{};w.scrollBy=()=>{};
  const observers=[];
  if(observer)w.PerformanceObserver=class{
    static supportedEntryTypes=['longtask','layout-shift'];
    constructor(cb){this.cb=cb;this.entries=[];this.disconnected=false;observers.push(this);}
    observe({type}){this.entries=type==='longtask'?[{entryType:type,duration:55}]:[{entryType:type,value:.2,hadRecentInput:false},{entryType:type,value:.1,hadRecentInput:true}];}
    takeRecords(){const rows=this.entries;this.entries=[];return rows;}
    disconnect(){this.disconnected=true;}
  };
  w.eval(fs.readFileSync(path.join(__dirname,'visual-webview-metrics.js'),'utf8'));
  w.eval(fs.readFileSync(path.join(root,'assets/js/playlist.js'),'utf8'));
  let result;w.RustDLDev={beginPhase(){},caseComplete(json){result=JSON.parse(json);}};
  if(abort)w.RustDLRenderingMetrics.abort();
  if(broken)w.document.querySelectorAll('[data-page-next]').forEach(b=>b.remove());
  await w.RustDLRenderingMetrics.run();
  const cleaned=observers.every(o=>o.disconnected);dom.window.close();return {result,cleaned};
}
test('real playlist interactions preserve the row pool and unavailable metrics',async()=>{
  const {result:r}=await run();assert.equal(r.pass,true);assert.equal(r.interactions.length,6);
  assert.equal(r.totals.longTasks,null);assert.equal(r.totals.layoutShiftScore,null);
  assert.equal(r.interactions[0].observed.layoutShiftScore,null);
});
test('background interruption fails rather than reporting success',async()=>{
  const {result:r}=await run({abort:true});assert.equal(r.pass,false);assert.equal(r.aborted,true);
});
test('broken controls fail correctness validation',async()=>{
  const {result:r}=await run({broken:true});assert.equal(r.pass,false);assert.ok(r.error);
});
test('observer records are drained once and observers disconnected',async()=>{
  const {result:r,cleaned}=await run({observer:true});assert.equal(r.pass,true);assert.equal(cleaned,true);
  assert.equal(r.totals.longTasks,1);assert.equal(r.totals.longTaskMs,55);
  assert.ok(Math.abs(r.totals.layoutShiftScore-.3)<1e-9);
  assert.equal(r.totals.layoutShiftWithoutRecentInput,.2);
});
test('metrics mode blocks both native capture entry points',()=>{
 const java=fs.readFileSync(path.join(__dirname,'android/GalleryBenchmarkActivity.java'),'utf8');
 assert.match(java,/private void capture\(\)\{\s*if\(metricsOnly\)return;/);
 assert.match(java,/private void captureFailure[^\n]+\{\s*if\(metricsOnly\)\{next.run\(\);return;\}/);
});

for(const count of [1000,5000])test(`synthetic ${count}-entry fixture keeps 32 reusable nodes`,async()=>{
 const {result:r}=await run({count});assert.equal(r.pass,true);assert.equal(r.entryCount,count);
});
