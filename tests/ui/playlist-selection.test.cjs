const {test}=require('node:test');const assert=require('node:assert/strict');const fixture=require('./playlist-fixture.cjs');
test('playlist bulk selection covers filtered and off-page items without creating their DOM',t=>{
 const dom=fixture(500);t.after(()=>dom.window.close());const d=dom.window.document;
 const click=id=>d.getElementById(id).click();const count=()=>Number(d.getElementById('selected-count').textContent);
 const search=d.getElementById('playlist-filter');search.value='Artist A';search.dispatchEvent(new dom.window.Event('input'));
 click('select-visible');assert.equal(count(),250);click('select-all');assert.equal(count(),500);
 assert.equal(d.querySelectorAll('.candidate').length,32);assert.equal(d.querySelectorAll('input[name="pick"]').length,0);
 click('clear-selection');assert.equal(count(),0);assert.equal(d.getElementById('playlist-continue').disabled,true);
 click('select-first');assert.equal(count(),10);click('select-all');
 const form=d.getElementById('playlist-form');assert.equal(form.dispatchEvent(new dom.window.Event('submit',{cancelable:true})),true);
 const picks=new dom.window.FormData(form).getAll('pick');assert.equal(picks.length,500);assert.equal(new Set(picks).size,500);assert.ok(picks.includes('synthetic-token:499'));
 assert.equal(d.getElementById('playlist-continue').textContent,'Opening preparation…');
});
test('playlist labels remain text and the parsed JSON node is released',t=>{
 const payload='<img src=x onerror="throw 1"> & </script><script>throw 2</script>';
 const dom=fixture(2,{title:payload});t.after(()=>dom.window.close());const d=dom.window.document;
 assert.equal(d.getElementById('playlist-data'),null);assert.equal(d.querySelectorAll('.candidate img,.candidate script').length,0);assert.ok(d.querySelector('.candidate strong').textContent.includes(payload));
 const form=d.getElementById('playlist-form');assert.equal(form.dispatchEvent(new dom.window.Event('submit',{cancelable:true})),false);
});
