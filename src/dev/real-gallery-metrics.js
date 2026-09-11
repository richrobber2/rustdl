(() => {
  if(window.__devRealGalleryStarted||!document.querySelector('.library'))return;
  window.__devRealGalleryStarted=true;
  let abort=false,loadEvents=0,errorEvents=0,bufferFull=0;
  const origin=location.origin, savedX=scrollX,savedY=scrollY;
  const thumbnail=name=>{try{const u=new URL(name,location.href);return u.origin===origin&&u.pathname.startsWith('/thumbnail/')}catch(_){return false}};
  performance.setResourceTimingBufferSize(3000);
  const bufferListener=()=>bufferFull++;
  performance.addEventListener('resourcetimingbufferfull',bufferListener);
  const load=e=>{if(e.target instanceof HTMLImageElement&&thumbnail(e.target.currentSrc||e.target.src))loadEvents++};
  const error=e=>{if(e.target instanceof HTMLImageElement&&thumbnail(e.target.currentSrc||e.target.src))errorEvents++};
  document.addEventListener('load',load,true);document.addEventListener('error',error,true);
  const visibility=()=>{if(document.hidden)abort=true};document.addEventListener('visibilitychange',visibility);
  window.stopRealGalleryMetrics=()=>{abort=true};
  const wait=ms=>new Promise(resolve=>setTimeout(resolve,ms));
  const quantile=(v,p)=>{const a=[...v].sort((x,y)=>x-y);return a.length?a[Math.max(0,Math.ceil(a.length*p)-1)]:0};
  const backend=async()=>{try{const r=await fetch('/__app/gallery-metrics.json',{cache:'no-store'});return r.ok?await r.json():{}}catch(_){return {}}};
  let longTasks=[];
  let observer;try{observer=new PerformanceObserver(list=>longTasks.push(...list.getEntries().map(e=>({start:e.startTime,duration:e.duration}))));observer.observe({type:'longtask',buffered:true})}catch(_){}
  const hide=document.createElement('style');hide.textContent='.media-art{visibility:hidden!important}';
  async function phase(index){
    if(index===2)document.head.append(hide);
    scrollTo(0,0);await wait(300);
    const before=await backend(),loadsBefore=loadEvents,errorsBefore=errorEvents;
    DevRealGallery.begin(index);
    const result=await new Promise(resolve=>{
      let start,last,lastY=scrollY,distance=0,direction=1;const intervals=[];
      const frame=now=>{
        if(start===undefined){start=last=now;requestAnimationFrame(frame);return}
        const dt=now-last;last=now;intervals.push(dt);
        if(abort||document.hidden||now-start>=15000){
          const resourceStart=index===0?0:start;
          const resources=performance.getEntriesByType('resource').filter(e=>thumbnail(e.name)&&e.startTime>=resourceStart&&e.startTime<=now);
          const images=[...document.querySelectorAll('img.media-art')];
          resolve({phase:index,aborted:abort||document.hidden,elapsedMs:now-start,rafCallbacksPerSecond:intervals.length*1000/(now-start),rafIntervalMedianMs:quantile(intervals,.5),rafIntervalP95Ms:quantile(intervals,.95),rafIntervalMaxMs:Math.max(...intervals),actualCssPixelsPerSecond:distance*1000/(now-start),renderedCards:document.querySelectorAll('#gallery-items > .media-card-shell,#gallery-items > .media-card').length,thumbnailRequests:resources.length,thumbnailDurationMedianMs:quantile(resources.map(e=>e.duration),.5),thumbnailDurationP95Ms:quantile(resources.map(e=>e.duration),.95),thumbnailDurationMaxMs:Math.max(0,...resources.map(e=>e.duration)),thumbnailTransferBytes:resources.reduce((s,e)=>s+e.transferSize,0),thumbnailHttpErrors:resources.filter(e=>e.responseStatus>=400).length,thumbnailStatusKnown:resources.filter(e=>e.responseStatus>0).length,loadEventsAfterObserver:loadEvents-loadsBefore,errorEventsAfterObserver:errorEvents-errorsBefore,imagesTotal:images.length,imagesLoaded:images.filter(e=>e.complete&&e.naturalWidth>0).length,imagesPending:images.filter(e=>!e.complete).length,imagesBroken:images.filter(e=>e.complete&&e.naturalWidth===0).length,longTasks:longTasks.filter(e=>e.start>=start&&e.start<=now).length,longTaskTotalMs:longTasks.filter(e=>e.start>=start&&e.start<=now).reduce((s,e)=>s+e.duration,0),resourceBufferFull:bufferFull});return;
        }
        const bottom=Math.max(0,document.documentElement.scrollHeight-innerHeight);
        if(direction>0&&scrollY>=bottom-2&&document.getElementById('gallery-sentinel')?.hidden)direction=-1;
        if(direction<0&&scrollY<=0)direction=1;
        scrollTo(0,Math.max(0,Math.min(bottom,scrollY+direction*1000*dt/1000)));
        distance+=Math.abs(scrollY-lastY);lastY=scrollY;requestAnimationFrame(frame);
      };requestAnimationFrame(frame);
    });
    result.thumbnailHitsBefore=before.thumbnailHits||0;result.thumbnailMissesBefore=before.thumbnailMisses||0;result.thumbnailPendingBefore=before.thumbnailPending||0;
    const after=await backend();
    for(const key of ['thumbnailHits','thumbnailMisses','thumbnailQueueDrops'])result[key+'Delta']=(after[key]||0)-(before[key]||0);
    result.thumbnailPending=after.thumbnailPending||0;result.libraryItems=after.lastItemCount||0;result.rustRenderMicros=after.lastRenderMicros||0;
    DevRealGallery.record(JSON.stringify(result));return result;
  }
  (async()=>{
    try{for(let i=0;i<3&&!abort;i++)await phase(i);}
    finally{hide.remove();scrollTo(savedX,savedY);observer?.disconnect();document.removeEventListener('load',load,true);document.removeEventListener('error',error,true);document.removeEventListener('visibilitychange',visibility);performance.removeEventListener('resourcetimingbufferfull',bufferListener);DevRealGallery.finish(abort||document.hidden);}
  })();
})();
