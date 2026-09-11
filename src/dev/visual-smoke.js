(() => {
  const times=[];let start,last;
  RustDLDev.beginPhase('visual-screen');
  const step=now=>{
    if(start===undefined){start=now;last=now;requestAnimationFrame(step);return}
    times.push(now-last);last=now;
    const elapsed=now-start,bottom=Math.max(0,document.documentElement.scrollHeight-innerHeight);
    if(elapsed<1000){scrollTo(0,bottom*Math.min(1,elapsed/800));requestAnimationFrame(step);return}
    const bottomAudit=window.RustDLVisualAudit.run();
    scrollTo(0,0);
    requestAnimationFrame(()=>{
    const overflow=document.documentElement.scrollWidth-innerWidth;
    const audit=window.RustDLVisualAudit.merge([bottomAudit,window.RustDLVisualAudit.run()]);
    const clipped=audit.issues.filter(issue=>issue.severity==='error'&&['element-clipped','outside-viewport'].includes(issue.type));
    times.sort((a,b)=>a-b);
    RustDLDev.caseComplete(JSON.stringify({aborted:document.hidden||window.devInterrupted,viewport:{width:innerWidth,height:innerHeight},overflowPixels:Math.max(0,overflow),audit,clippedControls:clipped,jsErrors:window.devErrors||[],rafSamples:times.length,rafIntervalP95Ms:times[Math.max(0,Math.ceil(times.length*.95)-1)],scrollableCssPixels:bottom,pass:!document.hidden&&!window.devInterrupted&&overflow<=1&&audit.pass&&!(window.devErrors||[]).length}));
    });
  };requestAnimationFrame(step);
})();
