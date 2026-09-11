(()=>{
  const form=document.getElementById('playlist-form'),cards=[...document.querySelectorAll('.candidate')],boxes=cards.map(card=>card.querySelector('input')),counter=document.getElementById('selected-count'),submit=document.getElementById('playlist-continue'),filter=document.getElementById('playlist-filter');
  const searchText=cards.map(card=>card.textContent.toLocaleLowerCase());
  const pages=globalThis.RustDLPagination?.create(document.getElementById('playlist-items'),cards);
  let filtered=cards;
  const sync=()=>{const count=boxes.filter(box=>box.checked).length;counter.textContent=count;submit.disabled=count===0;};
  const clear=()=>{boxes.forEach(box=>box.checked=false);sync();};
  filter.addEventListener('input',()=>{
    const query=filter.value.trim().toLocaleLowerCase();
    filtered=cards.filter((card,index)=>query===''||searchText[index].includes(query));
    if(pages)pages.setItems(filtered);
    else cards.forEach(card=>card.hidden=!filtered.includes(card));
  });
  document.getElementById('select-all').addEventListener('click',()=>{boxes.forEach(box=>box.checked=true);sync();});
  document.getElementById('select-visible').addEventListener('click',()=>{filtered.forEach(card=>card.querySelector('input').checked=true);sync();});
  document.getElementById('select-first').addEventListener('click',()=>{clear();boxes.slice(0,10).forEach(box=>box.checked=true);sync();});
  document.getElementById('clear-selection').addEventListener('click',clear);
  boxes.forEach(box=>box.addEventListener('change',sync));
  form.addEventListener('submit',event=>{if(submit.disabled){event.preventDefault();return;}submit.disabled=true;submit.textContent='Opening preparation…';});
  sync();
})();
