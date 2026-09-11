const {test}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const vm=require('node:vm');
const {JSDOM}=require('jsdom');
const root=path.join(__dirname,'../..');
const source=fs.readFileSync(path.join(root,'assets/js/playlist-resolution.js'),'utf8');
const tick=()=>new Promise(resolve=>setImmediate(resolve));
function fixture(responses){
 const html=fs.readFileSync(path.join(root,'assets/html/playlist-resolution.html'),'utf8').replace(/<style>[\s\S]*?<\/style>/,'').replaceAll('{token}','test-token').replace('{script}','');
 const dom=new JSDOM(html,{url:'http://localhost/playlist/resolution?job=test-token',pretendToBeVisual:true});
 const timers=[],redirects=[],requests=[];
 vm.runInNewContext(source,{document:dom.window.document,location:{replace:url=>redirects.push(url)},setTimeout:fn=>{timers.push(fn);return timers.length;},clearTimeout:()=>{},fetch:async url=>{requests.push(url);const value=responses.shift();return {ok:value.status===200,status:value.status,json:async()=>value.body};}});
 return {dom,doc:dom.window.document,timers,redirects,requests};
}
test('progress reports partial results, safely shows failures, and preserves explicit continuation',async t=>{
 const state={total:474,completed:3,ready:3,failures:[],finished:false,cancelled:false,elapsedSeconds:1,canContinue:false};
 const f=fixture([{status:200,body:state},{status:200,body:{...state,completed:474,ready:472,failures:['<script>not markup</script>','Unavailable video'],finished:true,elapsedSeconds:200,canContinue:true}}]);
 t.after(()=>f.dom.window.close());await tick();
 assert.match(f.doc.getElementById('resolution-status').textContent,/3 of 474 checked/);
 assert.equal(f.doc.getElementById('resolution-continue').hidden,true);
 assert.equal(f.timers.length,1);
 f.timers.shift()();await tick();
 assert.equal(f.doc.getElementById('resolution-continue').hidden,false);
 assert.equal(f.doc.getElementById('resolution-error-list').querySelectorAll('script').length,0);
 assert.equal(f.doc.getElementById('resolution-error-list').children.length,2);
 assert.equal(f.doc.getElementById('resolution-cancel').hidden,true);
 assert.equal(f.timers.length,0);assert.deepEqual(f.redirects,[]);
});
test('successful preparation automatically opens formats and transient errors retry',async t=>{
 const f=fixture([{status:503},{status:200,body:{total:500,completed:500,ready:500,failures:[],finished:true,cancelled:false,elapsedSeconds:120,canContinue:true}}]);
 t.after(()=>f.dom.window.close());await tick();
 assert.match(f.doc.getElementById('resolution-status').textContent,/Retrying/);
 f.timers.shift()();await tick();
 assert.deepEqual(f.redirects,['http://localhost/playlist/formats?job=test-token']);
 assert.equal(f.timers.length,0);
});
test('cancelled and expired preparations stop polling',async t=>{
 for(const response of [{status:404},{status:200,body:{total:500,completed:2,ready:2,failures:[],finished:true,cancelled:true,elapsedSeconds:2,canContinue:false}}]){
  const f=fixture([response]);t.after(()=>f.dom.window.close());await tick();
  assert.equal(f.timers.length,0);assert.deepEqual(f.redirects,[]);
  assert.equal(f.doc.getElementById('resolution-continue').hidden,true);
 }
});
