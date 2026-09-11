// Check the clipping detector without claiming these are browser timing tests.
const assert=require('node:assert/strict');
const fs=require('node:fs');
const {JSDOM}=require('jsdom');
const script=fs.readFileSync(require('node:path').join(__dirname,'visual-smoke.js'),'utf8');
function check(scrollable,tooWide=false){
 const dom=new JSDOM('<main><div id="strip"><button>Filter</button></div></main>',{runScripts:'outside-only',pretendToBeVisual:true});
 const w=dom.window;let frame,result;w.scrollTo=()=>{};
 Object.defineProperty(w,'innerWidth',{value:320});
 Object.defineProperty(w.document.documentElement,'scrollWidth',{value:320});
 const strip=w.document.getElementById('strip');
 if(scrollable){strip.style.overflowX='auto';Object.defineProperty(strip,'scrollWidth',{value:600});Object.defineProperty(strip,'clientWidth',{value:200});}
 w.document.querySelector('button').getBoundingClientRect=()=>({left:280,right:tooWide?580:360,width:tooWide?300:80,height:40});
 w.RustDLDev={beginPhase(){},caseComplete(json){result=JSON.parse(json)}};
 w.RustDLVisualAudit={merge:samples=>samples[0],run:()=>({pass:scrollable&&!tooWide,issues:[],counts:{error:0}})};w.requestAnimationFrame=callback=>{frame=callback};w.eval(script);frame(0);frame(1001);frame(1017);dom.window.close();return result;
}
assert.equal(check(false).pass,false,'Offscreen control must fail');
assert.equal(check(true).pass,true,'Reachable horizontal scroll control must pass');
assert.equal(check(true,true).pass,false,'Control wider than its scroller must still fail');
console.log('PASS smoke runner respects audit failures');
