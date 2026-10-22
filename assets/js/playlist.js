(()=>{
  const form=document.getElementById('playlist-form'),container=document.getElementById('playlist-items'),data=document.getElementById('playlist-data'),counter=document.getElementById('selected-count'),submit=document.getElementById('playlist-continue'),filter=document.getElementById('playlist-filter');
  const entries=JSON.parse(data.textContent),token=container.dataset.token,selected=new Set(),size=32,controls=[];
  data.remove();
  let filtered=entries.map((_,index)=>index),page=0;
  const sync=()=>{counter.textContent=selected.size;submit.disabled=selected.size===0;};
  // Keep one bounded pool of row nodes; page changes only replace their content.
  const rows=Array.from({length:Math.min(size,entries.length)},()=>{
    const card=document.createElement('label'),box=document.createElement('input'),check=document.createElement('span'),copy=document.createElement('span'),title=document.createElement('strong'),position=document.createElement('i'),text=document.createTextNode(''),author=document.createElement('span'),filename=document.createElement('code');
    card.className='candidate';box.type='checkbox';check.className='check';check.textContent='✓';copy.className='candidate-copy';
    title.append(position,text);copy.append(title,author,filename);card.append(box,check,copy);container.append(card);
    return {card,box,position,text,author,filename};
  });
  const render=()=>{
    page=Math.max(0,Math.min(page,Math.ceil(filtered.length/size)-1));
    rows.forEach(({card,box,position,text,author,filename},slot)=>{
      const index=filtered[page*size+slot],entry=entries[index];
      card.hidden=!entry;box.disabled=!entry;
      if(!entry){delete box.dataset.index;box.checked=false;return;}
      box.checked=selected.has(index);
      if(box.dataset.index!==String(index)){
        box.dataset.index=index;position.textContent='#'+(index+1)+' ';text.data=entry.title;author.textContent=entry.author;filename.textContent='youtube-'+entry.video_id+'.mp4';
      }
    });
    // Reserve a full page even on the final page, so the dock and scroll stay put.
    container.style.setProperty('--playlist-rows',Math.max(1,Math.min(size,filtered.length)));
    container.dataset.filteredCount=filtered.length;container.dataset.pageSize=size;
    controls.forEach(({nav,previous,next,status})=>{
      nav.hidden=entries.length<=size;previous.disabled=page===0;next.disabled=(page+1)*size>=filtered.length;
      status.textContent=filtered.length?(page*size+1)+'–'+Math.min((page+1)*size,filtered.length)+' of '+filtered.length:'No matching videos';
    });
    sync();
  };
  for(const host of [document.querySelector('.playlist-tools'),document.querySelector('.selection-bar')]){
    const nav=document.createElement('nav'),previous=document.createElement('button'),next=document.createElement('button'),status=document.createElement('span');
    nav.className='list-pagination';nav.setAttribute('aria-label','Video pages');previous.type=next.type='button';previous.textContent='Previous';next.textContent='Next';previous.dataset.pagePrevious='';next.dataset.pageNext='';status.dataset.pageStatus='';status.setAttribute('role','status');
    const move=offset=>{
      page+=offset;render();
      // A short final page can end above a deeply scrolled viewport. Reveal its
      // rows only in that case; ordinary page changes leave scroll untouched.
      const visible=rows.filter(({card})=>!card.hidden);
      if(visible.length){
        const last=visible[visible.length-1].card.getBoundingClientRect(),top=document.querySelector('.playlist-tools').getBoundingClientRect().bottom;
        if(last.height>0&&last.bottom<=top)window.scrollBy(0,visible[0].card.getBoundingClientRect().top-top-12);
      }
    };
    previous.addEventListener('click',()=>move(-1));next.addEventListener('click',()=>move(1));nav.append(previous,status,next);host.append(nav);controls.push({nav,previous,next,status});
  }
  container.addEventListener('change',event=>{
    const box=event.target;if(!box.matches('input[type="checkbox"]')||box.disabled)return;
    const index=Number(box.dataset.index);if(box.checked)selected.add(index);else selected.delete(index);sync();
  });
  filter.addEventListener('input',()=>{
    const query=filter.value.trim().toLocaleLowerCase();filtered=[];
    entries.forEach((entry,index)=>{if(query===''||(entry.title+'\n'+entry.author+'\n'+entry.video_id).toLocaleLowerCase().includes(query))filtered.push(index);});page=0;render();
  });
  const choose=indices=>{for(const index of indices)selected.add(index);render();};
  document.getElementById('select-all').addEventListener('click',()=>choose(entries.keys()));
  document.getElementById('select-visible').addEventListener('click',()=>choose(filtered));
  document.getElementById('select-first').addEventListener('click',()=>{selected.clear();choose(Array.from({length:Math.min(10,entries.length)},(_,index)=>index));});
  document.getElementById('clear-selection').addEventListener('click',()=>{selected.clear();render();});
  form.addEventListener('submit',event=>{
    if(submit.disabled){event.preventDefault();return;}
    // Only materialize selected form fields when leaving the preview, not all
    // playlist cards while browsing. Keep the existing token:index protocol.
    const picks=document.createDocumentFragment();
    for(const index of selected){const input=document.createElement('input');input.type='hidden';input.name='pick';input.value=token+':'+index;picks.append(input);}
    form.querySelectorAll('input[name="pick"]').forEach(input=>input.remove());form.append(picks);
    submit.disabled=true;submit.textContent='Opening preparation…';
  });
  render();
})();
