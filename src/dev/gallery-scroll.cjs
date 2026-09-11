// Exercises the production scroll sentinel callback. No real media or browser paint.
const fs = require('node:fs');
const path = require('node:path');
const {performance} = require('node:perf_hooks');
const {JSDOM} = require('jsdom');
const root = path.resolve(__dirname, '../..');
const script = fs.readFileSync(path.join(root, 'assets/js/playback.js'), 'utf8');
const template = fs.readFileSync(path.join(root, 'assets/html/gallery.html'), 'utf8');
const quantile = (values,p) => [...values].sort((a,b)=>a-b)[Math.max(0,Math.ceil(values.length*p)-1)];
const tick = () => new Promise(resolve=>setImmediate(resolve));
async function trial(count) {
  const entries=Array.from({length:count},(_,i)=>({href:`/watch/${i+1}-1.m4a`,filename:`${i+1}-1.m4a`,state:'Ready',title:'Synthetic audio',subtitle:String(i),kind:'audio',thumbnail:null,transitionName:null}));
  const html=template.replace(/\{(library_heading|collection_nav|gallery_json|initial_cards|library_summary)\}/g,(_,key)=>({library_heading:'Synthetic library',collection_nav:'',gallery_json:JSON.stringify(entries),initial_cards:'',library_summary:count+' media items'})[key]);
  const dom=new JSDOM(html,{url:'http://localhost/',runScripts:'outside-only',pretendToBeVisual:true});
  const w=dom.window;let intersect;
  w.setInterval=()=>0;
  w.IntersectionObserver=class {constructor(fn){intersect=fn} observe(){}};
  w.fetch=async()=>({ok:true,json:async()=>({jobs:[],active:0})});
  try {
    w.eval(script);await tick();
    const initial=Number(JSON.parse(w.sessionStorage.getItem('rustdl:gallery-performance')).rendered);
    if(initial!==Math.min(32,count))throw Error('Incorrect initial card count');
    const batches=[];
    for(let rendered=initial;rendered<count;rendered+=32) {
      const start=performance.now();intersect([{isIntersecting:true}]);const elapsed=performance.now()-start;
      batches.push({cards:Math.min(32,count-rendered),ms:elapsed});
      await tick();
    }
    const finalCards=w.document.querySelectorAll('#gallery-items > .media-card-shell,#gallery-items > .media-card').length;
    if(finalCards!==count)throw Error('Missing cards after scrolling');
    intersect([{isIntersecting:true}]);
    if(w.document.querySelectorAll('#gallery-items > .media-card-shell,#gallery-items > .media-card').length!==count)throw Error('Extra cards at end');
    await tick();return {batches,finalCards};
  } finally {dom.window.close();}
}
(async()=>{
  await trial(100);global.gc?.();
  const results=[];
  for(const count of [100,1000,5000]) {
    const trials=[];
    for(let run=0;run<3;run++) {trials.push(await trial(count));global.gc?.();}
    const full=trials.flatMap(t=>t.batches.filter(b=>b.cards===32).map(b=>b.ms));
    const totals=trials.map(t=>t.batches.reduce((sum,b)=>sum+b.ms,0));
    const early=trials.flatMap(t=>t.batches.filter(b=>b.cards===32).slice(0,5).map(b=>b.ms));
    const late=trials.flatMap(t=>t.batches.filter(b=>b.cards===32).slice(-5).map(b=>b.ms));
    const row={items:count,trials:3,full_batch_samples:full.length,batch_cards:32,batch_median_ms:quantile(full,.5),batch_p95_ms:quantile(full,.95),batch_max_ms:Math.max(...full),batches_over_50ms:full.filter(x=>x>50).length,early_batch_median_ms:quantile(early,.5),late_batch_median_ms:quantile(late,.5),all_append_work_median_ms:quantile(totals,.5),final_dom_cards:trials[0].finalCards};
    results.push(row);console.log(JSON.stringify(row));
  }
  fs.writeFileSync(path.join(root,'target/gallery-performance/scroll.json'),JSON.stringify({environment:'Production sentinel callback in Node/jsdom on this phone. Synthetic audio cards. No real scroll geometry, layout, paint, image decoding, or GPU; not FPS.',results},null,2));
})().catch(error=>{console.error(error);process.exitCode=1});
