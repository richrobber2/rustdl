(async()=>{
  const result={interactions:[],jsErrors:window.devErrors||[]},intervals=[],audits=[];
  const frame=()=>new Promise(resolve=>requestAnimationFrame(resolve));
  const percentile=(values,p)=>{const sorted=[...values].sort((a,b)=>a-b);return sorted[Math.max(0,Math.ceil(sorted.length*p)-1)]||0;};
  const tasks=[];
  try{new PerformanceObserver(list=>tasks.push(...list.getEntries().map(e=>({start:e.startTime,duration:e.duration})))).observe({type:'longtask',buffered:true});}catch(_){}
  RustDLDev.beginPhase('batch-500');
  const started=performance.now();
  const check=(condition,message)=>{if(!condition)throw Error(message);};
  const action=async(name,change,verify)=>{
    const begin=performance.now();change();const handlerMs=performance.now()-begin;
    await frame();await frame();
    check(verify(),name+' produced an incorrect result');
    result.interactions.push({name,handlerMs,throughNextFrameMs:performance.now()-begin});
  };
  try{
    const boxes=[...document.querySelectorAll('.candidate input')];
    const formats=[...document.querySelectorAll('.quality-card select')];
    result.itemCount=boxes.length||formats.length||document.querySelectorAll('article[data-filename]').length;
    check(result.itemCount===500,'Expected 500 rendered entries');
    if(boxes.length){
      const click=id=>document.getElementById(id).click();
      const checked=()=>boxes.filter(box=>box.checked).length;
      const filter=document.getElementById('playlist-filter');
      await action('Select all',()=>click('select-all'),()=>checked()===500&&!document.getElementById('playlist-continue').disabled&&new FormData(document.getElementById('playlist-form')).getAll('pick').length===500);
      await action('Clear',()=>click('clear-selection'),()=>checked()===0);
      await action('Filter',()=>{filter.value='Artist 0';filter.dispatchEvent(new Event('input'));},()=>Number(document.getElementById('playlist-items').dataset.filteredCount||document.querySelectorAll('.candidate:not([hidden])').length)===250);
      await action('Select visible',()=>click('select-visible'),()=>checked()===250);
      await action('Select all while filtered',()=>click('select-all'),()=>checked()===500);
      await action('Clear filter',()=>{filter.value='';filter.dispatchEvent(new Event('input'));},()=>Number(document.getElementById('playlist-items').dataset.filteredCount||document.querySelectorAll('.candidate:not([hidden])').length)===500);
      await action('First 10',()=>click('select-first'),()=>checked()===10);
      await action('Restore all',()=>click('select-all'),()=>checked()===500);
    }
    if(formats.length){
      for(const [preset,kind,height] of [['720','video','720'],['audio','audio','0'],['best','video','1080']]){
        await action('Apply '+preset+' to 500',()=>{const select=document.getElementById('bulk-format');select.value=preset;select.dispatchEvent(new Event('change'));},()=>formats.every(select=>select.selectedOptions[0].dataset.kind===kind&&select.selectedOptions[0].dataset.height===height));
      }
    }
    if(document.querySelector('[data-page-next]')){
      const rows=[...document.querySelectorAll('.candidate,.quality-card')];const visited=new Set();let pagesVisited=0;
      do{
        const visible=rows.filter(row=>!row.hidden);check(visible.length<=32,'Page exceeded 32 rows');visible.forEach(row=>visited.add(row));pagesVisited++;audits.push(RustDLVisualAudit.run());
        const next=document.querySelector('[data-page-next]');if(next.disabled)break;next.click();await frame();await frame();
      }while(pagesVisited<100);
      check(visited.size===result.itemCount,'Pagination did not expose every item');result.pagesVisited=pagesVisited;result.maxRowsPerPage=32;
      while(!document.querySelector('[data-page-previous]').disabled)document.querySelector('[data-page-previous]').click();
    }
    scrollTo(0,0);await frame();
    const bottom=Math.max(0,document.documentElement.scrollHeight-innerHeight);
    result.scrollableCssPixels=bottom;
    let start,last,maxY=0;
    await new Promise(resolve=>{
      const step=now=>{
        if(start===undefined){start=last=now;requestAnimationFrame(step);return;}
        intervals.push(now-last);last=now;
        const elapsed=now-start;
        if(elapsed>=8000||document.hidden){resolve();return;}
        const progress=elapsed<=4000?elapsed/4000:1-(elapsed-4000)/4000;
        scrollTo(0,Math.round(bottom*Math.max(0,progress)));maxY=Math.max(maxY,scrollY);
        requestAnimationFrame(step);
      };requestAnimationFrame(step);
    });
    // Verify the last item is reachable, then return to the controls.
    scrollTo(0,bottom);await frame();audits.push(RustDLVisualAudit.run());result.reachedBottom=scrollY>=bottom-2;
    result.maxAnimatedScrollY=maxY;scrollTo(0,0);await frame();
    result.viewport={width:innerWidth,height:innerHeight,dpr:devicePixelRatio};
    result.overflowPixels=Math.max(0,document.documentElement.scrollWidth-innerWidth);
    audits.push(RustDLVisualAudit.run());result.audit=RustDLVisualAudit.merge(audits);
    result.clippedControls=result.audit.issues.filter(issue=>issue.severity==='error'&&['element-clipped','outside-viewport'].includes(issue.type));
    result.aborted=document.hidden||window.devInterrupted;
    result.pass=!result.aborted&&result.reachedBottom&&result.overflowPixels<=1&&result.audit.pass&&!result.jsErrors.length;
  }catch(error){result.pass=false;result.error=String(error);}
  result.elapsedMs=performance.now()-started;
  result.rafSamples=intervals.length;result.rafIntervalP95Ms=percentile(intervals,.95);result.rafIntervalMaxMs=Math.max(0,...intervals);
  result.longTasks=tasks.filter(t=>t.start>=started).length;
  result.longTaskTotalMs=tasks.filter(t=>t.start>=started).reduce((sum,t)=>sum+t.duration,0);
  result.firstContentfulPaintMs=performance.getEntriesByName('first-contentful-paint')[0]?.startTime??null;
  RustDLDev.caseComplete(JSON.stringify(result));
})();
