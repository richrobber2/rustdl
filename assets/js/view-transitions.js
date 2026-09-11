(()=>{
  'use strict';
  const supported=typeof document.startViewTransition==='function';
  document.documentElement.dataset.viewTransitions=supported?'supported':'fallback';
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
  const selectSharedElement=link=>{
    const selected=link.querySelector('.media-thumb');
    const name=transitionName(link);
    if(!selected||!name)return;
    document.querySelectorAll('.media-thumb').forEach(thumb=>{thumb.style.viewTransitionName='none'});
    selected.style.viewTransitionName=name;
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