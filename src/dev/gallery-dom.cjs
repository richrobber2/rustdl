// Synthetic DOM work only: jsdom does not measure layout, paint, or image decoding.
const fs = require('node:fs');
const path = require('node:path');
const {performance} = require('node:perf_hooks');
const {JSDOM} = require('jsdom');
const root = path.resolve(__dirname, '../..');
const script = fs.readFileSync(path.join(root, 'assets/js/playback.js'), 'utf8');
const template = fs.readFileSync(path.join(root, 'assets/html/gallery.html'), 'utf8');
const median = values => [...values].sort((a,b)=>a-b)[Math.floor(values.length/2)];
(async () => {
const results=[];
for(const count of [0,38,100,1000,5000]) {
  const entries=Array.from({length:count},(_,i)=>({href:`/watch/${i+1}-1.m4a`,filename:`${i+1}-1.m4a`,state:'Ready',title:'Synthetic audio',subtitle:String(i),kind:'audio',thumbnail:null,transitionName:null}));
  const html=template.replace(/\{(library_heading|collection_nav|gallery_json|initial_cards|library_summary)\}/g,(_,key)=>({library_heading:'Synthetic library',collection_nav:'',gallery_json:JSON.stringify(entries),initial_cards:'',library_summary:count+' media items'})[key]);
  const parse=[],init=[],batch=[];let cards=0;
  for(let run=0;run<8;run++) {
    const start=performance.now();
    const dom=new JSDOM(html,{url:'http://localhost/',runScripts:'outside-only',pretendToBeVisual:true});
    const w=dom.window;let observer;
    w.setInterval=()=>0;
    w.IntersectionObserver=class {constructor(fn){observer=fn} observe(){}};
    w.fetch=async()=>({ok:true,json:async()=>({jobs:[],active:0})});
    const parsed=performance.now();w.eval(script);const initialized=performance.now();
    cards=w.document.querySelectorAll('#gallery-items > .media-card-shell,#gallery-items > .media-card').length;
    if(cards!==Math.min(count,32))throw Error('Unexpected initial card count: '+cards);
    const beforeBatch=performance.now();if(observer)observer([{isIntersecting:true}]);const afterBatch=performance.now();
    if(run>0){parse.push(parsed-start);init.push(initialized-parsed);batch.push(afterBatch-beforeBatch);}
    await new Promise(resolve => setImmediate(resolve));
    dom.window.close();
  }
  const row={items:count,initial_cards:cards,samples:7,html_parse_median_ms:median(parse),script_init_median_ms:median(init),next_batch_median_ms:median(batch)};
  results.push(row);console.log(JSON.stringify(row));
}
fs.writeFileSync(path.join(root,'target/gallery-performance/dom.json'),JSON.stringify({environment:'Node + jsdom; synthetic entries; no layout/paint/thumbnails; one warmup per size',results},null,2));

})().catch(error => { console.error(error); process.exitCode=1; });
