const {test}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {JSDOM}=require('jsdom');
const script=fs.readFileSync(path.join(__dirname,'../../assets/js/changelog.js'),'utf8');
const id=index=>'version-0-1-'+(45-index);
const article=index=>`<article id="${id(index)}" data-release-index="${index}"><h2>Synthetic release ${index}</h2></article>`;
function page(hash='') {
 const dom=new JSDOM(`<select id="version-jump">${Array.from({length:46},(_,i)=>`<option value="${id(i)}">${i}</option>`).join('')}</select><button id="version-go">Go</button><section class="releases" data-next="5" data-total="46">${Array.from({length:5},(_,i)=>article(i)).join('')}</section><button id="load-releases">More</button><span id="release-load-status"></span><span id="release-sentinel"></span>`,{url:'http://localhost/changelog'+hash,runScripts:'outside-only'});
 const w=dom.window,calls=[],scrolls=[];let fail=false,observer;
 w.matchMedia=()=>({matches:true});
 w.HTMLElement.prototype.scrollIntoView=function(options){scrolls.push({id:this.id,options})};
 w.IntersectionObserver=class {constructor(fn){observer=fn}observe(){}};
 w.fetch=async url=>{
  calls.push(url);if(fail)throw new Error('offline');
  const query=new URL(url,'http://localhost').searchParams,target=query.get('version');
  const start=target?45-Number(target.split('-').at(-1)):Number(query.get('offset'));
  const count=target?1:Math.min(8,46-start);
  return {ok:true,json:async()=>({html:Array.from({length:count},(_,i)=>article(start+i)).join(''),next:start+count,total:46})};
 };
 w.eval(script);
 return {dom,w,calls,scrolls,fail:value=>{fail=value},intersect:()=>observer([{isIntersecting:true}])};
}
const settle=()=>new Promise(resolve=>setImmediate(resolve));
test('older entries load only on demand in bounded batches, without duplicate requests',async()=>{
 const {dom,w,calls,intersect}=page();assert.equal(calls.length,0);assert.equal(w.document.querySelectorAll('article').length,5);
 intersect();intersect();await settle();assert.equal(calls.length,1);assert.equal(w.document.querySelectorAll('article').length,13);
 w.document.querySelector('#load-releases').click();await settle();assert.equal(w.document.querySelectorAll('article').length,21);dom.window.close();
});
test('oldest deep link fetches one entry, keeps chronological order, and respects reduced motion',async()=>{
 const {dom,w,calls,scrolls}=page('#version-0-1-0');await settle();
 assert.equal(calls.length,1);assert.match(calls[0],/version=version-0-1-0/);assert.equal(w.document.querySelectorAll('article').length,6);
 assert.equal(scrolls[0].id,'version-0-1-0');assert.equal(scrolls[0].options.behavior,'auto');
 for(let i=0;i<6;i++){w.document.querySelector('#load-releases').click();await settle()}
 const articles=[...w.document.querySelectorAll('article')];assert.equal(articles.length,46);assert.equal(new Set(articles.map(a=>a.id)).size,46);
 assert.deepEqual(articles.map(a=>Number(a.dataset.releaseIndex)),Array.from({length:46},(_,i)=>i));assert.equal(w.document.querySelector('#load-releases').hidden,true);dom.window.close();
});
test('failed loads retain existing releases and allow an explicit retry',async()=>{
 const {dom,w,calls,fail,intersect}=page();fail(true);intersect();await settle();
 assert.equal(w.document.querySelectorAll('article').length,5);assert.match(w.document.querySelector('#release-load-status').textContent,/retry/);
 intersect();await settle();assert.equal(calls.length,1);
 fail(false);w.document.querySelector('#load-releases').click();await settle();assert.equal(w.document.querySelectorAll('article').length,13);dom.window.close();
});
