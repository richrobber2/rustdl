/* Dev-only synthetic WebView instrumentation. Never loaded by production pages. */
(()=>{
  const observers=[], support={}, counts={longTasks:0,longTaskMs:0,layoutShiftScore:0,layoutShiftWithoutRecentInput:0};
  const errors=[];
  let interrupted=false, running=false;
  const onError=()=>{if(errors.length<10)errors.push('page-script-error');};
  const onVisibility=()=>{if(document.hidden)interrupted=true;};
  addEventListener('error',onError);addEventListener('unhandledrejection',onError);
  document.addEventListener('visibilitychange',onVisibility);
  const accept=entries=>{for(const e of entries){
    if(e.entryType==='longtask'){counts.longTasks++;counts.longTaskMs+=e.duration;}
    if(e.entryType==='layout-shift'){counts.layoutShiftScore+=e.value;if(!e.hadRecentInput)counts.layoutShiftWithoutRecentInput+=e.value;}
  }};
  for(const type of ['longtask','layout-shift']){
    support[type]=false;
    try{
      if(!PerformanceObserver.supportedEntryTypes.includes(type))continue;
      const observer=new PerformanceObserver(list=>accept(list.getEntries()));
      observer.observe({type,buffered:true});observers.push(observer);support[type]=true;
    }catch(_){}
  }
  const drain=()=>observers.forEach(o=>accept(o.takeRecords()));
  const frame=()=>new Promise((resolve,reject)=>{
    let id;const timer=setTimeout(()=>{cancelAnimationFrame(id);reject(Error('frame-timeout'));},2000);
    id=requestAnimationFrame(()=>{clearTimeout(timer);if(interrupted||document.hidden)reject(Error('backgrounded'));else resolve();});
  });
  const geometry=()=>{
    const box=document.getElementById('playlist-items').getBoundingClientRect();
    const dock=document.querySelector('.playlist-tools').getBoundingClientRect();
    return {containerHeight:box.height,containerTop:box.top,dockTop:dock.top,scrollY,scrollHeight:document.documentElement.scrollHeight};
  };
  const cleanup=()=>{observers.forEach(o=>o.disconnect());removeEventListener('error',onError);removeEventListener('unhandledrejection',onError);document.removeEventListener('visibilitychange',onVisibility);};
  window.RustDLRenderingMetrics={abort(){interrupted=true;},async run(){
    if(running)return;running=true;
    const result={schema:1,mode:'webview-metrics',support,interactions:[],pass:false};
    const started=performance.now();
    const check=(ok,message)=>{if(!ok)throw Error(message);};
    try{
      RustDLDev.beginPhase('WebView pagination metrics');
      check(!document.hidden&&!interrupted,'backgrounded');
      const count=Number(document.documentElement.dataset.syntheticCount);
      check([100,1000,5000].includes(count),'invalid synthetic fixture');result.entryCount=count;
      const rows=[...document.querySelectorAll('.candidate')];
      const first=()=>Number(rows[0].querySelector('input').dataset.index);
      check(rows.length===32&&first()===0,'invalid initial row pool');
      await frame();await frame();
      const action=async(name,change,verify)=>{
        drain();const before={...counts},geometryBefore=geometry();
        const start=performance.now();change();const handlerMs=performance.now()-start;
        const layoutStart=performance.now(),geometryAfter=geometry();
        const forcedGeometryReadMs=performance.now()-layoutStart;
        await frame();await frame();
        const throughTwoRafMs=performance.now()-start;
        check(verify(),name+' result mismatch');
        check(rows.every((row,i)=>document.querySelectorAll('.candidate')[i]===row),'row pool replaced');
        drain();const delta={};for(const key of Object.keys(counts))delta[key]=counts[key]-before[key];
        if(!support.longtask){delta.longTasks=null;delta.longTaskMs=null;}
        if(!support['layout-shift']){delta.layoutShiftScore=null;delta.layoutShiftWithoutRecentInput=null;}
        result.interactions.push({name,handlerMs,forcedGeometryReadMs,throughTwoRafMs,geometryBefore,geometryAfter,observed:delta});
      };
      const click=selector=>document.querySelector(selector).click();
      await action('next',()=>click('[data-page-next]'),()=>first()===32);
      await action('previous',()=>click('[data-page-previous]'),()=>first()===0);
      await action('select-all',()=>click('#select-all'),()=>Number(document.getElementById('selected-count').textContent)===count);
      await action('clear-selection',()=>click('#clear-selection'),()=>Number(document.getElementById('selected-count').textContent)===0);
      const filter=document.getElementById('playlist-filter');
      const setFilter=value=>{filter.value=value;filter.dispatchEvent(new Event('input'));};
      await action('filter',()=>setFilter('Artist 0'),()=>Number(document.getElementById('playlist-items').dataset.filteredCount)===count/2);
      await action('clear-filter',()=>setFilter(''),()=>Number(document.getElementById('playlist-items').dataset.filteredCount)===count);
      result.pass=!interrupted&&!document.hidden&&!errors.length;
    }catch(error){result.error=String(error);}
    finally{
      drain();result.totals={...counts};
      for(const field of ['longTasks','longTaskMs'])if(!support.longtask)result.totals[field]=null;
      for(const field of ['layoutShiftScore','layoutShiftWithoutRecentInput'])if(!support['layout-shift'])result.totals[field]=null;
      result.firstContentfulPaintMs=(performance.getEntriesByName?.('first-contentful-paint')||[])[0]?.startTime??null;
      result.elapsedMs=performance.now()-started;result.aborted=interrupted||document.hidden;result.jsErrors=errors;
      result.viewport={width:innerWidth,height:innerHeight,dpr:devicePixelRatio};
      result.limitations=['Two rAF callbacks do not prove pixel presentation; forced geometry reads perturb layout timing.',
        'Layout shift sums include initial load; they are not session-window CLS. Per-action attribution is approximate.',
        'Synthetic programmatic interactions; no network, media decoding, cross-document navigation or RSS measurement.'];
      cleanup();RustDLDev.caseComplete(JSON.stringify(result));
    }
  }};
})();
