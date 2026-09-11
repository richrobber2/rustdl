(() => {
  let running=false, stopped=false;
  const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
  const percentile=(values,p)=>{const sorted=[...values].sort((a,b)=>a-b);return sorted[Math.max(0,Math.ceil(sorted.length*p)-1)]||0};
  const cards=()=>document.querySelectorAll('#gallery-items > .media-card-shell').length;
  let tasks=[];
  try {new PerformanceObserver(list=>tasks.push(...list.getEntries().map(e=>({start:e.startTime,duration:e.duration})))).observe({type:'longtask',buffered:true})} catch(_) {}
  const phase=speed=>new Promise(resolve=>{
    const name=speed+' CSS px/s';scrollTo(0,0);let start=null,last=null,distance=0,direction=1,lastY=scrollY;
    const intervals=[],initialCards=cards();
    RustDLDev.beginPhase(name);
    const step=now=>{
      if(start===null){start=now;last=now;requestAnimationFrame(step);return}
      const dt=now-last;last=now;intervals.push(dt);
      if(stopped||document.hidden||now-start>=12000){
        const result={name,targetCssPixelsPerSecond:speed,elapsedMs:now-start,aborted:stopped||document.hidden,
          rafCallbacksPerSecond:intervals.length*1000/(now-start),rafIntervalMedianMs:percentile(intervals,.5),rafIntervalP95Ms:percentile(intervals,.95),rafIntervalMaxMs:Math.max(...intervals),
          distanceCssPixels:distance,actualCssPixelsPerSecond:distance*1000/(now-start),initialCards,finalCards:cards(),
          longTasks:tasks.filter(t=>t.start>=start&&t.start<=now).length,
          longTaskTotalMs:tasks.filter(t=>t.start>=start&&t.start<=now).reduce((sum,t)=>sum+t.duration,0)};
        RustDLDev.endPhase(JSON.stringify(result));resolve(result);return;
      }
      const bottom=Math.max(0,document.documentElement.scrollHeight-innerHeight);
      if(direction>0&&scrollY>=bottom-2&&document.getElementById('gallery-sentinel').hidden)direction=-1;
      if(direction<0&&scrollY<=0)direction=1;
      scrollTo(0,Math.max(0,Math.min(bottom,scrollY+direction*speed*dt/1000)));
      distance+=Math.abs(scrollY-lastY);lastY=scrollY;
      requestAnimationFrame(step);
    };requestAnimationFrame(step);
  });
  window.runVisualBenchmark=async()=>{
    if(running)return;running=true;stopped=false;
    const results=[];
    for(const speed of [1000,3000,8000]){if(stopped)break;await wait(500);results.push(await phase(speed));}
    RustDLDev.complete(JSON.stringify({results,viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio},userAgent:navigator.userAgent,aborted:stopped||results.some(r=>r.aborted),method:'Programmatic visible WebView scrolling; rAF cadence is not presented-frame FPS; synthetic SVG thumbnails, no video decoding.'}));
    running=false;
  };
  window.stopVisualBenchmark=()=>{stopped=true};
  document.addEventListener('visibilitychange',()=>{if(document.hidden)stopped=true});
})();
