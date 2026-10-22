const {test}=require('node:test');const assert=require('node:assert/strict');
const {JSDOM}=require('jsdom');const fs=require('node:fs');const path=require('node:path');
const root=path.join(__dirname,'../..');const script=require('./read-script.cjs');
function fixture(name,cards){
 const html=fs.readFileSync(path.join(root,'assets/html',name+'.html'),'utf8').replace(/<style>[\s\S]*?<\/style>/,'').replace('{cards}',cards);
 const dom=new JSDOM(html,{url:'http://localhost/',runScripts:'outside-only',pretendToBeVisual:true});dom.window.scrollTo=()=>{};
 dom.window.eval(script('list-pagination.js')+'\n'+script(name==='playlist'?'playlist.js':'bulk-quality.js'));return dom;
}
test('5000 playlist entries retain selection across pages with at most 32 rendered rows',t=>{
 const dom=require('./playlist-fixture.cjs')(5000);t.after(()=>dom.window.close());const d=dom.window.document;
 const cards=()=>[...d.querySelectorAll('.candidate:not([hidden])')];const index=card=>card.querySelector('input').dataset.index;
 assert.equal(cards().length,32);assert.equal(d.querySelector('[data-page-status]').textContent,'1–32 of 5000');
 const first=cards()[0].querySelector('input');first.checked=true;first.dispatchEvent(new dom.window.Event('change',{bubbles:true}));
 d.querySelector('[data-page-next]').click();assert.equal(index(cards()[0]),'32');assert.equal(cards().length,32);
 d.querySelector('[data-page-previous]').click();assert.equal(cards()[0].querySelector('input').checked,true);
 const search=d.getElementById('playlist-filter');search.value='Artist B';search.dispatchEvent(new dom.window.Event('input'));assert.equal(index(cards()[0]),'250');
 d.getElementById('select-visible').click();assert.equal(d.getElementById('selected-count').textContent,'4751');
 const visited=new Set();do{cards().forEach(card=>visited.add(index(card)));const next=d.querySelector('[data-page-next]');if(next.disabled)break;next.click();}while(true);
 assert.equal(visited.size,4750);assert.ok(cards().length<=32);
 search.value='nothing matches';search.dispatchEvent(new dom.window.Event('input'));assert.equal(cards().length,0);assert.equal(d.querySelector('[data-page-next]').disabled,true);
 d.getElementById('select-all').click();assert.equal(d.getElementById('selected-count').textContent,'5000');assert.equal(d.querySelectorAll('input[name="pick"]').length,0);
 const form=d.getElementById('playlist-form');form.dispatchEvent(new dom.window.Event('submit',{cancelable:true}));const picks=new dom.window.FormData(form).getAll('pick');assert.equal(picks.length,5000);assert.equal(new Set(picks).size,5000);
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

test('playlist paging reuses rows and sticky docks without moving scroll or focus',t=>{
 const dom=require('./playlist-fixture.cjs')(65);t.after(()=>dom.window.close());const w=dom.window,d=w.document;
 let scrolls=0;w.scrollTo=()=>{scrolls++;};
 const rows=[...d.querySelectorAll('.candidate')],next=d.querySelector('.playlist-tools [data-page-next]');
 assert.ok(d.querySelector('.selection-bar [data-page-previous]'));
 next.focus();next.click();assert.equal(d.activeElement,next);assert.equal(scrolls,0);
 assert.deepEqual([...d.querySelectorAll('.candidate')],rows);assert.equal(rows[0].querySelector('input').dataset.index,'32');
 next.click();assert.deepEqual([...d.querySelectorAll('.candidate')],rows);
 assert.equal(d.querySelectorAll('.candidate:not([hidden])').length,1);
 assert.equal(d.getElementById('playlist-items').style.getPropertyValue('--playlist-rows'),'32');
 assert.equal(rows[1].querySelector('input').disabled,true);assert.equal(rows[1].querySelector('input').dataset.index,undefined);
 d.getElementById('select-all').click();assert.deepEqual([...d.querySelectorAll('.candidate')],rows);
 d.querySelector('.selection-bar [data-page-previous]').click();assert.equal(rows[0].querySelector('input').checked,true);assert.equal(scrolls,0);
});

test('a short final page is revealed when its rows would be above the viewport',t=>{
 const dom=require('./playlist-fixture.cjs')(33);t.after(()=>dom.window.close());const d=dom.window.document;
 const first=d.querySelector('.candidate');first.getBoundingClientRect=()=>({top:-200,bottom:-80,height:120});
 d.querySelector('.playlist-tools').getBoundingClientRect=()=>({bottom:180});
 const moves=[];dom.window.scrollBy=(x,y)=>moves.push([x,y]);
 d.querySelector('[data-page-next]').click();assert.deepEqual(moves,[[0,-392]]);
 assert.equal(first.querySelector('input').dataset.index,'32');
});
