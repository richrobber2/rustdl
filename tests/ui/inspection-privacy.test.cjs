const {test}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {JSDOM}=require('jsdom');
const root=path.resolve(__dirname,'../..');
const boot=fs.readFileSync(path.join(root,'assets/js/appearance-boot.js'),'utf8');
const css=fs.readFileSync(path.join(root,'assets/css/appearance.css'),'utf8');
function page(url,enabled=true) {
  const dom=new JSDOM(`<style>${css}</style><main><span class="media-title">Synthetic title</span><span class="media-file">synthetic.mp4</span><div class="media-thumb"><img></div><video></video><button>Pause</button><span class="size">10 MB</span></main>`,{url,runScripts:'outside-only'});
  const w=dom.window;
  w.matchMedia=()=>({matches:false});
  w.RustDLSettings={screenshotRedactionEnabled:()=>enabled};
  w.eval(boot);
  return dom;
}
test('private gallery masks media and newly inserted cards while keeping controls and sizes visible',()=>{
  const dom=page('http://localhost/'),w=dom.window;
  for(const selector of ['.media-title','.media-file','img','video'])assert.equal(w.getComputedStyle(w.document.querySelector(selector)).visibility,'hidden',selector);
  for(const selector of ['button','.size'])assert.equal(w.getComputedStyle(w.document.querySelector(selector)).visibility,'visible',selector);
  const title=w.document.createElement('span');title.className='media-title';title.textContent='Synthetic next page';w.document.body.append(title);
  assert.equal(w.getComputedStyle(title).visibility,'hidden');dom.window.close();
});
test('private player hides frames and filenames and preserves app controls',()=>{
  const dom=page('http://localhost/watch/synthetic.mp4'),w=dom.window;
  assert.equal(w.document.documentElement.dataset.privacyPage,'player');
  assert.equal(w.getComputedStyle(w.document.querySelector('video')).visibility,'hidden');
  assert.equal(w.getComputedStyle(w.document.querySelector('button')).visibility,'visible');dom.window.close();
});
test('unaudited pages are concealed and normal viewing restores media',()=>{
  const protectedPage=page('http://localhost/storage/confirm');
  assert.equal(protectedPage.window.getComputedStyle(protectedPage.window.document.querySelector('button')).visibility,'hidden');protectedPage.window.close();
  const normal=page('http://localhost/',false);
  assert.equal(normal.window.getComputedStyle(normal.window.document.querySelector('.media-title')).visibility,'visible');normal.window.close();
});
test('release history remains readable with inspection privacy enabled',()=>{
  const dom=page('http://localhost/changelog'),w=dom.window;
  const note=w.document.createElement('p');note.textContent='Synthetic release note';w.document.body.append(note);
  assert.equal(w.document.documentElement.dataset.privacyPage,'changelog');
  assert.equal(w.getComputedStyle(note).visibility,'visible');
  assert.equal(w.getComputedStyle(w.document.querySelector('img')).visibility,'hidden');
  dom.window.close();
});
