(()=>{
  const picker=document.querySelector('#version-jump'),go=document.querySelector('#version-go');
  const releases=document.querySelector('.releases'),more=document.querySelector('#load-releases');
  const status=document.querySelector('#release-load-status'),sentinel=document.querySelector('#release-sentinel');
  let next=Number(releases.dataset.next),total=Number(releases.dataset.total),loading=false,failed=false,jumpGeneration=0;
  const update=()=>{more.hidden=next>=total;more.disabled=loading;releases.setAttribute('aria-busy',String(loading))};
  const request=async query=>{
    const controller=new AbortController(),timer=setTimeout(()=>controller.abort(),15000);
    try{
      const response=await fetch('/__app/changelog.json?'+query,{cache:'no-store',signal:controller.signal});
      if(!response.ok)throw new Error('Could not load release notes');
      const data=await response.json();
      if(typeof data.html!=='string'||!Number.isInteger(data.next)||!Number.isInteger(data.total)||data.next<0||data.next>data.total)throw new Error('Invalid release notes');
      const template=document.createElement('template');template.innerHTML=data.html;
      for(const article of template.content.children){
        if(article.tagName!=='ARTICLE'||!/^version-\d+-\d+-\d+$/.test(article.id)||!Number.isInteger(Number(article.dataset.releaseIndex)))throw new Error('Invalid release entry');
      }
      for(const article of [...template.content.children]){
        if(document.getElementById(article.id))continue;
        const index=Number(article.dataset.releaseIndex);
        const after=[...releases.children].find(item=>Number(item.dataset.releaseIndex)>index);
        releases.insertBefore(article,after||null);
      }
      return data;
    }finally{clearTimeout(timer)}
  };
  const load=async()=>{
    if(loading||next>=total)return;
    loading=true;failed=false;status.textContent='Loading older versions…';update();
    try{const data=await request('offset='+next);if(data.next<=next&&next<data.total)throw new Error('Release loading made no progress');next=data.next;total=data.total;status.textContent=next>=total?'All versions loaded.':''}
    catch(_error){failed=true;status.textContent='Could not load older versions. Tap to retry.'}
    finally{loading=false;update()}
  };
  const jump=async()=>{
    const id=picker.value,generation=++jumpGeneration;
    if(![...picker.options].some(option=>option.value===id))return;
    go.disabled=true;
    try{
      let target=document.getElementById(id);
      if(!target){status.textContent='Loading selected version…';await request('version='+encodeURIComponent(id));target=document.getElementById(id)}
      if(generation!==jumpGeneration)return;
      if(!target)throw new Error('Missing release');
      status.textContent='';history.replaceState(null,'','#'+id);
      target.scrollIntoView({block:'start',behavior:(window.RustDLTheme?.isMotionReduced()||matchMedia('(prefers-reduced-motion: reduce)').matches)?'auto':'smooth'});
    }catch(_error){if(generation===jumpGeneration)status.textContent='Could not load that version. Tap Go to retry.'}
    finally{if(generation===jumpGeneration)go.disabled=false}
  };
  go.addEventListener('click',jump);picker.addEventListener('change',jump);more.addEventListener('click',load);
  if('IntersectionObserver' in window)new IntersectionObserver(entries=>{if(entries.some(entry=>entry.isIntersecting)&&!failed)load()},{rootMargin:'200px'}).observe(sentinel);
  const fromHash=()=>{const id=location.hash.slice(1);if([...picker.options].some(option=>option.value===id)){picker.value=id;jump()}};
  addEventListener('hashchange',fromHash);update();fromHash();
})();
