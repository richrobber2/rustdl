const {test}=require('node:test');
const assert=require('node:assert/strict');
const {JSDOM,VirtualConsole}=require('jsdom');
const script=require('./read-script.cjs');
function fixture(t,supported=false,prepare=()=>{}){
 const console=new VirtualConsole();
 console.on('jsdomError',error=>{if(!error.message.includes('navigation'))throw error;});
 const dom=new JSDOM('<body><a id="local" href="/settings"><span>Settings</span></a><a id="next" href="/queue">Queue</a><a id="hash" href="#section">Here</a><a id="external" href="https://example.invalid/">External</a><a id="blank" target="_blank" href="/activity">New tab</a><a id="download" download href="/file">Download</a><a id="mode" href="rustdl://mode/normal">Mode</a></body>',{url:'http://localhost/',runScripts:'outside-only',pretendToBeVisual:true,virtualConsole:console});
 const w=dom.window,timers=new Map();let serial=0;
 t.after(()=>w.close());w.matchMedia=()=>({matches:false});
 w.setTimeout=(callback,delay)=>{assert.equal(delay,10000);timers.set(++serial,callback);return serial;};w.clearTimeout=id=>timers.delete(id);
 if(supported)w.document.startViewTransition=()=>{};
 prepare(w);w.eval(script('view-transitions.js'));return {w,d:w.document,timers};
}
function click(w,node,options={}){const e=new w.MouseEvent('click',{bubbles:true,cancelable:true,button:0,...options});node.dispatchEvent(e);return e;}
test('local navigation gives immediate feedback without intercepting fallback navigation',t=>{
 const {w,d,timers}=fixture(t);const e=click(w,d.querySelector('#local span'));
 assert.equal(e.defaultPrevented,false);assert.equal(d.documentElement.dataset.viewTransitions,'fallback');
 assert.equal(d.documentElement.dataset.navigationPending,'on');assert.equal(d.querySelector('#local').dataset.navigationPending,'on');
 assert.equal(d.querySelectorAll('.rustdl-navigation-progress').length,1);assert.equal(d.querySelector('.rustdl-navigation-progress').hidden,false);assert.equal(timers.size,1);
 w.dispatchEvent(new w.Event('pageshow'));assert.equal(d.documentElement.dataset.navigationPending,undefined);assert.equal(d.querySelector('#local').dataset.navigationPending,undefined);assert.equal(timers.size,0);assert.equal(d.querySelector('.rustdl-navigation-progress').hidden,true);
});
test('cancelled, modified, external, download and same-page links retain normal handling',t=>{
 const {w,d,timers}=fixture(t);
 for(const id of ['hash','external','blank','download','mode'])click(w,d.getElementById(id));
 for(const options of [{ctrlKey:true},{metaKey:true},{shiftKey:true},{altKey:true},{button:1}])click(w,d.getElementById('local'),options);
 d.getElementById('next').addEventListener('click',e=>e.preventDefault());click(w,d.getElementById('next'));
 assert.equal(d.querySelector('.rustdl-navigation-progress'),null);assert.equal(timers.size,0);
});
test('pending feedback reuses one indicator and clears on timeout and page exit',t=>{
 const {w,d,timers}=fixture(t);click(w,d.getElementById('local'));const indicator=d.querySelector('.rustdl-navigation-progress');
 click(w,d.getElementById('next'));assert.equal(d.getElementById('local').dataset.navigationPending,undefined);assert.equal(timers.size,1);assert.equal(d.querySelector('.rustdl-navigation-progress'),indicator);
 [...timers.values()][0]();assert.equal(indicator.hidden,true);assert.equal(d.documentElement.dataset.navigationPending,undefined);
 click(w,d.getElementById('local'));w.dispatchEvent(new w.Event('pagehide'));assert.equal(indicator.hidden,true);assert.equal(timers.size,0);
 w.eval(script('view-transitions.js'));click(w,d.getElementById('next'));assert.equal(timers.size,1);assert.equal(d.querySelectorAll('.rustdl-navigation-progress').length,1);
});
test('reduced motion skips supported cross-document transitions and clears feedback',t=>{
 const {w,d}=fixture(t,true);let skips=0;d.documentElement.dataset.reduceMotion='on';click(w,d.getElementById('local'));
 for(const name of ['pageswap','pagereveal']){const e=new w.Event(name);e.viewTransition={skipTransition:()=>skips++};w.dispatchEvent(e);}
 assert.equal(skips,2);assert.equal(d.documentElement.dataset.navigationPending,undefined);
 d.documentElement.dataset.reduceMotion='off';w.matchMedia=()=>({matches:true});const e=new w.Event('pagereveal');e.viewTransition={skipTransition:()=>skips++};w.dispatchEvent(e);assert.equal(skips,3);
});
test('pointerdown and click select the same shared thumbnail only once',async t=>{
 const {w,d}=fixture(t,true);const link=d.createElement('a');link.className='media-card';link.href='/watch/synthetic.mp4';link.innerHTML='<span class="media-thumb" data-view-transition-name="video-synthetic"></span>';d.body.append(link);
 const thumb=link.firstChild;link.dispatchEvent(new w.MouseEvent('pointerdown',{bubbles:true,button:0}));assert.equal(thumb.style.viewTransitionName,'video-synthetic');
 const changes=[];new w.MutationObserver(records=>changes.push(...records)).observe(thumb,{attributes:true,attributeFilter:['style']});
 click(w,link);await Promise.resolve();assert.equal(changes.length,0);
});

test('shared selection uses a weak reference and handles a collected thumbnail',t=>{
 const refs=[];
 const {w,d}=fixture(t,true,w=>{w.WeakRef=class{constructor(value){this.value=value;refs.push(this)}deref(){return this.value}}});
 const makeCard=name=>{const link=d.createElement('a');link.className='media-card';link.href='/watch/'+name+'.mp4';link.innerHTML='<span class="media-thumb" data-view-transition-name="video-'+name+'"></span>';d.body.append(link);return link};
 const old=makeCard('synthetic-old');old.dispatchEvent(new w.MouseEvent('pointerdown',{bubbles:true,button:0}));click(w,old);
 assert.equal(refs.length,1);assert.equal(refs[0].deref(),old.firstChild);
 old.remove();refs[0].value=undefined; // Simulate collection without relying on GC timing.
 const next=makeCard('synthetic-next');click(w,next);
 assert.equal(refs.length,2);assert.equal(refs[1].deref(),next.firstChild);assert.equal(next.firstChild.style.viewTransitionName,'video-synthetic-next');
});
test('shared selection remains correct when WeakRef is unavailable',t=>{
 const {w,d}=fixture(t,true,w=>{w.WeakRef=undefined});
 const link=d.createElement('a');link.className='media-card';link.href='/watch/synthetic.mp4';link.innerHTML='<span class="media-thumb" data-view-transition-name="video-synthetic"></span>';d.body.append(link);
 link.dispatchEvent(new w.MouseEvent('pointerdown',{bubbles:true,button:0}));click(w,link);
 assert.equal(link.firstChild.style.viewTransitionName,'video-synthetic');
});
