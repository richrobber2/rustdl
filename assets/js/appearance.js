(()=>{
  'use strict';
  const root=document.documentElement,bridge=window.RustDLSettings||null,media=matchMedia('(prefers-color-scheme:light)'),reduced=matchMedia('(prefers-reduced-motion:reduce)');
  const isMotionReduced=()=>reduced.matches||root.dataset.reduceMotion==='on';
  const valid=mode=>['system','light','dark'].includes(mode)?mode:'system';
  const validBackground=value=>value==='rainy-city'?'rainy-city':'space';
  const resolve=mode=>mode==='system'?(media.matches?'light':'dark'):mode;
  let button=null;
  let updateBackdrop=()=>{};
  let transitionCity=()=>{};
  const label=()=>{if(!button)return;const next=root.dataset.theme==='dark'?'light':'dark';button.textContent=next==='light'?'☀':'☾';button.setAttribute('aria-label','Switch to '+next+' mode');button.title='Switch to '+next+' mode'};
  const mutate=(mode,space,background,reduceMotion=root.dataset.reduceMotion==='on')=>{const previous=root.dataset.theme,oldBackground=root.dataset.background,oldImage=getComputedStyle(root).getPropertyValue("--rustdl-page-background-image");root.dataset.reduceMotion=reduceMotion?'on':'off';root.dataset.appearance=mode;root.dataset.theme=resolve(mode);root.dataset.space=space?'on':'off';root.dataset.background=validBackground(background);label();updateBackdrop();transitionCity(previous,oldBackground,oldImage)};
  const persist=(mode,space,background,reduceMotion)=>{try{localStorage.setItem('rustdl:reduce-motion',reduceMotion?'on':'off');localStorage.setItem('rustdl:appearance',mode);localStorage.setItem('rustdl:space',space?'on':'off');localStorage.setItem('rustdl:background',validBackground(background))}catch(_error){}try{bridge?.setAppearance(mode)}catch(_error){}};
  const share=(mode,space,background,reduceMotion)=>{if(window===window.top){document.querySelectorAll('iframe').forEach(frame=>{try{frame.contentWindow.RustDLTheme?.receive(mode,space,background,reduceMotion)}catch(_error){}})}else{try{window.parent.RustDLTheme?.receive(mode,space,background,reduceMotion)}catch(_error){}}};
  const apply=(requested,space=true,save=true,notify=true,background=root.dataset.background,reduceMotion=root.dataset.reduceMotion==='on')=>{const mode=valid(requested),change=()=>mutate(mode,space,background,reduceMotion);if(document.startViewTransition&&!reduceMotion&&!isMotionReduced()&&root.dataset.background!=='rainy-city')document.startViewTransition(change);else change();if(save)persist(mode,space,background,reduceMotion);if(notify)share(mode,space,background,reduceMotion)};
  const receive=(requested,space=true,background=root.dataset.background,reduceMotion=root.dataset.reduceMotion==='on')=>mutate(valid(requested),space,background,reduceMotion);
  window.RustDLTheme={apply,receive,isMotionReduced};
  const mountBackdrop=()=>{
    if(!document.body||typeof requestAnimationFrame!=='function')return;
    const backdrop=document.createElement('div');
    backdrop.className='rustdl-city-backdrop';
    backdrop.setAttribute('aria-hidden','true');
    const picture=document.createElement('div');
    picture.className='rustdl-city-image';
    backdrop.append(picture);
    document.body.prepend(backdrop);
    let sequence=null,animations=[],generation=0,loaded=null;
    const stopSunrise=()=>{generation++;animations.forEach(animation=>animation.cancel());animations=[];sequence?.remove();sequence=null};
    const loadFrames=()=>{
      if(!loaded){
        const styles=getComputedStyle(root);
        const urls=Array.from({length:12},(_,i)=>styles.getPropertyValue('--rustdl-sunrise-'+(i+1)).trim());
        loaded=Promise.all(urls.map(url=>new Promise((resolve,reject)=>{
          const match=url.match(/^url\(["']?(.*?)["']?\)$/);
          if(!match){reject(new Error('Missing sunrise frame'));return}
          const image=new Image();image.onload=()=>resolve(url);image.onerror=reject;image.src=match[1];
        }))).catch(()=>{loaded=null;return null});
      }
      return loaded;
    };
    transitionCity=async(previous,oldBackground,oldImage)=>{
      if(previous===root.dataset.theme&&oldBackground===root.dataset.background)return;
      stopSunrise();
      if(oldBackground!=='rainy-city'||root.dataset.background!=='rainy-city'||previous===root.dataset.theme||isMotionReduced()||document.hidden||document.body.classList.contains('pip')||typeof picture.animate!=='function')return;
      const token=generation,target=root.dataset.theme;
      const container=document.createElement('div');container.className='rustdl-sunrise';container.style.backgroundImage=oldImage;
      picture.append(container);sequence=container;
      const urls=await loadFrames();
      if(token!==generation)return;
      if(!urls){stopSunrise();return}
      const ordered=target==='light'?urls:[...urls].reverse();
      ordered.forEach((url,i)=>{
        const layer=document.createElement('div');layer.style.backgroundImage=url;layer.style.opacity='0';container.append(layer);
        animations.push(layer.animate([{opacity:0},{opacity:1}],{duration:400,delay:i*280,fill:'forwards',easing:'ease-in-out'}));
      });
      const ending=container.animate([{opacity:1},{opacity:0}],{duration:700,delay:3600,fill:'forwards',easing:'ease-in-out'});
      animations.push(ending);
      ending.finished.then(()=>{if(token===generation)stopSunrise()},()=>{});
    };
    let frame=null;
    const cancel=()=>{if(frame!==null){cancelAnimationFrame(frame);frame=null}};
    updateBackdrop=()=>{
      const enabled=root.dataset.background==='rainy-city'&&!isMotionReduced()&&!document.body.classList.contains('pip');
      const state=enabled?'on':'off';
      if(root.dataset.cityParallax!==state)root.dataset.cityParallax=state;
      if(!enabled||document.hidden){cancel();stopSunrise();return}
      if(frame!==null)return;
      frame=requestAnimationFrame(()=>{
        frame=null;
        const viewport=window.innerHeight;
        const distance=Math.max(0,root.scrollHeight-viewport);
        const progress=distance?Math.min(1,Math.max(0,window.scrollY/distance)):.5;
        const offset=(.5-progress)*viewport*.08;
        picture.style.transform=`translate3d(0,${offset.toFixed(2)}px,0)`;
      });
    };
    addEventListener('scroll',updateBackdrop,{passive:true});
    addEventListener('resize',updateBackdrop,{passive:true});
    addEventListener('pageshow',updateBackdrop);
    addEventListener('pagehide',()=>{cancel();stopSunrise()});
    document.addEventListener('visibilitychange',updateBackdrop);
    reduced.addEventListener?.('change',updateBackdrop);
    if(typeof ResizeObserver==='function')new ResizeObserver(updateBackdrop).observe(document.body);
    new MutationObserver(updateBackdrop).observe(document.body,{attributes:true,attributeFilter:['class']});
    updateBackdrop();
  };
  if(document.readyState==='loading')addEventListener('DOMContentLoaded',mountBackdrop,{once:true});else mountBackdrop();
  const mount=()=>{button=document.createElement('button');button.type='button';button.className='rustdl-theme-toggle';button.addEventListener('click',()=>apply(root.dataset.theme==='dark'?'light':'dark',root.dataset.space!=='off'));document.body.append(button);label()};
  media.addEventListener?.('change',()=>{if(root.dataset.appearance==='system')mutate('system',root.dataset.space!=='off',root.dataset.background)});
  document.addEventListener('visibilitychange',()=>root.dataset.spacePaused=String(document.hidden));
  if(window===window.top){if(document.readyState==='loading')addEventListener('DOMContentLoaded',mount,{once:true});else mount()}
})();