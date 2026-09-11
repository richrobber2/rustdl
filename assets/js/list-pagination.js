(()=>{
  globalThis.RustDLPagination={create(container,cards,size=32){
    let items=cards,page=0;
    const controls=[];
    const render=()=>{
      page=Math.max(0,Math.min(page,Math.ceil(items.length/size)-1));
      const visible=new Set(items.slice(page*size,(page+1)*size));
      cards.forEach(card=>{const hidden=!visible.has(card);if(card.hidden!==hidden)card.hidden=hidden;});
      container.dataset.filteredCount=items.length;
      container.dataset.pageSize=size;
      controls.forEach(({nav,previous,next,status})=>{
        nav.hidden=cards.length<=size;
        previous.disabled=page===0;next.disabled=(page+1)*size>=items.length;
        status.textContent=items.length?(page*size+1)+'–'+Math.min((page+1)*size,items.length)+' of '+items.length:'No matching videos';
      });
    };
    const move=offset=>{page+=offset;render();container.tabIndex=-1;container.focus({preventScroll:true});scrollTo(0,0);};
    for(const position of ['before','after']){
      const nav=document.createElement('nav');nav.className='list-pagination';nav.setAttribute('aria-label','Video pages');
      const previous=document.createElement('button'),next=document.createElement('button'),status=document.createElement('span');
      previous.type=next.type='button';previous.textContent='Previous';next.textContent='Next';
      previous.dataset.pagePrevious='';next.dataset.pageNext='';status.dataset.pageStatus='';status.setAttribute('role','status');
      previous.addEventListener('click',()=>move(-1));next.addEventListener('click',()=>move(1));
      nav.append(previous,status,next);container[position](nav);controls.push({nav,previous,next,status});
    }
    render();
    return {setItems(nextItems){items=nextItems;page=0;render();}};
  }};
})();
