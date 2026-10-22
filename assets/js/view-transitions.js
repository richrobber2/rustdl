(()=>{
  'use strict';
  const root=document.documentElement;
  if(root.dataset.navigationReady==='true')return;
  root.dataset.navigationReady='true';
  const supported=typeof document.startViewTransition==='function';
  document.documentElement.dataset.viewTransitions=supported?'supported':'fallback';
  let pending=null,timer=null,indicator=null;
  const resetNavigation=()=>{
    if(timer!==null){clearTimeout(timer);timer=null;}
    delete root.dataset.navigationPending;
    if(pending){delete pending.dataset.navigationPending;pending=null;}
    if(indicator)indicator.hidden=true;
  };
  const navigationLink=event=>{
    if(event.defaultPrevented||event.button!==0||event.metaKey||event.ctrlKey||event.shiftKey||event.altKey)return null;
    const link=event.target instanceof Element?event.target.closest('a[href]'):null;
    if(!link||link.hasAttribute('download')||(link.target&&link.target.toLowerCase()!=='_self'))return null;
    try{
      const url=new URL(link.href,location.href);
      if(url.origin!==location.origin||(url.pathname===location.pathname&&url.search===location.search))return null;
    }catch(_error){return null;}
    return link;
  };
  addEventListener('click',event=>{
    const link=navigationLink(event);if(!link)return;
    resetNavigation();pending=link;root.dataset.navigationPending='on';link.dataset.navigationPending='on';
    if(!indicator){indicator=document.createElement('div');indicator.className='rustdl-navigation-progress';indicator.setAttribute('aria-hidden','true');document.body.append(indicator);}
    indicator.hidden=false;
    // No interception, speculative requests or delay before normal navigation.
    timer=setTimeout(resetNavigation,10000);
  });
  addEventListener('pagehide',resetNavigation);
  addEventListener('pageshow',resetNavigation);
  const respectMotion=event=>{
    resetNavigation();
    if(root.dataset.reduceMotion==='on'||globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches)event.viewTransition?.skipTransition();
  };
  addEventListener('pageswap',respectMotion);
  addEventListener('pagereveal',respectMotion);
  if(!supported)return;

  const transitionName=link=>{
    const thumb=link.querySelector('.media-thumb');
    if(!thumb)return null;
    const declared=thumb.dataset.viewTransitionName||thumb.style.viewTransitionName;
    if(declared&&declared!=='none')return declared;
    let filename='';
    try{filename=decodeURIComponent(new URL(link.href,location.href).pathname.slice(7))}catch(_error){return null}
    const stem=filename.replace(/\.(?:mp4|m4a)$/,'');
    return 'video-'+stem.replace(/[^A-Za-z0-9-]/g,'-');
  };
  let lastShared=null;
  const selectSharedElement=link=>{
    const selected=link.querySelector('.media-thumb');
    const name=transitionName(link);
    if(!selected||!name||lastShared?.deref()===selected&&selected.style.viewTransitionName===name)return;
    document.querySelectorAll('.media-thumb').forEach(thumb=>{thumb.style.viewTransitionName='none'});
    // Remember the selection without retaining a card removed by gallery refresh.
    selected.style.viewTransitionName=name;lastShared=typeof WeakRef==='function'?new WeakRef(selected):null;
  };
  const mediaLink=target=>target instanceof Element?target.closest('a.media-card[href^="/watch/"]'):null;
  addEventListener('pointerdown',event=>{
    if(event.button!==0)return;
    const link=mediaLink(event.target);if(link)selectSharedElement(link);
  },{passive:true});
  addEventListener('click',event=>{
    if(event.button!==0||event.metaKey||event.ctrlKey||event.shiftKey||event.altKey)return;
    const link=mediaLink(event.target);if(link)selectSharedElement(link);
  });
})();