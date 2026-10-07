const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {JSDOM} = require('jsdom');
const root = path.resolve(__dirname, '../..');
const script = fs.readFileSync(path.join(root, 'assets/js/playback.js'), 'utf8');
const template = fs.readFileSync(path.join(root, 'assets/html/gallery.html'), 'utf8');
const item = id => ({href:'/watch/synthetic-'+id+'.m4a', filename:'synthetic-'+id+'.m4a',state:'Ready',title:'Synthetic audio',subtitle:String(id),kind:'audio',thumbnail:null,transitionName:null});
const page = entries => template.replace(/\{(library_heading|collection_nav|gallery_json|initial_cards|library_summary)\}/g, (_,key)=>({library_heading:'Synthetic library',collection_nav:'',gallery_json:JSON.stringify(entries),initial_cards:'',library_summary:entries.length+' media items'})[key]);
const wait = ms => new Promise(resolve=>setTimeout(resolve,ms));
function fixture(entries, pathname='/', beforeStart=()=>{}) {
  const dom=new JSDOM(page(entries),{url:'http://localhost'+pathname,runScripts:'outside-only',pretendToBeVisual:true});
  const w=dom.window;let y=0;let next=entries;let requests=[];let responseFactory=null;let intersection;
  Object.defineProperty(w,'scrollY',{get:()=>y});w.scrollTo=options=>{y=options.top};
  w.setInterval=()=>0;
  w.IntersectionObserver=class { constructor(callback){intersection=callback} observe(){} };
  w.HTMLElement.prototype.getBoundingClientRect=function(){
    const children=[...w.document.querySelectorAll('#gallery-items > .media-card-shell,#gallery-items > .media-card')];
    const index=this.id==='gallery-sentinel'?children.length:children.indexOf(this);const top=index<0?0:500+index*100-y;return {top,bottom:top+100,left:0,right:100,width:100,height:100};
  };
  w.fetch=async url=>{
    if(url.startsWith('/__app/state.json'))return {ok:true,json:async()=>({jobs:[],active:0})};
    requests.push(url);if(responseFactory)return responseFactory();
    return {ok:true,text:async()=>page(next)};
  };
  beforeStart(w);
  w.eval(script);
  return {w,dom,requests,setNext:entries=>next=entries,setResponse:fn=>responseFactory=fn,more:()=>intersection([{isIntersecting:true}]),emit:()=>w.dispatchEvent(new w.Event('rustdl:gallery')),cards:()=>[...w.document.querySelectorAll('#gallery-items > .media-card-shell,#gallery-items > .media-card')]};
}

test('import preserves the visible card, loaded batches, focused search and filter',async t=>{
  const entries=Array.from({length:100},(_,i)=>item(i));const f=fixture(entries,'/gallery/playlist/PL1234567890');t.after(()=>f.dom.window.close());
  f.more();assert.equal(f.cards().length,64);
  const search=f.w.document.querySelector('.gallery-search');search.value='Synthetic';search.dispatchEvent(new f.w.Event('input'));await wait(25);
  f.w.document.querySelector('[data-gallery-filter="audio"]').click();search.focus();f.w.scrollTo({top:4500});
  const anchor=f.cards().find(node=>node.getBoundingClientRect().bottom>0);const top=anchor.getBoundingClientRect().top;
  f.setNext([item('import'),...entries]);f.emit();f.emit();f.emit();await wait(300);
  assert.equal(f.requests.length,1);assert.equal(f.requests[0],'/gallery/playlist/PL1234567890');
  assert.ok(f.cards().includes(anchor));assert.equal(anchor.getBoundingClientRect().top,top);
  assert.ok(f.cards().length>=64);assert.equal(search.value,'Synthetic');assert.equal(f.w.document.activeElement,search);
  assert.equal(f.w.document.querySelector('[data-gallery-filter="audio"]').getAttribute('aria-pressed'),'true');
  assert.equal(f.w.document.querySelector('.library-count').textContent,'101 media items');
  assert.equal(f.w.location.pathname,'/gallery/playlist/PL1234567890');
});

test('first import fills an empty gallery without navigation',async t=>{
  const f=fixture([]);t.after(()=>f.dom.window.close());f.w.scrollTo({top:150});
  f.setNext([item(1)]);f.emit();await wait(300);
  assert.equal(f.cards().length,1);assert.equal(f.w.scrollY,150);
  assert.equal(f.w.document.querySelector('#gallery-empty').hidden,true);
  assert.equal(f.w.document.querySelector('.gallery-update-toast').textContent,'Library updated');
});

test('a notification during a pending refresh schedules another refresh',async t=>{
  const f=fixture([item(1)]);t.after(()=>f.dom.window.close());let release;
  f.setResponse(()=>new Promise(resolve=>release=()=>resolve({ok:true,text:async()=>page([item(1)])})));
  f.emit();await wait(250);f.emit();await wait(250);
  f.setResponse(null);f.setNext([item(2),item(1)]);release();await wait(300);
  assert.equal(f.requests.length,2);assert.equal(f.cards().length,2);
});

test('invalid refresh data keeps current cards and scroll intact',async t=>{
  const f=fixture([item(1)]);t.after(()=>f.dom.window.close());const card=f.cards()[0];f.w.scrollTo({top:120});
  f.setResponse(()=>({ok:true,text:async()=>'<html>Unavailable</html>'}));f.emit();await wait(300);
  assert.equal(f.cards()[0],card);assert.equal(f.w.scrollY,120);assert.equal(f.w.location.pathname,'/');
});


test('imports leave users at the top when they have not scrolled',async t=>{
  const f=fixture([item(1)]);t.after(()=>f.dom.window.close());
  f.setNext([item(2),item(1)]);f.emit();await wait(300);
  assert.equal(f.cards().length,2);assert.equal(f.w.scrollY,0);
});


test('startup and matching filters retain server-rendered thumbnail elements',async t=>{
  const entries=[{...item(1),kind:'video',thumbnail:'synthetic-1.mp4'}];let original,image;
  const f=fixture(entries,'/',w=>{
    original=w.document.createElement('div');original.className='media-card-shell';
    original.innerHTML='<a class="media-card" data-gallery-index="0" href="/watch/synthetic-1.m4a"><img class="media-art" src="/thumbnail/synthetic-1.mp4.jpg"></a>';
    image=original.querySelector('img');
    w.document.getElementById('gallery-items').prepend(original);
  });t.after(()=>f.dom.window.close());
  assert.equal(f.cards()[0],original);assert.equal(original.querySelector('img'),image);
  assert.equal(image.dataset.thumbnailWired,'true');
  const search=f.w.document.querySelector('.gallery-search');search.value='Synthetic';search.dispatchEvent(new f.w.Event('input'));await wait(25);
  f.w.document.querySelector('[data-gallery-filter="video"]').click();
  assert.equal(f.cards()[0],original);assert.equal(original.querySelector('img'),image);
  search.value='no matching cards';search.dispatchEvent(new f.w.Event('input'));await wait(25);
  assert.equal(f.cards().length,0);assert.equal(f.w.document.getElementById('gallery-empty').hidden,false);
});


test('batching keeps filling when the sentinel stays near the viewport',async t=>{
  const f=fixture(Array.from({length:130},(_,i)=>item(i)),'/',w=>{
    w.document.getElementById('gallery-sentinel').getBoundingClientRect=()=>({top:100,bottom:101});
  });t.after(()=>f.dom.window.close());
  // No observer notification: startup must fill repeatedly without waiting for a threshold crossing.
  for(let i=0;i<30&&f.cards().length<130;i++)await wait(20);
  assert.equal(f.cards().length,130);
  assert.equal(f.w.document.getElementById('gallery-sentinel').hidden,true);
  await wait(25);assert.equal(f.cards().length,130);
});

test('scroll fallback loads the next batch without IntersectionObserver',async t=>{
  const f=fixture(Array.from({length:100},(_,i)=>item(i)),'/',w=>{w.IntersectionObserver=undefined});
  t.after(()=>f.dom.window.close());await wait(25);assert.equal(f.cards().length,32);
  f.w.scrollTo({top:3500});f.w.dispatchEvent(new f.w.Event('scroll'));
  for(let i=0;i<30&&f.cards().length===32;i++)await wait(20);
  assert.equal(f.cards().length,64);
});

function fakeClock(w){
  let now=0,id=0;const timers=new Map();
  w.setTimeout=(fn,delay)=>{timers.set(++id,{fn,at:now+delay});return id};
  w.clearTimeout=id=>timers.delete(id);
  return {advance:async ms=>{
    const end=now+ms;
    for(;;){
      const next=[...timers].filter(([,value])=>value.at<=end).sort((a,b)=>a[1].at-b[1].at)[0];
      if(!next)break;now=next[1].at;timers.delete(next[0]);next[1].fn();
      for(let i=0;i<20;i++)await Promise.resolve();
    }
    now=end;
  }};
}

test('a hung response body times out and cannot overwrite a newer refresh',async t=>{
  let timer,release;
  const f=fixture([item(1)],'/',w=>timer=fakeClock(w));t.after(()=>f.dom.window.close());
  f.setResponse(()=>({ok:true,text:()=>new Promise(resolve=>release=resolve)}));
  f.emit();await timer.advance(200);f.emit();
  await timer.advance(10000);
  f.setResponse(null);f.setNext([item(2)]);await timer.advance(1500);
  assert.equal(f.requests.length,2);
  assert.equal(f.cards()[0].querySelector('a').getAttribute('href'),item(2).href);
  release(page([item(3)]));await timer.advance(0);await Promise.resolve();
  assert.equal(f.cards()[0].querySelector('a').getAttribute('href'),item(2).href);
});

test('continuous notifications do not postpone the first refresh',async t=>{
  let timer;const f=fixture([item(1)],'/',w=>timer=fakeClock(w));t.after(()=>f.dom.window.close());
  for(let i=0;i<10;i++){f.emit();await timer.advance(100)}
  assert.ok(f.requests.length>=1);
});

test('thumbnail success cancels duplicate error retries',async t=>{
  let timer;const f=fixture([{...item(1),kind:'video',thumbnail:'synthetic.mp4'}],'/',w=>timer=fakeClock(w));t.after(()=>f.dom.window.close());
  const image=f.w.document.querySelector('#gallery-items img'),source=image.src;
  image.dispatchEvent(new f.w.Event('error'));image.dispatchEvent(new f.w.Event('error'));
  image.dispatchEvent(new f.w.Event('load'));await timer.advance(60000);
  assert.equal(image.src,source);assert.equal(image.dataset.thumbnailRetry,'0');
});

test('slow thumbnails keep recovering with bounded backoff and stop when detached',async t=>{
  let timer;const f=fixture([{...item(1),kind:'video',thumbnail:'synthetic.mp4'}],'/',w=>timer=fakeClock(w));t.after(()=>f.dom.window.close());
  const image=f.w.document.querySelector('#gallery-items img');
  for(const delay of [300,600,1200,2400,4800,9600,19200,30000]){
    const source=image.src;image.dispatchEvent(new f.w.Event('error'));
    await timer.advance(delay);assert.notEqual(image.src,source);
  }
  assert.equal(image.dataset.thumbnailRetry,'8');
  const source=image.src;image.dispatchEvent(new f.w.Event('error'));image.remove();await timer.advance(60000);
  assert.equal(image.src,source);
});

test('background thumbnail retries pause and resume when visible',async t=>{
  let timer,hidden=false;
  const f=fixture([{...item(1),kind:'video',thumbnail:'synthetic.mp4'}],'/',w=>{
    timer=fakeClock(w);Object.defineProperty(w.document,'hidden',{get:()=>hidden});
  });t.after(()=>f.dom.window.close());
  const image=f.w.document.querySelector('#gallery-items img'),source=image.src;
  image.dispatchEvent(new f.w.Event('error'));hidden=true;await timer.advance(30000);assert.equal(image.src,source);
  hidden=false;f.w.document.dispatchEvent(new f.w.Event('visibilitychange'));await wait(25);await timer.advance(300);
  assert.notEqual(image.src,source);
});

test('offscreen thumbnails wait for scrolling back into range',async t=>{
  let timer;const f=fixture([{...item(1),kind:'video',thumbnail:'synthetic.mp4'}],'/',w=>timer=fakeClock(w));t.after(()=>f.dom.window.close());
  const image=f.w.document.querySelector('#gallery-items img'),source=image.src;
  image.getBoundingClientRect=()=>({top:10000,bottom:10100});
  image.dispatchEvent(new f.w.Event('error'));await timer.advance(60000);assert.equal(image.src,source);
  image.getBoundingClientRect=()=>({top:0,bottom:100});f.w.dispatchEvent(new f.w.Event('scroll'));
  await wait(25);await timer.advance(300);assert.notEqual(image.src,source);
});

test('refresh recovers automatically after more than three failures',async t=>{
  let timer;const f=fixture([item(1)],'/',w=>timer=fakeClock(w));t.after(()=>f.dom.window.close());
  f.setResponse(()=>({ok:false}));f.emit();await timer.advance(200);
  await timer.advance(1500);await timer.advance(3000);await timer.advance(6000);
  assert.equal(f.requests.length,4);
  f.setResponse(null);f.setNext([item(2)]);await timer.advance(12000);
  assert.equal(f.requests.length,5);assert.equal(f.cards()[0].querySelector('a').getAttribute('href'),item(2).href);
});


test('Up Next imports legacy state into the Android bridge and saves menu changes to both stores', t=>{
  let nativeQueue=['123-1.m4a'];let imported=[];let saved=[];
  const entry={...item(1),href:'/watch/123-1.m4a',filename:'123-1.m4a'};
  const f=fixture([entry], '/', w=>{
    w.localStorage.setItem('rustdl:up-next:v1', JSON.stringify(['123-1.m4a']));
    w.RustDLPlayback={getContinueWatching:()=> '[]',
      importPlaybackQueue:value=>imported.push(JSON.parse(value)),
      getPlaybackQueue:()=>JSON.stringify(nativeQueue),
      savePlaybackQueue:value=>{nativeQueue=JSON.parse(value);saved.push(nativeQueue)}};
  });
  t.after(()=>f.dom.window.close());
  assert.deepEqual(imported[0], ['123-1.m4a']);
  f.w.document.querySelector('.card-menu-button').click();
  const button=f.w.document.querySelector('[data-card-action="queue"]');
  assert.equal(button.textContent, 'Remove from Up Next');
  button.click();
  assert.deepEqual(saved.at(-1), []);
  assert.deepEqual(JSON.parse(f.w.localStorage.getItem('rustdl:up-next:v1')), []);
});
