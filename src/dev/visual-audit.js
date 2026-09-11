/* Dev-only geometry audit. Run in a laid-out browser, never infer layout from jsdom. */
(()=>{
  'use strict';
  const tolerance=1.5,interactive='button,input:not([type=hidden]),select,textarea,a[href],[role=button]';
  const rect=r=>({left:r.left,top:r.top,right:r.right,bottom:r.bottom,width:r.width,height:r.height});
  const selector=el=>{
    const parts=[];
    for(let node=el;node&&node!==document.body&&parts.length<5;node=node.parentElement){
      if(node.id){parts.unshift('#'+CSS.escape(node.id));break}
      const siblings=node.parentElement?[...node.parentElement.children].filter(n=>n.tagName===node.tagName):[];
      parts.unshift(node.tagName.toLowerCase()+(siblings.length>1?':nth-of-type('+(siblings.indexOf(node)+1)+')':''));
    }
    return parts.join(' > ')||'body';
  };
  const shown=el=>{
    if(!el.getClientRects().length)return false;
    for(let node=el;node;node=node.parentElement){const s=getComputedStyle(node);if(s.display==='none'||s.visibility==='hidden'||s.visibility==='collapse'||Number(s.opacity)===0)return false}
    return true;
  };
  const clientRect=el=>{
    const r=el.getBoundingClientRect(),sx=el.offsetWidth?r.width/el.offsetWidth:1,sy=el.offsetHeight?r.height/el.offsetHeight:1;
    return {left:r.left+el.clientLeft*sx,top:r.top+el.clientTop*sy,right:r.left+(el.clientLeft+el.clientWidth)*sx,bottom:r.top+(el.clientTop+el.clientHeight)*sy};
  };
  const outside=(r,b,axis)=>axis==='x'?r.left<b.left-tolerance||r.right>b.right+tolerance:r.top<b.top-tolerance||r.bottom>b.bottom+tolerance;
  const intentional=el=>{
    for(let node=el;node&&node!==document.body;node=node.parentElement){
      const s=getComputedStyle(node);
      if(node.hasAttribute('data-visual-allow-truncation'))return 'annotated: '+node.getAttribute('data-visual-allow-truncation');
      if(s.textOverflow==='ellipsis'||parseInt(s.webkitLineClamp,10)>0)return 'CSS ellipsis/line clamp';
    }
    return null;
  };
  window.RustDLVisualAudit={merge(samples){
    const issues=[],keys=new Set(),counts={error:0,warning:0,allowed:0};let total=0;
    for(const sample of samples){
      for(const level of Object.keys(counts))counts[level]+=sample.counts[level];
      total+=sample.issues.length+sample.omitted;
      for(const issue of sample.issues){const key=issue.type+'|'+issue.selector+'|'+(issue.axis||'')+'|'+(issue.ancestor||'');if(keys.has(key))continue;keys.add(key);if(issues.length<60)issues.push({...issue,scroll:sample.scroll})}
    }
    const missingCoverage=samples.flatMap(sample=>sample.missingCoverage||[]);
    return {samples:samples.length,counts,issues,missingCoverage,omitted:Math.max(0,total-issues.length),pass:samples.every(sample=>sample.pass)};
  },run(options={}){
    const issues=[],keys=new Set(),counts={error:0,warning:0,allowed:0};let checked=0;
    const limit=options.limit??60;
    const add=(type,el,detail={},severity='error')=>{
      const target=selector(el),key=type+'|'+target+'|'+(detail.axis||'')+'|'+(detail.ancestor||'');
      if(keys.has(key))return;keys.add(key);counts[severity]++;
      if(issues.length<limit)issues.push({type,severity,selector:target,label:(el.getAttribute('aria-label')||el.textContent||el.getAttribute('placeholder')||'').trim().replace(/\s+/g,' ').slice(0,100),rect:rect(el.getBoundingClientRect()),...detail});
    };
    // A scrollable ancestor only makes its own clipping reachable. Continue checking
    // outer ancestors against the visible scrollport so an outer hidden box still fails.
    const clipped=(el,bounds,text=false)=>{
      let r={...bounds};
      for(let parent=text?el:el.parentElement;parent&&parent!==document.documentElement;parent=parent.parentElement){
        const s=getComputedStyle(parent),b=clientRect(parent);
        for(const axis of ['x','y']){
          const overflow=axis==='x'?s.overflowX:s.overflowY;
          if(!['hidden','clip','auto','scroll'].includes(overflow)||!outside(r,b,axis))continue;
          const scrollable=['auto','scroll'].includes(overflow);
          const size=axis==='x'?r.right-r.left:r.bottom-r.top,available=axis==='x'?b.right-b.left:b.bottom-b.top;
          if(!scrollable||size>available+tolerance){
            const reason=text?intentional(el):null;
            add(text?'text-clipped':'element-clipped',el,{axis,ancestor:selector(parent),...(reason?{reason}:{})},reason?(reason.startsWith('annotated:')?'allowed':'warning'):'error');
          }
          if(axis==='x'){r.left=Math.max(r.left,b.left);r.right=Math.min(r.right,b.right)}else{r.top=Math.max(r.top,b.top);r.bottom=Math.min(r.bottom,b.bottom)}
        }
      }
      if(!text&&(r.left< -tolerance||r.right>innerWidth+tolerance))add('outside-viewport',el,{axis:'x'});
    };
    const all=[...document.body.querySelectorAll('*')];
    for(const el of all){
      if(el.closest('[data-visual-audit-ignore]')||['SCRIPT','STYLE','OPTION','NOSCRIPT','SVG','PATH'].includes(el.tagName)||!shown(el))continue;
      checked++;
      const r=el.getBoundingClientRect(),s=getComputedStyle(el),control=el.matches(interactive);
      if(control||el.matches('img,video,canvas,iframe'))clipped(el,rect(r));
      if(el.matches('img')&&el.complete&&el.naturalWidth===0)add('broken-image',el);
      const textNodes=[...el.childNodes].filter(n=>n.nodeType===3&&n.textContent.trim());
      if(textNodes.length){
        for(const node of textNodes){
          const range=document.createRange();range.selectNodeContents(node);
          for(const box of range.getClientRects())if(box.width>0&&box.height>0)clipped(el,rect(box),true);
          range.detach();
        }
      }
      // Native inputs/selects do not expose painted text as DOM ranges. Their
      // internal scrolling/selection is intentional and is not called truncation.
      if(textNodes.length&&['hidden','clip'].includes(s.overflowX)&&el.scrollWidth>el.clientWidth+tolerance){
        const reason=intentional(el);add('text-truncated',el,{axis:'x',hiddenPixels:el.scrollWidth-el.clientWidth,...(reason?{reason}:{})},reason?(reason.startsWith('annotated:')?'allowed':'warning'):'error');
      }
      let insideClip=true;
      for(let parent=el.parentElement;parent&&parent!==document.documentElement;parent=parent.parentElement){
        const ps=getComputedStyle(parent),b=clientRect(parent);
        if((['hidden','clip','auto','scroll'].includes(ps.overflowX)&&outside(r,b,'x'))||(['hidden','clip','auto','scroll'].includes(ps.overflowY)&&outside(r,b,'y'))){insideClip=false;break}
      }
      if(control&&insideClip&&!el.disabled&&s.pointerEvents!=='none'&&r.width>0&&r.height>0&&r.left>=0&&r.right<=innerWidth&&r.top>=0&&r.bottom<=innerHeight&&typeof document.elementFromPoint==='function'){
        const points=[[.5,.5],[.2,.2],[.8,.2],[.2,.8],[.8,.8]];
        const hits=points.map(([x,y])=>document.elementFromPoint(r.left+r.width*x,r.top+r.height*y));
        if(hits.every(hit=>hit&&hit!==el&&!el.contains(hit)&&!hit.contains(el)))add('control-obscured',el,{covering:selector(hits[0])});
      }
    }
    const overflowPixels=Math.max(0,document.documentElement.scrollWidth-innerWidth);
    if(overflowPixels>tolerance)add('document-overflow',document.body,{axis:'x',overflowPixels});
    const missingCoverage=[...document.querySelectorAll('[data-visual-audit-required]')]
      .filter(el=>shown(el)&&!el.closest('[data-visual-audit-ignore]'))
      .filter(el=>!issues.some(issue=>issue.selector===selector(el)))
      .map(el=>({selector:selector(el),label:(el.getAttribute('aria-label')||el.textContent||'').trim().replace(/\s+/g,' ').slice(0,100),reason:el.getAttribute('data-visual-audit-required')||'Required singular element was not exercised by a detector'}));
    return {checked,viewport:{width:innerWidth,height:innerHeight,dpr:devicePixelRatio},scroll:{x:scrollX,y:scrollY},overflowPixels,counts,issues,missingCoverage,omitted:Math.max(0,keys.size-issues.length),pass:counts.error===0};
  }};
})();
