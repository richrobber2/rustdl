// Geometry logic regression tests only. Actual layout is tested by the dev WebView.
const {test}=require('node:test');
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {JSDOM}=require('jsdom');
const script=fs.readFileSync(path.join(__dirname,'visual-audit.js'),'utf8');
function scene(markup){
 const dom=new JSDOM('<body>'+markup+'</body>',{runScripts:'outside-only',pretendToBeVisual:true}),w=dom.window;
 w.CSS={escape:s=>s};Object.defineProperty(w,'innerWidth',{value:320});Object.defineProperty(w,'innerHeight',{value:600});
 Object.defineProperty(w.document.documentElement,'scrollWidth',{value:320});
 const box=(el,x,y,width,height,extra={})=>{
  el.getBoundingClientRect=()=>({left:x,top:y,right:x+width,bottom:y+height,width,height});el.getClientRects=()=>[el.getBoundingClientRect()];
  for(const [key,value] of Object.entries({clientWidth:width,clientHeight:height,offsetWidth:width,offsetHeight:height,clientLeft:0,clientTop:0,...extra}))Object.defineProperty(el,key,{value,configurable:true});
 };
 box(w.document.body,0,0,320,600);
 for(const el of w.document.body.querySelectorAll('*'))box(el,0,0,100,40);
 w.Range.prototype.getClientRects=function(){return this.startContainer.parentElement.textBoxes||[]};
 w.eval(script);
 return {w,box,run:()=>w.RustDLVisualAudit.run(),close:()=>dom.window.close()};
}
test('offscreen control fails; normal horizontal scrolling remains reachable',()=>{
 const s=scene('<div id="strip"><button id="control"></button></div>'),{w,box}=s,p=w.document.querySelector('#strip'),b=w.document.querySelector('button');
 box(p,0,0,200,40,{scrollWidth:600});box(b,280,0,80,40);
 assert.ok(s.run().issues.some(i=>i.type==='outside-viewport'));
 p.style.overflowX='auto';assert.equal(s.run().pass,true);
 box(b,280,0,300,40);assert.ok(s.run().issues.some(i=>i.type==='element-clipped'));s.close();
});
test('nested scrolling does not hide clipping by an outer container',()=>{
 const s=scene('<div id="outer"><div id="strip"><button></button></div></div>'),{w,box}=s;
 const outer=w.document.querySelector('#outer'),strip=w.document.querySelector('#strip');outer.style.overflowX='hidden';strip.style.overflowX='auto';
 box(outer,0,0,80,40);box(strip,0,0,200,40,{scrollWidth:600});box(w.document.querySelector('button'),100,0,80,40);
 assert.ok(s.run().issues.some(i=>i.ancestor==='#outer'));s.close();
});
test('hard text clipping fails; ellipsis warns; explicit truncation annotations are recorded',()=>{
 const s=scene('<p id="title">A long title</p>'),p=s.w.document.querySelector('p');p.style.overflowX='hidden';s.box(p,0,0,100,20,{scrollWidth:200});
 p.textBoxes=[{left:0,top:0,right:200,bottom:20,width:200,height:20}];
 assert.equal(s.run().pass,false);assert.ok(s.run().issues.some(i=>i.type==='text-clipped'));
 p.style.textOverflow='ellipsis';assert.equal(s.run().pass,true);assert.ok(s.run().counts.warning>0);
 p.setAttribute('data-visual-allow-truncation','Title is available on detail page');assert.ok(s.run().counts.allowed>0);s.close();
});
test('vertical clipping is detected and hidden ancestors are excluded',()=>{
 const s=scene('<div id="outer"><button>Save</button></div>'),p=s.w.document.querySelector('#outer'),b=s.w.document.querySelector('button');p.style.overflowY='hidden';s.box(p,0,0,100,20);s.box(b,0,0,100,40);
 assert.ok(s.run().issues.some(i=>i.axis==='y'));p.style.display='none';assert.equal(s.run().pass,true);s.close();
});
test('obscured controls and broken images fail',()=>{
 const s=scene('<button>Save</button><div id="cover"></div><img>');s.w.document.elementFromPoint=()=>s.w.document.querySelector('#cover');
 const report=s.run();assert.ok(report.issues.some(i=>i.type==='control-obscured'));assert.ok(report.issues.some(i=>i.type==='broken-image'));s.close();
});
test('report limits do not hide the total number of failures',()=>{
 const s=scene('<button></button><button></button>');for(const b of s.w.document.querySelectorAll('button'))s.box(b,310,0,50,40);
 const r=s.w.RustDLVisualAudit.run({limit:1});assert.equal(r.issues.length,1);assert.ok(r.omitted>0);assert.equal(r.pass,false);s.close();
});

test('required singular elements produce review coverage gaps',()=>{
 const s=scene('<button id="one" data-visual-audit-required="Check its icon and label"></button>');
 const r=s.run();assert.equal(r.pass,true);assert.equal(r.missingCoverage.length,1);assert.equal(r.missingCoverage[0].selector,'#one');s.close();
});
