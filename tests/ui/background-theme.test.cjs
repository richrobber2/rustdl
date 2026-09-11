const {test}=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {JSDOM}=require('jsdom');
const root=path.resolve(__dirname,'../..');
const script=name=>fs.readFileSync(path.join(root,'assets/js',name),'utf8');
function page(initial={}) {
  let saved={reduceMotion:false,allowScreenshots:false,appearance:'dark',backgroundTheme:'space',spaceEffectEnabled:true,...initial};
  const dom=new JSDOM(fs.readFileSync(path.join(root,'assets/html/settings.html'),'utf8'),{url:'http://localhost/settings',runScripts:'outside-only'});
  const w=dom.window;
  w.matchMedia=()=>({matches:false,addEventListener(){}});
  const response=()=>JSON.stringify({ok:true,downloadFolder:'RustDL',diagnosticsRefreshSeconds:5,...saved});
  w.RustDLSettings={appearance:()=>saved.appearance,spaceEffectEnabled:()=>saved.spaceEffectEnabled,backgroundTheme:()=>saved.backgroundTheme,reduceMotionEnabled:()=>saved.reduceMotion===true,settings:response,
    setAppearance:value=>{saved.appearance=value;return true},
    save:(folder,awake,refresh,appearance,space,background,allowScreenshots,reduceMotion,mobileDownloadPolicy)=>{saved={appearance,spaceEffectEnabled:space,backgroundTheme:background,allowScreenshots,reduceMotion,mobileDownloadPolicy};return response()},
    reset:()=>{saved={appearance:'system',spaceEffectEnabled:true,backgroundTheme:'space'};return response()}};
  for(const name of ['appearance-boot.js','appearance.js','settings.js'])w.eval(script(name));
  return {dom,w,saved:()=>saved};
}
test('mobile-data policy uses native settings, saves with the form, and resets to Ask',()=>{
 const {dom,w,saved}=page({mobileDownloadPolicy:'block'});
 const policy=w.document.querySelector('#mobile-downloads');
 assert.equal(policy.value,'block');
 policy.value='allow';policy.dispatchEvent(new w.Event('change'));
 assert.equal(saved().mobileDownloadPolicy,'block');
 w.document.querySelector('#settings-form').dispatchEvent(new w.Event('submit',{cancelable:true}));
 assert.equal(saved().mobileDownloadPolicy,'allow');
 const reloaded=page(saved());assert.equal(reloaded.w.document.querySelector('#mobile-downloads').value,'allow');reloaded.dom.window.close();
 w.document.querySelector('#reset').click();assert.equal(policy.value,'ask');
 dom.window.close();
});
test('Rainy City previews without saving, persists on save, and restores defaults',()=>{
  const {dom,w,saved}=page();
  const background=w.document.querySelector('#background-theme');
  background.value='rainy-city';background.dispatchEvent(new w.Event('change'));
  assert.equal(w.document.documentElement.dataset.background,'rainy-city');
  assert.equal(saved().backgroundTheme,'space');
  assert.equal(w.document.querySelector('#space-effect').disabled,true);
  w.document.querySelector('#settings-form').dispatchEvent(new w.Event('submit',{cancelable:true}));
  assert.equal(saved().backgroundTheme,'rainy-city');
  const reloaded=page(saved());
  assert.equal(reloaded.w.document.documentElement.dataset.background,'rainy-city');
  reloaded.dom.window.close();
  w.document.querySelector('#reset').click();
  assert.equal(w.document.documentElement.dataset.background,'space');
  assert.equal(background.value,'space');
  assert.equal(w.document.querySelector('#space-effect').disabled,false);
  dom.window.close();
});
test('light/dark changes and frame updates preserve background selection',()=>{
  const {dom,w}=page({backgroundTheme:'rainy-city'});
  w.RustDLTheme.apply('light',true);
  assert.equal(w.document.documentElement.dataset.theme,'light');
  assert.equal(w.document.documentElement.dataset.background,'rainy-city');
  w.RustDLTheme.receive('dark',false,'space');
  assert.equal(w.document.documentElement.dataset.background,'space');
  w.RustDLTheme.receive('dark',true,'rainy-city');
  assert.equal(w.document.documentElement.dataset.background,'rainy-city');
  dom.window.close();
});
test('browser boot restores a saved background and rejects unknown values',()=>{
  const dom=new JSDOM('',{url:'http://localhost',runScripts:'outside-only'}),w=dom.window;
  w.matchMedia=()=>({matches:false});
  w.localStorage.setItem('rustdl:background','rainy-city');w.eval(script('appearance-boot.js'));
  assert.equal(w.document.documentElement.dataset.background,'rainy-city');
  w.localStorage.setItem('rustdl:background','unknown');w.eval(script('appearance-boot.js'));
  assert.equal(w.document.documentElement.dataset.background,'space');
  dom.window.close();
});

test('screenshots require an explicit save, persist, and reset to blocked',()=>{
  const {dom,w,saved}=page();
  const toggle=w.document.querySelector('#allow-screenshots');
  assert.equal(toggle.checked,false);
  toggle.checked=true;toggle.dispatchEvent(new w.Event('change'));
  assert.equal(saved().allowScreenshots,false);
  w.document.querySelector('#settings-form').dispatchEvent(new w.Event('submit',{cancelable:true}));
  assert.equal(saved().allowScreenshots,true);
  const reloaded=page(saved());
  assert.equal(reloaded.w.document.querySelector('#allow-screenshots').checked,true);
  reloaded.dom.window.close();
  toggle.checked=false;
  w.document.querySelector('#settings-form').dispatchEvent(new w.Event('submit',{cancelable:true}));
  assert.equal(saved().allowScreenshots,false);
  toggle.checked=true;
  w.document.querySelector('#settings-form').dispatchEvent(new w.Event('submit',{cancelable:true}));
  w.document.querySelector('#reset').click();
  assert.equal(toggle.checked,false);
  dom.window.close();
});

test('Reduce motion previews, saves across reloads, survives theme changes, and resets',()=>{
 const {dom,w,saved}=page();const root=w.document.documentElement,toggle=w.document.querySelector('#reduce-motion');
 assert.equal(toggle.checked,false);assert.equal(w.RustDLTheme.isMotionReduced(),false);
 toggle.checked=true;toggle.dispatchEvent(new w.Event('change'));
 assert.equal(w.RustDLTheme.isMotionReduced(),true);assert.equal(saved().reduceMotion,false);
 w.document.querySelector('#settings-form').dispatchEvent(new w.Event('submit',{cancelable:true}));
 assert.equal(saved().reduceMotion,true);
 const reload=page(saved());assert.equal(reload.w.document.querySelector('#reduce-motion').checked,true);assert.equal(reload.w.RustDLTheme.isMotionReduced(),true);reload.dom.window.close();
 w.RustDLTheme.apply('light',true,false,false);assert.equal(root.dataset.reduceMotion,'on');
 w.RustDLTheme.receive('dark',true,'rainy-city');assert.equal(root.dataset.reduceMotion,'on');
 w.document.querySelector('#reset').click();assert.equal(toggle.checked,false);assert.equal(w.RustDLTheme.isMotionReduced(),false);
 dom.window.close();
});
test('browser boot restores explicit reduced motion',()=>{
 const dom=new JSDOM('<html></html>',{url:'http://localhost',runScripts:'outside-only'}),w=dom.window;
 w.matchMedia=()=>({matches:false});w.localStorage.setItem('rustdl:reduce-motion','on');w.eval(script('appearance-boot.js'));
 assert.equal(w.document.documentElement.dataset.reduceMotion,'on');dom.window.close();
});
