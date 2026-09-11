const {test}=require('node:test');const assert=require('node:assert/strict');
const {JSDOM}=require('jsdom');const fs=require('node:fs');const path=require('node:path');
const root=path.join(__dirname,'../..');const script=name=>fs.readFileSync(path.join(root,'assets/js',name),'utf8');
function fixture(name,cards){
 const html=fs.readFileSync(path.join(root,'assets/html',name+'.html'),'utf8').replace(/<style>[\s\S]*?<\/style>/,'').replace('{cards}',cards);
 const dom=new JSDOM(html,{url:'http://localhost/',runScripts:'outside-only',pretendToBeVisual:true});dom.window.scrollTo=()=>{};
 dom.window.eval(script('list-pagination.js')+'\n'+script(name==='playlist'?'playlist.js':'bulk-quality.js'));return dom;
}
test('500 playlist choices remain selectable and submittable across 32-item pages and filters',t=>{
 const cards=Array.from({length:500},(_,i)=>`<label class="candidate"><input name="pick" type="checkbox" value="${i}"><span>Artist ${i<250?'A':'B'} video ${i}</span></label>`).join('');
 const dom=fixture('playlist',cards);t.after(()=>dom.window.close());const d=dom.window.document;
 const boxes=[...d.querySelectorAll('.candidate input')];const visible=()=>[...d.querySelectorAll('.candidate:not([hidden])')];
 assert.equal(visible().length,32);assert.equal(d.querySelector('[data-page-status]').textContent,'1–32 of 500');
 d.getElementById('select-all').click();assert.equal(new dom.window.FormData(d.getElementById('playlist-form')).getAll('pick').length,500);
 d.querySelector('[data-page-next]').click();assert.equal(visible()[0].querySelector('input').value,'32');assert.equal(boxes.every(box=>box.checked),true);
 d.getElementById('clear-selection').click();const search=d.getElementById('playlist-filter');search.value='Artist B';search.dispatchEvent(new dom.window.Event('input'));
 assert.equal(visible()[0].querySelector('input').value,'250');assert.equal(visible().length,32);
 d.getElementById('select-visible').click();assert.equal(boxes.filter(box=>box.checked).length,250);
 const visited=new Set();do{visible().forEach(card=>visited.add(card.querySelector('input').value));const next=d.querySelector('[data-page-next]');if(next.disabled)break;next.click();}while(true);
 assert.equal(visited.size,250);assert.ok(visible().length<=32);
 search.value='nothing matches';search.dispatchEvent(new dom.window.Event('input'));assert.equal(visible().length,0);assert.equal(d.querySelector('[data-page-next]').disabled,true);
 d.getElementById('select-all').click();assert.equal(boxes.filter(box=>box.checked).length,500);
});
test('bulk formats update hidden pages without losing individual choices',t=>{
 const cards=Array.from({length:500},(_,i)=>`<article class="quality-card"><select name="pick"><option value="${i}:0" data-kind="video" data-height="1080">1080p</option><option value="${i}:1" data-kind="video" data-height="720">720p</option><option value="${i}:2" data-kind="audio" data-height="0">Audio</option></select></article>`).join('');
 const dom=fixture('quality',cards);t.after(()=>dom.window.close());const d=dom.window.document,selects=[...d.querySelectorAll('.quality-card select')];
 assert.equal(d.querySelectorAll('.quality-card:not([hidden])').length,32);
 const preset=d.getElementById('bulk-format');preset.value='720';preset.dispatchEvent(new dom.window.Event('change'));
 assert.ok(selects.every((select,i)=>select.value===i+':1'));
 while(!d.querySelector('[data-page-next]').disabled)d.querySelector('[data-page-next]').click();
 assert.equal(d.querySelector('[data-page-status]').textContent,'481–500 of 500');
 selects[499].value='499:2';d.querySelector('[data-page-previous]').click();assert.equal(selects[499].value,'499:2');
 const values=new dom.window.FormData(d.querySelector('form')).getAll('pick');assert.equal(values.length,500);assert.equal(values[499],'499:2');
});
