pub(crate) const QUEUE_SCRIPT: &str = r#"<script>(()=>{
  'use strict';
  const section=document.querySelector('main>section');
  if(!section)return;
  const phaseLabels={queued:'Queued',starting:'Starting',downloading:'Downloading',paused:'Paused',ready:'Ready',failed:'Needs retry',cancelled:'Cancelled'};
  const formatBytes=value=>{value=Number(value||0);const units=['B','KB','MB','GB'];let unit=0;while(value>=1024&&unit<units.length-1){value/=1024;unit++}return value.toFixed(unit?1:0)+' '+units[unit]};
  const makeLink=(label,href,danger=false)=>{const link=document.createElement('a');link.textContent=label;link.href=href;if(danger)link.className='danger';return link};
  const updateActions=(nav,job)=>{
    const watch='/watch/'+encodeURIComponent(job.filename);
    const action=name=>'/queue/action?file='+encodeURIComponent(job.filename)+'&action='+name;
    const links=[];
    if(['queued','starting','downloading'].includes(job.phase))links.push(makeLink('Play',watch),makeLink('Pause',action('pause')),makeLink('Cancel',action('cancel'),true));
    else if(['paused','failed'].includes(job.phase))links.push(makeLink('Resume',action('resume')),makeLink('Cancel',action('cancel'),true));
    else if(job.phase==='ready')links.push(makeLink('Open player',watch));
    else if(job.phase==='cancelled')links.push(makeLink('Start again',action('resume')));
    nav.replaceChildren(...links);
  };
  const updateArticle=(article,job)=>{
    article.dataset.phase=job.phase;
    article.querySelector('.phase').textContent=phaseLabels[job.phase]||job.phase;
    const downloaded=Number(job.downloaded||0),total=Number(job.total||0);
    article.querySelector('.size').textContent=total?formatBytes(downloaded)+' / '+formatBytes(total):formatBytes(downloaded);
    const progress=article.querySelector('.progress');let bar=progress.querySelector('i');if(!bar){bar=document.createElement('i');progress.append(bar)}
    bar.style.width=(total?Math.min(100,downloaded/total*100):0)+'%';
    updateActions(article.querySelector('nav'),job);
    let error=article.querySelector('.error');
    if(job.error){if(!error){error=document.createElement('p');error.className='error';article.append(error)}error.textContent=job.error}
    else error?.remove();
  };
  const render=state=>{
    const jobs=state.jobs||[],articles=[...section.querySelectorAll('article[data-filename]')];
    const byName=new Map(jobs.map(job=>[job.filename,job]));
    if(jobs.length!==articles.length||articles.some(article=>!byName.has(article.dataset.filename))){location.reload();return}
    articles.forEach(article=>updateArticle(article,byName.get(article.dataset.filename)));
  };
  let pending=false;
  const refresh=()=>{if(pending)return;pending=true;fetch('/__app/state.json',{cache:'no-store'}).then(response=>response.ok?response.json():Promise.reject()).then(render).catch(()=>{}).finally(()=>pending=false)};
  addEventListener('rustdl:state',event=>{if(['queue','sync'].includes(event.detail?.type))refresh()});
  document.addEventListener('visibilitychange',()=>{if(!document.hidden)refresh()});
  refresh();setInterval(()=>{if(!document.hidden)refresh()},15000);
})();</script>"#;
