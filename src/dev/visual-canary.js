// Deliberately broken synthetic layout: proves the detector works in this WebView.
requestAnimationFrame(()=>requestAnimationFrame(()=>{
  const audit=RustDLVisualAudit.run(),required=['element-clipped','text-clipped','text-truncated','control-obscured'];
  const detected=new Set(audit.issues.filter(i=>i.severity==='error').map(i=>i.type));
  const missing=required.filter(type=>!detected.has(type));
  const scrollFalsePositive=audit.issues.some(i=>i.selector==='#reachable'&&i.severity==='error');
  RustDLDev.caseComplete(JSON.stringify({pass:missing.length===0&&!scrollFalsePositive&&!document.hidden,canary:true,missingDetections:missing,scrollFalsePositive,audit,jsErrors:window.devErrors||[]}));
}));
