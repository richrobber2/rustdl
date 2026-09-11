(()=>{
  'use strict';
  const bridge=window.RustDLPlayback||null;
  const video=document.querySelector('video[data-filename],audio[data-filename]');
  const finite=value=>Number.isFinite(value);
  const clamp=(value,min,max)=>Math.min(max,Math.max(min,value));
  const formatTime=value=>{
    if(!finite(value)||value<0)return '0:00';
    const seconds=Math.floor(value%60).toString().padStart(2,'0');
    const minutes=Math.floor(value/60)%60;
    const hours=Math.floor(value/3600);
    return hours?hours+':'+minutes.toString().padStart(2,'0')+':'+seconds:minutes+':'+seconds;
  };
  const formatBytes=value=>{
    if(!finite(value)||value<=0)return '0 B';
    const units=['B','KB','MB','GB'];let unit=0;
    while(value>=1024&&unit<units.length-1){value/=1024;unit++}
    return value.toFixed(unit?1:0)+' '+units[unit];
  };
  const localKey=name=>'rustdl:playback:'+name;
  const safeParse=value=>{try{return JSON.parse(value)}catch(_error){return null}};
  const readLocal=name=>safeParse(localStorage.getItem(localKey(name)))||{};
  const saveLocal=(name,position,duration)=>localStorage.setItem(localKey(name),JSON.stringify({position,duration,updated:Date.now()}));
  const loadPosition=name=>bridge?Number(bridge.getPosition(name)):Number(readLocal(name).position||0);
  const savePosition=(name,position,duration)=>{
    if(!finite(position)||!finite(duration)||duration<=0)return;
    if(bridge)bridge.savePosition(name,position,duration);else saveLocal(name,position,duration);
  };
  const clearPosition=name=>{if(bridge)bridge.clearPosition(name);else localStorage.removeItem(localKey(name));};
  const loadRate=()=>bridge?Number(bridge.getPlaybackRate()):Number(localStorage.getItem('rustdl:rate')||1);
  const saveRate=rate=>{if(bridge)bridge.savePlaybackRate(rate);else localStorage.setItem('rustdl:rate',String(rate));};
  const playbackQueueKey='rustdl:up-next:v1';
  const fullscreenResumeKey='rustdl:resume-fullscreen:v1';
  const saveFullscreenResume=target=>{try{sessionStorage.setItem(fullscreenResumeKey,JSON.stringify({path:new URL('/watch/'+encodeURIComponent(target),location.origin).pathname,at:Date.now()}))}catch(_error){}};
  const loadFullscreenResume=()=>{try{return safeParse(sessionStorage.getItem(fullscreenResumeKey))}catch(_error){return null}};
  const validQueuedFilename=name=>typeof name==='string'&&/^[A-Za-z0-9_-]+\.(?:mp4|m4a)$/.test(name);
  const loadPlaybackQueue=()=>{try{const value=safeParse(localStorage.getItem(playbackQueueKey));if(!Array.isArray(value))return[];return[...new Set(value.filter(validQueuedFilename))].slice(0,200)}catch(_error){return[]}};
  const savePlaybackQueue=queue=>{queue=[...new Set(queue.filter(validQueuedFilename))].slice(0,200);try{localStorage.setItem(playbackQueueKey,JSON.stringify(queue))}catch(_error){}return queue};
  const enqueuePlayback=name=>{const queue=loadPlaybackQueue();if(queue.includes(name))return false;queue.push(name);savePlaybackQueue(queue);return true};
  const removeFromPlaybackQueue=name=>savePlaybackQueue(loadPlaybackQueue().filter(item=>item!==name));
  const fetchState=filename=>fetch('/__app/state.json'+(filename?'?file='+encodeURIComponent(filename):''),{cache:'no-store'}).then(response=>response.ok?response.json():Promise.reject()).catch(()=>null);
  const supportsPopover='showPopover' in HTMLElement.prototype;
  const connectPopover=(button,popover)=>{
    button.setAttribute('popovertarget',popover.id);
    if(supportsPopover)return;
    popover.hidden=true;
    button.addEventListener('click',event=>{event.preventDefault();popover.hidden=!popover.hidden});
  };

  if(video){
    addEventListener('rustdl:gallery',()=>{document.querySelectorAll('iframe.mini-library').forEach(frame=>{try{frame.contentWindow.dispatchEvent(new Event('rustdl:gallery'))}catch(_error){}})});
    const audioOnly=video instanceof HTMLAudioElement;
    let filename=video.dataset.filename;
    const growing=video.dataset.growing==='true';
    const frame=video.closest('.player-frame');
    const toast=document.querySelector('.seek-toast');
    const rates=[0.5,0.75,1,1.25,1.5,2];
    let locked=false;
    let lastSaved=0;
    let toastTimer=0;
    let idleTimer=0;
    let downloadFraction=1;
    let bufferedFraction=0;
    let currentState=null;
    let sleepTimer=0;
    let seekingPointer=null;
    const showToast=message=>{
      if(!toast)return;
      toast.textContent=message;toast.classList.add('visible');clearTimeout(toastTimer);
      toastTimer=setTimeout(()=>toast.classList.remove('visible'),850);
    };
    const showSeek=delta=>showToast((delta>0?'+':'−')+Math.abs(delta)+' seconds');

    const island=document.createElement('div');
    island.className='control-island';island.setAttribute('role','group');island.setAttribute('aria-label',audioOnly?'Audio controls':'Video controls');
    island.innerHTML='<button class="control-button" type="button" data-control-play aria-label="Play">▶</button><button class="control-button" type="button" data-control-next aria-label="Play next">⏭</button><span class="control-time" data-control-time>0:00 / 0:00</span><div class="timeline-shell"><span class="timeline-track"></span><span class="timeline-downloaded"></span><span class="timeline-played"></span><span class="download-boundary"></span><span class="scrub-anchor"></span><span class="scrub-preview"><img alt=""><video muted playsinline preload="metadata" aria-hidden="true"></video><output>0:00</output></span><input class="timeline-input" type="range" min="0" max="1000" value="0" aria-label="Seek video"></div><button class="control-button" type="button" data-control-mute aria-label="Mute">Vol</button><button class="control-button" type="button" data-control-speed aria-label="Playback speed">1×</button><button class="control-button" type="button" data-control-mini aria-label="Browse with mini-player">▦</button><button class="control-button" type="button" data-control-pip aria-label="Picture in picture">PiP</button><button class="control-button" type="button" data-control-more aria-label="More playback controls">•••</button><button class="control-button" type="button" data-control-fullscreen aria-label="Fullscreen">⛶</button>';
    const downloadLabel=document.createElement('span');downloadLabel.className='download-label';downloadLabel.textContent='Saved locally';
    const speedMenu=document.createElement('div');speedMenu.id='speed-popover';speedMenu.className='control-popover';speedMenu.setAttribute('popover','auto');
    speedMenu.innerHTML='<h3>Playback speed</h3><div class="control-popover-grid"></div>';
    const moreMenu=document.createElement('div');moreMenu.id='more-popover';moreMenu.className='control-popover';moreMenu.setAttribute('popover','auto');
    moreMenu.innerHTML='<h3>Playback options</h3><div class="control-popover-grid"><button type="button" data-option-rotation>Lock rotation</button><button type="button" data-sleep="15">Sleep 15m</button><button type="button" data-sleep="30">Sleep 30m</button><button type="button" data-sleep="60">Sleep 60m</button><button type="button" data-sleep="0">Clear timer</button></div><div class="control-detail"><span data-up-next>Up Next · loading</span><button type="button" data-clear-up-next hidden>Clear Up Next</button><span data-quality>Quality · detecting</span><span data-audio>Audio · default track</span><span data-captions>Captions · none</span><a data-requality hidden>Choose another quality</a></div>';
    frame.append(island,downloadLabel,speedMenu,moreMenu);
    document.querySelector('.player-actions')?.remove();
    video.controls=false;video.dataset.enhanced='true';

    const play=island.querySelector('[data-control-play]');
    const next=island.querySelector('[data-control-next]');
    const time=island.querySelector('[data-control-time]');
    const timeline=island.querySelector('.timeline-input');
    const played=island.querySelector('.timeline-played');
    const downloaded=island.querySelector('.timeline-downloaded');
    const boundary=island.querySelector('.download-boundary');
    const scrubAnchor=island.querySelector('.scrub-anchor');
    const preview=island.querySelector('.scrub-preview');
    const previewImage=preview.querySelector('img');
    const previewVideo=preview.querySelector('video');
    const previewTime=preview.querySelector('output');
    let previewTimer=0;
    let previewTarget=0;
    let previewSequence=0;
    const mute=island.querySelector('[data-control-mute]');
    const speed=island.querySelector('[data-control-speed]');
    const mini=island.querySelector('[data-control-mini]');
    const labelPipButton=button=>{
      button.classList.add('pip-button');
      button.title='Picture-in-picture · Floating video';
      button.setAttribute('aria-label','Picture-in-picture: open floating video');
      button.innerHTML='<svg viewBox="0 0 24 18" width="24" height="18" aria-hidden="true" focusable="false"><rect x="1" y="1" width="22" height="16" rx="2" fill="none" stroke="currentColor" stroke-width="1.5"/><rect x="12" y="9" width="8" height="5" rx="1" fill="currentColor"/></svg><span class="pip-button-label" aria-hidden="true">PiP</span>';
    };
    const pip=island.querySelector('[data-control-pip]');labelPipButton(pip);
    const more=island.querySelector('[data-control-more]');
    const fullscreen=island.querySelector('[data-control-fullscreen]');
    const rotation=moreMenu.querySelector('[data-option-rotation]');
    const upNextDetail=moreMenu.querySelector('[data-up-next]');
    const clearUpNext=moreMenu.querySelector('[data-clear-up-next]');
    if(!audioOnly)previewImage.src='/thumbnail/'+encodeURIComponent(filename)+'.jpg';
    connectPopover(speed,speedMenu);connectPopover(more,moreMenu);

    let playbackOrder=[];
    const queuedWithoutCurrent=()=>loadPlaybackQueue().filter(item=>item!==filename);
    if(loadPlaybackQueue().includes(filename))removeFromPlaybackQueue(filename);
    const fallbackNext=()=>{const index=playbackOrder.indexOf(filename);return index>=0?playbackOrder[index+1]||null:playbackOrder.find(item=>item!==filename)||null};
    const syncUpNext=()=>{const queue=queuedWithoutCurrent(),fallback=fallbackNext(),candidate=queue[0]||fallback;next.disabled=!candidate;next.setAttribute('aria-label',candidate?'Play next · '+candidate:'No next item');upNextDetail.textContent=queue.length?'Up Next · '+queue.length+' queued':'Up Next · '+(fallback?'gallery fallback ready':'queue empty');clearUpNext.hidden=queue.length===0};
    const playbackOrderPromise=fetch('/__app/playback-order.json',{cache:'no-store'}).then(response=>response.ok?response.json():Promise.reject()).then(data=>{playbackOrder=Array.isArray(data.items)?data.items.filter(validQueuedFilename):[]}).catch(()=>{playbackOrder=[]}).finally(syncUpNext);
    const spaNavigate=async target=>{
      const href='/watch/'+encodeURIComponent(target);
      try{
        const response=await fetch(href,{cache:'no-store'});
        if(!response.ok)throw new Error('track unavailable');
        const markup=await response.text();
        const parsed=new DOMParser().parseFromString(markup,'text/html');
        const nextMedia=parsed.querySelector('video[data-filename],audio[data-filename]');
        if(!nextMedia||nextMedia.tagName!==video.tagName)throw new Error('media type changed');
        const source=nextMedia.currentSrc||nextMedia.src||nextMedia.querySelector('source')?.src;
        if(!source)throw new Error('media source missing');
        video.pause();video.removeAttribute('src');video.querySelectorAll('source').forEach(node=>node.remove());
        video.src=source;filename=nextMedia.dataset.filename||target;video.dataset.filename=filename;seekingPointer=null;timeline.value='0';played.style.width='0%';downloaded.style.width='0%';video.load();
        const heading=parsed.querySelector('h1');const currentHeading=document.querySelector('.player-shell header h1');if(heading&&currentHeading)currentHeading.textContent=heading.textContent;
        history.pushState({player:filename},'',href);document.title=parsed.title||document.title;showToast('Loaded next track');
        return true;
      }catch(_error){return false}
    };
    const navigateNext=async target=>{if(await spaNavigate(target))return;if(document.fullscreenElement)saveFullscreenResume(target);location.href='/watch/'+encodeURIComponent(target)};
    const advanceToNext=async()=>{await playbackOrderPromise;let queue=queuedWithoutCurrent().filter(item=>playbackOrder.includes(item));savePlaybackQueue(queue);if(queue.length){const target=queue.shift();savePlaybackQueue(queue);navigateNext(target);return}const target=fallbackNext();if(target)navigateNext(target);else showToast('Nothing queued next')};
    next.addEventListener('click',advanceToNext);
    clearUpNext.addEventListener('click',()=>{savePlaybackQueue([]);syncUpNext();showToast('Up Next cleared')});
    addEventListener('storage',event=>{if(event.key===playbackQueueKey)syncUpNext()});
    syncUpNext();

    const revealControls=()=>{
      frame.classList.remove('controls-idle');clearTimeout(idleTimer);
      if(!video.paused)idleTimer=setTimeout(()=>frame.classList.add('controls-idle'),2600);
    };
    ['pointerdown','pointermove','focusin'].forEach(type=>frame.addEventListener(type,revealControls,{passive:true}));
    const updateControls=()=>{
      const duration=finite(video.duration)?video.duration:0;
      const percent=duration?clamp(video.currentTime/duration*100,0,100):0;
      if(seekingPointer===null){timeline.value=String(Math.round(percent*10));played.style.width=percent+'%'}
      time.textContent=formatTime(video.currentTime)+' / '+formatTime(duration);
      play.textContent=video.paused?'▶':'❚❚';play.setAttribute('aria-label',video.paused?'Play':'Pause');
    };
    const bufferedEnd=()=>{
      const duration=finite(video.duration)?video.duration:0;if(!duration)return 0;let limit=0;
      for(let index=0;index<video.buffered.length;index++){const start=video.buffered.start(index),end=video.buffered.end(index);if(start<=limit+.35||video.currentTime>=start-.35&&video.currentTime<=end+.35)limit=Math.max(limit,end)}
      return clamp(limit,0,duration);
    };
    const syncBuffered=()=>{
      const duration=finite(video.duration)?video.duration:0;bufferedFraction=duration?clamp(bufferedEnd()/duration,0,1):0;
      const ready=!growing||currentState&&currentState.phase==="ready",playable=ready?1:(bufferedFraction||downloadFraction);boundary.style.left=(playable*100)+"%";boundary.style.opacity=ready?"0":"1";
    };
    const isBuffered=time=>{for(let index=0;index<video.buffered.length;index++)if(time>=video.buffered.start(index)-.05&&time<=video.buffered.end(index)-.2)return true;return false};
    const seekSafely=(requested,notify=true)=>{
      const duration=finite(video.duration)?video.duration:0;if(!duration)return false;requested=clamp(requested,0,duration);
      if(!growing||currentState&&currentState.phase==="ready"||requested<=video.currentTime||isBuffered(requested)){video.currentTime=requested;return true}
      const limit=bufferedEnd(),target=Math.max(0,limit-.35);video.currentTime=target;if(notify)showToast("Waiting for download");return false;
    };
    const seekBy=delta=>{const before=video.currentTime;if(seekSafely(before+delta))showSeek(Math.round(video.currentTime-before))};
    const queuePreview=requested=>{
      if(audioOnly)return;previewTarget=requested;previewSequence++;
      const available=!growing||currentState&&currentState.phase==='ready'||requested<=video.currentTime||isBuffered(requested);
      preview.classList.toggle('waiting',!available);
      if(!available){clearTimeout(previewTimer);previewTimer=0;preview.classList.remove('has-frame');previewTime.value=formatTime(requested)+' · waiting';return}
      previewTime.value=formatTime(requested);if(previewTimer)return;previewTimer=setTimeout(()=>{previewTimer=0;const sequence=previewSequence;
        if(!previewVideo.getAttribute('src')){previewVideo.src=video.currentSrc||video.src;previewVideo.load()}
        const seek=()=>{if(sequence!==previewSequence)return;previewVideo.dataset.sequence=String(sequence);const duration=finite(previewVideo.duration)?previewVideo.duration:video.duration||0,target=clamp(previewTarget,0,duration);if(Math.abs(previewVideo.currentTime-target)<.08){preview.classList.add('has-frame');return}if(typeof previewVideo.fastSeek==='function')previewVideo.fastSeek(target);else previewVideo.currentTime=target};
        if(previewVideo.readyState>=1)seek();else previewVideo.onloadedmetadata=seek;
      },70);
    };
    previewVideo.addEventListener('seeked',()=>{if(Number(previewVideo.dataset.sequence)!==previewSequence)return;preview.classList.remove('waiting');preview.classList.add('has-frame')});
    previewVideo.addEventListener('error',()=>preview.classList.remove('has-frame'));
    const hidePreview=()=>{clearTimeout(previewTimer);previewTimer=0;preview.classList.remove('visible')};
    const togglePlayback=()=>video.paused?video.play().catch(()=>{}):video.pause();
    play.addEventListener('click',togglePlayback);
    video.addEventListener('click',togglePlayback);
    mute.addEventListener('click',()=>{video.muted=!video.muted;mute.textContent=video.muted?'Muted':'Vol';mute.setAttribute('aria-pressed',String(video.muted))});
    const previewSeek=()=>{
      const percent=Number(timeline.value)/10;
      scrubAnchor.style.left=percent+'%';preview.style.left=percent+'%';
      played.style.width=percent+'%';
      const requested=(video.duration||0)*percent/100;previewTime.value=formatTime(requested);preview.classList.add('visible');queuePreview(requested);
    };
    timeline.addEventListener('input',previewSeek);
    const seekFromPointer=event=>{
      const bounds=timeline.getBoundingClientRect();
      const fraction=bounds.width?clamp((event.clientX-bounds.left)/bounds.width,0,1):0;
      timeline.value=String(Math.round(fraction*1000));previewSeek();
    };
    const commitSeek=()=>{
      seekSafely((video.duration||0)*Number(timeline.value)/1000);
      hidePreview();updateControls();
    };
    timeline.addEventListener('pointerdown',event=>{
      if(event.pointerType==='mouse'&&event.button!==0)return;
      event.preventDefault();event.stopPropagation();seekingPointer=event.pointerId;
      timeline.setPointerCapture?.(event.pointerId);seekFromPointer(event);
    });
    timeline.addEventListener('pointermove',event=>{
      if(seekingPointer!==event.pointerId)return;
      event.preventDefault();event.stopPropagation();seekFromPointer(event);
    });
    timeline.addEventListener('pointerup',event=>{
      if(seekingPointer!==event.pointerId)return;
      event.preventDefault();event.stopPropagation();seekFromPointer(event);
      timeline.releasePointerCapture?.(event.pointerId);seekingPointer=null;commitSeek();
    });
    timeline.addEventListener('pointercancel',event=>{
      if(seekingPointer!==event.pointerId)return;
      event.preventDefault();event.stopPropagation();seekingPointer=null;
      hidePreview();updateControls();
    });
    timeline.addEventListener('click',event=>event.preventDefault());
    timeline.addEventListener('change',commitSeek);

    const applyRate=rate=>{
      video.playbackRate=rate;speed.textContent=rate+'×';saveRate(rate);
      speedMenu.querySelectorAll('button').forEach(button=>button.classList.toggle('active',Number(button.dataset.rate)===rate));
    };
    const speedGrid=speedMenu.querySelector('.control-popover-grid');
    rates.forEach(rate=>{
      const button=document.createElement('button');button.type='button';button.dataset.rate=String(rate);button.textContent=rate+'×';
      button.addEventListener('click',()=>{applyRate(rate);if(supportsPopover)speedMenu.hidePopover()});speedGrid.append(button);
    });
    rotation.addEventListener('click',()=>{
      locked=!locked;rotation.classList.toggle('active',locked);rotation.textContent=locked?'Rotation locked':'Lock rotation';
      if(bridge)bridge.setRotationLocked(locked);
    });
    moreMenu.querySelectorAll('[data-sleep]').forEach(button=>button.addEventListener('click',()=>{
      clearTimeout(sleepTimer);const minutes=Number(button.dataset.sleep);
      if(minutes){sleepTimer=setTimeout(()=>{video.pause();showToast('Sleep timer finished')},minutes*60000);showToast('Sleep timer · '+minutes+' minutes')}
      else showToast('Sleep timer cleared');
      if(supportsPopover)moreMenu.hidePopover();
    }));
    const nativePip=bridge&&bridge.supportsPictureInPicture();
    const browserPip=document.pictureInPictureEnabled&&video.requestPictureInPicture;
    pip.hidden=audioOnly;pip.disabled=audioOnly||(!nativePip&&!browserPip);
    rotation.hidden=audioOnly;
    const enterPip=()=>{
      if(nativePip)bridge.enterPictureInPicture(video.videoWidth||16,video.videoHeight||9);
      else if(browserPip)video.requestPictureInPicture().catch(()=>{});
    };
    pip.addEventListener('click',enterPip);

    const frameParent=frame.parentNode;
    const frameNext=frame.nextSibling;
    const frameTransitionName=frame.style.viewTransitionName;
    let miniState=null;
    const syncMiniPlay=()=>{if(miniState)miniState.play.textContent=video.paused?'▶':'❚❚'};
    const exitMini=()=>{
      if(!miniState)return;
      if(frameNext&&frameNext.parentNode===frameParent)frameParent.insertBefore(frame,frameNext);else frameParent.append(frame);
      miniState.browser.remove();miniState.dock.remove();miniState=null;
      document.body.classList.remove('mini-player-mode');frame.style.viewTransitionName=frameTransitionName;
    };
    const wireMiniBrowser=browser=>{
      let doc;try{doc=browser.contentDocument}catch(_error){return}if(!doc)return;
      doc.addEventListener('click',event=>{
        const link=event.target&&typeof event.target.closest==='function'?event.target.closest('a[href]'):null;if(!link)return;
        let target;try{target=new URL(link.href,location.href)}catch(_error){return}
        if(target.origin===location.origin&&target.pathname.startsWith('/watch/')){event.preventDefault();location.href=target.href}
      },true);
    };
    const enterMini=()=>{
      if(miniState)return;
      const browser=document.createElement('iframe');browser.className='mini-library';browser.title='RustDL gallery';browser.addEventListener('load',()=>wireMiniBrowser(browser));browser.src='/';
      const dock=document.createElement('aside');dock.className='mini-player-dock';dock.setAttribute('aria-label','Now playing');
      const footer=document.createElement('div');footer.className='mini-player-footer';
      const title=document.createElement('span');title.className='mini-player-title';title.textContent=filename;
      const miniPlay=document.createElement('button');miniPlay.type='button';miniPlay.setAttribute('aria-label','Play or pause');
      const miniNext=document.createElement('button');miniNext.type='button';miniNext.textContent='⏭';miniNext.setAttribute('aria-label','Play next');
      const miniPip=document.createElement('button');miniPip.type='button';labelPipButton(miniPip);miniPip.hidden=audioOnly||(!nativePip&&!browserPip);
      const expand=document.createElement('button');expand.type='button';expand.textContent='↗';expand.setAttribute('aria-label','Return to full player');
      const close=document.createElement('button');close.type='button';close.textContent='×';close.setAttribute('aria-label','Close player');
      footer.append(title,miniPlay,miniNext,miniPip,expand,close);dock.append(frame,footer);document.body.append(browser,dock);
      miniState={browser,dock,play:miniPlay};document.body.classList.add('mini-player-mode');frame.style.viewTransitionName='none';syncMiniPlay();
      miniPlay.addEventListener('click',togglePlayback);miniNext.addEventListener('click',advanceToNext);miniPip.addEventListener('click',enterPip);expand.addEventListener('click',exitMini);
      close.addEventListener('click',()=>{video.pause();savePosition(filename,video.currentTime,video.duration);location.href='/'});
    };
    mini.addEventListener('click',enterMini);
    document.querySelectorAll('.action[href="/"]').forEach(link=>link.addEventListener('click',event=>{event.preventDefault();enterMini()}));
    fullscreen.addEventListener('click',()=>{
      if(document.fullscreenElement)document.exitFullscreen().catch(()=>{});
      else if(frame.requestFullscreen)frame.requestFullscreen().catch(()=>video.requestFullscreen?.().catch(()=>{}));
      else video.requestFullscreen?.().catch(()=>{});
    });
    const resumeFullscreen=()=>{
      const request=()=>{if(frame.requestFullscreen)return frame.requestFullscreen();if(video.requestFullscreen)return video.requestFullscreen();return Promise.reject()};
      request().then(()=>{try{sessionStorage.removeItem(fullscreenResumeKey)}catch(_error){};resumeButton?.remove()}).catch(()=>showToast('Tap Resume fullscreen to continue'));
    };
    let resumeButton=null;
    const resumeMarker=loadFullscreenResume();
    if(resumeMarker&&resumeMarker.path===location.pathname&&Date.now()-Number(resumeMarker.at||0)<30000){
      try{sessionStorage.removeItem(fullscreenResumeKey)}catch(_error){}
      resumeButton=document.createElement('button');resumeButton.type='button';resumeButton.className='fullscreen-resume';resumeButton.textContent='Resume fullscreen';resumeButton.setAttribute('aria-label','Resume fullscreen');frame.append(resumeButton);resumeButton.addEventListener('click',resumeFullscreen,{once:true});
      setTimeout(()=>resumeFullscreen(),0);
    }

    const applySaved=()=>{
      let rate=loadRate();if(!rates.includes(rate))rate=1;applyRate(rate);
      const position=loadPosition(filename);
      if(finite(position)&&position>=5&&position<video.duration-5)seekSafely(position,false);
      updateControls();
    };
    video.addEventListener('loadedmetadata',applySaved);
    if(video.readyState>=1)applySaved();
    ['progress','canplay','durationchange'].forEach(type=>video.addEventListener(type,syncBuffered));
    video.addEventListener('timeupdate',()=>{
      updateControls();
      if(Date.now()-lastSaved>=2000){lastSaved=Date.now();savePosition(filename,video.currentTime,video.duration)}
      if('mediaSession' in navigator&&finite(video.duration)&&video.duration>0)try{navigator.mediaSession.setPositionState({duration:video.duration,playbackRate:video.playbackRate,position:clamp(video.currentTime,0,video.duration)})}catch(_error){}
    });
    const setNativePlaying=playing=>{if(bridge)bridge.setPlaybackState(playing&&!audioOnly,video.videoWidth||16,video.videoHeight||9)};
    video.addEventListener('pause',()=>{updateControls();syncMiniPlay();revealControls();savePosition(filename,video.currentTime,video.duration);setNativePlaying(false);if('mediaSession' in navigator)navigator.mediaSession.playbackState='paused'});
    video.addEventListener('play',()=>{updateControls();syncMiniPlay();revealControls();setNativePlaying(true);if('mediaSession' in navigator)navigator.mediaSession.playbackState='playing'});
    video.addEventListener('ended',()=>{if(bridge){bridge.markWatched(filename);setNativePlaying(false)}else{clearPosition(filename);localStorage.setItem('rustdl:watched:'+filename,'1')}});
    video.addEventListener('dblclick',event=>seekBy(event.offsetX<video.clientWidth/2?-10:10));
    addEventListener('keydown',event=>{
      if(event.target instanceof HTMLInputElement||event.target instanceof HTMLButtonElement)return;
      if(event.code==='Space'){event.preventDefault();togglePlayback()}
      if(event.code==='ArrowLeft')seekBy(-10)
      if(event.code==='ArrowRight')seekBy(10)
      if(event.code==='KeyN')advanceToNext()
    });

    const updateDownloadState=state=>{
      currentState=state&&state.current||null;
      if(!currentState)return;
      const total=Number(currentState.total||0);const saved=Number(currentState.downloaded||0);
      downloadFraction=total?clamp(saved/total,0,1):currentState.phase==='ready'?1:0;
      downloaded.style.width=(downloadFraction*100)+'%';syncBuffered();
      downloadLabel.textContent=currentState.phase==='ready'?'Saved locally':formatBytes(saved)+(total?' / '+formatBytes(total):'')+' downloaded';
      moreMenu.querySelector('[data-quality]').textContent='Quality · '+(currentState.quality||currentState.height&&currentState.height+'p'||'original');
      const requality=moreMenu.querySelector('[data-requality]');
      if(currentState.source){requality.href='/discover?source='+encodeURIComponent(currentState.source);requality.hidden=false}
    };
    let downloadStatePending=false;
    const refreshDownloadState=()=>{
      if(downloadStatePending)return;downloadStatePending=true;
      fetchState(filename).then(state=>{if(state)updateDownloadState(state)}).finally(()=>downloadStatePending=false);
    };
    addEventListener('rustdl:state',event=>{if(['queue','peer','activity','sync'].includes(event.detail?.type))refreshDownloadState()});
    refreshDownloadState();setInterval(()=>{if(!document.hidden)refreshDownloadState()},15000);

    if('mediaSession' in navigator&&'MediaMetadata' in window){
      const metadata={title:filename,artist:'RustDL',album:audioOnly?'Saved audio':'Saved videos'};
      if(!audioOnly)metadata.artwork=[{src:'/thumbnail/'+encodeURIComponent(filename)+'.jpg',type:'image/jpeg'}];
      navigator.mediaSession.metadata=new MediaMetadata(metadata);
      const handlers={play:()=>video.play(),pause:()=>video.pause(),stop:()=>{video.pause();video.currentTime=0},seekbackward:event=>seekBy(-(event.seekOffset||10)),seekforward:event=>seekBy(event.seekOffset||10),seekto:event=>{if(finite(event.seekTime))seekSafely(event.seekTime)},previoustrack:()=>{video.currentTime=0},nexttrack:advanceToNext};
      Object.entries(handlers).forEach(([action,handler])=>{try{navigator.mediaSession.setActionHandler(action,handler)}catch(_error){}});
    }
    addEventListener('pagehide',()=>{clearTimeout(sleepTimer);savePosition(filename,video.currentTime,video.duration);if(bridge){setNativePlaying(false);bridge.setRotationLocked(false)}});
    updateControls();revealControls();
    return;
  }

  const gallery=document.querySelector('.library');
  if(!gallery)return;
  const galleryDataElement=document.getElementById('gallery-data');
  const galleryEntries=galleryDataElement?safeParse(galleryDataElement.textContent)||[]:[];
  const available=new Set(galleryEntries.map(entry=>entry.filename).filter(Boolean));
  let thumbnailRetries=0,longTasks=0,rendered=0,matches=[];
  try{new PerformanceObserver(list=>{longTasks+=list.getEntries().length}).observe({type:'longtask',buffered:true})}catch(_error){}
  const rememberPerformance=renderMs=>{try{sessionStorage.setItem('rustdl:gallery-performance',JSON.stringify({total:galleryEntries.length,matching:matches.length,rendered,renderMs:Math.round(renderMs*10)/10,longTasks,thumbnailRetries,updated:Date.now()}))}catch(_error){}};
  const pendingThumbnails=new Set();
  const wireThumbnail=image=>{
    if(!image||image.dataset.thumbnailWired)return;image.dataset.thumbnailWired='true';
    let timer=0;
    const cancelRetry=()=>{clearTimeout(timer);timer=0;pendingThumbnails.delete(resumeRetry)};
    const isNearViewport=()=>{const rect=image.getBoundingClientRect();return rect.bottom>=-700&&rect.top<=innerHeight+700};
    const resumeRetry=()=>{
      if(!image.isConnected){cancelRetry();return}
      if(timer||document.hidden||!isNearViewport())return;
      const retry=Number(image.dataset.thumbnailRetry||0);
      timer=setTimeout(()=>{
        timer=0;
        if(!image.isConnected){cancelRetry();return}
        if(document.hidden||!isNearViewport())return;
        image.dataset.thumbnailRetry=String(retry+1);thumbnailRetries++;
        image.src=image.src.split('?')[0]+'?retry='+(retry+1)+'&t='+Date.now();
      },Math.min(30000,300*2**Math.min(retry,7)));
    };
    const retryThumbnail=()=>{pendingThumbnails.add(resumeRetry);resumeRetry()};
    image.addEventListener('error',retryThumbnail);
    image.addEventListener('load',()=>{cancelRetry();image.dataset.thumbnailRetry='0'});
    if(image.complete&&image.naturalWidth===0&&image.getAttribute('src'))retryThumbnail();
  };
  let thumbnailFrame=0;
  const resumeThumbnails=()=>{
    if(thumbnailFrame||!pendingThumbnails.size||document.hidden)return;
    thumbnailFrame=requestAnimationFrame(()=>{thumbnailFrame=0;pendingThumbnails.forEach(resume=>resume())});
  };
  addEventListener('scroll',resumeThumbnails,{passive:true});
  addEventListener('resize',resumeThumbnails);
  document.addEventListener('visibilitychange',resumeThumbnails);
  const createGalleryCard=(entry,index,resumeProgress=0)=>{
    const card=document.createElement('a');card.className='media-card';card.href=entry.href;card.dataset.galleryIndex=String(index);card.dataset.galleryKind=entry.kind;
    if(entry.kind==='playlist')card.classList.add('collection-folder');if(entry.kind==='audio'||entry.kind==='downloading-audio')card.classList.add('audio');if(entry.kind.startsWith('downloading'))card.classList.add('downloading');
    const thumb=document.createElement('div');thumb.className='media-thumb';if(entry.transitionName)thumb.dataset.viewTransitionName=entry.transitionName;
    if(entry.thumbnail){const image=document.createElement('img');image.className='media-art';image.loading='lazy';image.decoding='async';image.src='/thumbnail/'+encodeURIComponent(entry.thumbnail)+'.jpg';image.alt='';image.dataset.thumbnailRetry='0';wireThumbnail(image);thumb.append(image)}
    if(resumeProgress>0){const progress=document.createElement('span');progress.className='watch-progress';const bar=document.createElement('i');bar.style.width=Math.min(100,resumeProgress*100)+'%';progress.append(bar);thumb.append(progress)}card.append(thumb);
    const info=document.createElement('div');info.className='media-info';for(const [className,text] of [['media-state',entry.state],['media-title',entry.title],['media-file',entry.subtitle]]){const element=document.createElement('span');element.className=className;element.textContent=text;info.append(element)}card.append(info);
    if(!entry.filename)return card;
    const shell=document.createElement('div');shell.className='media-card-shell';const button=document.createElement('button');button.type='button';button.className='card-menu-button';button.textContent='•••';button.setAttribute('aria-label','Actions for '+entry.filename);shell.append(card,button);return shell;
  };
  let items=[];
  if(bridge)items=safeParse(bridge.getContinueWatching())||[];
  else for(let index=0;index<localStorage.length;index++){
    const key=localStorage.key(index);if(!key||!key.startsWith('rustdl:playback:'))continue;
    const item=readLocal(key.slice(16));if(item.position)items.push({...item,filename:key.slice(16)});
  }
  items=items.filter(item=>available.has(item.filename)&&item.duration>0&&item.position>=5&&item.position<item.duration-5).sort((a,b)=>(b.updated||0)-(a.updated||0)).slice(0,4);
  if(items.length){
    const section=document.createElement('section');section.className='library continue-watching';
    const head=document.createElement('div');head.className='library-head';
    const title=document.createElement('h2');title.textContent='Continue watching';head.append(title);
    const count=document.createElement('span');count.className='library-count';count.textContent=items.length+(items.length===1?' item':' items');head.append(count);section.append(head);
    const cards=document.createElement('div');cards.className='gallery';
    for(const item of items){
      const audio=item.filename.endsWith('.m4a');cards.append(createGalleryCard({href:'/watch/'+encodeURIComponent(item.filename),filename:item.filename,state:'Resume',title:item.filename,subtitle:formatTime(item.position)+' watched',kind:audio?'audio':'video',thumbnail:audio?null:item.filename,transitionName:'video-'+item.filename.replace(/\.(?:mp4|m4a)$/,'').replace(/[^A-Za-z0-9-]/g,'-')},-1,item.position/item.duration));
    }
    section.append(cards);gallery.before(section);
  }

  let stateCache={jobs:[],active:0};
  const queueStatus=gallery.querySelector('.playback-queue-status');
  const syncGalleryQueueStatus=()=>{if(!queueStatus)return;const count=loadPlaybackQueue().length;queueStatus.textContent='Up Next · '+(count||'empty')};
  syncGalleryQueueStatus();addEventListener('storage',event=>{if(event.key===playbackQueueKey)syncGalleryQueueStatus()});
  let selectedFilename='',selectedButton=null;
  const actionMenu=document.createElement('div');actionMenu.className='card-popover';actionMenu.id='gallery-card-actions';actionMenu.setAttribute('popover','auto');
  const actionNav=document.createElement('nav');const actionLink=(text,key)=>{const link=document.createElement('a');link.textContent=text;link.dataset.cardAction=key;actionNav.append(link);return link};
  const playAction=actionLink('Play','play');
  const queueAction=document.createElement('button');queueAction.type='button';queueAction.dataset.cardAction='queue';actionNav.append(queueAction);
  const sendAction=actionLink('Send to device','send'),sourceAction=actionLink('Open source','source'),qualityAction=actionLink('Choose another quality','quality');sourceAction.hidden=true;qualityAction.hidden=true;
  let shareAction=null;if(bridge){shareAction=document.createElement('button');shareAction.type='button';shareAction.dataset.cardAction='share';actionNav.append(shareAction)}
  const watchedAction=document.createElement('button');watchedAction.type='button';watchedAction.dataset.cardAction='watched';watchedAction.textContent='Mark watched';actionNav.append(watchedAction);
  const deleteAction=actionLink('Delete…','delete');deleteAction.className='danger';actionMenu.append(actionNav);document.body.append(actionMenu);
  const closeActionMenu=()=>{if(supportsPopover)actionMenu.hidePopover?.();else actionMenu.hidden=true};
  const syncQueueAction=()=>{const queued=loadPlaybackQueue().includes(selectedFilename);queueAction.textContent=queued?'Remove from Up Next':'Add to Up Next';queueAction.setAttribute('aria-pressed',String(queued))};
  const openActionMenu=button=>{const card=button.closest('.media-card-shell')?.querySelector('.media-card[href^="/watch/"]');if(!card)return;selectedFilename=decodeURIComponent(card.getAttribute('href').slice(7));selectedButton?.style.removeProperty('anchor-name');selectedButton=button;button.style.setProperty('anchor-name','--active-card-actions');actionMenu.style.setProperty('position-anchor','--active-card-actions');playAction.href=card.getAttribute('href');sendAction.href='/peers/send?file='+encodeURIComponent(selectedFilename);deleteAction.href='/storage/confirm?action=delete&file='+encodeURIComponent(selectedFilename);syncQueueAction();if(shareAction)shareAction.textContent=selectedFilename.endsWith('.m4a')?'Share audio':'Share video';const job=(stateCache.jobs||[]).find(item=>item.filename===selectedFilename);sourceAction.hidden=!job?.source;qualityAction.hidden=!job?.source;if(job?.source){sourceAction.href=job.source;qualityAction.href='/discover?source='+encodeURIComponent(job.source)}if(supportsPopover)actionMenu.showPopover();else actionMenu.hidden=false};
  if(!supportsPopover)actionMenu.hidden=true;
  document.addEventListener('click',event=>{const button=event.target instanceof Element?event.target.closest('.card-menu-button'):null;if(button){event.preventDefault();openActionMenu(button);return}if(!supportsPopover&&!actionMenu.contains(event.target))closeActionMenu()});
  shareAction?.addEventListener('click',()=>{if(selectedFilename)bridge.shareVideo(selectedFilename);closeActionMenu()});
  queueAction.addEventListener('click',()=>{if(!selectedFilename)return;if(loadPlaybackQueue().includes(selectedFilename))removeFromPlaybackQueue(selectedFilename);else enqueuePlayback(selectedFilename);syncQueueAction();syncGalleryQueueStatus();closeActionMenu()});
  watchedAction.addEventListener('click',()=>{if(!selectedFilename)return;if(bridge)bridge.markWatched(selectedFilename);else{clearPosition(selectedFilename);localStorage.setItem('rustdl:watched:'+selectedFilename,'1')}closeActionMenu()});

  const galleryTools=document.querySelector('.gallery-tools'),galleryItems=document.getElementById('gallery-items');
  if(galleryTools&&galleryItems){
    const search=galleryTools.querySelector('.gallery-search'),buttons=[...galleryTools.querySelectorAll('[data-gallery-filter]')],status=galleryTools.querySelector('.gallery-filter-status'),empty=document.getElementById('gallery-empty'),sentinel=document.getElementById('gallery-sentinel'),batchSize=32;
    const stateKey='rustdl:gallery:'+location.pathname;let savedState=safeParse(sessionStorage.getItem(stateKey))||{};
    const restoreY=Number(savedState.scrollY)||0;let filter=buttons.some(button=>button.dataset.galleryFilter===savedState.filter)?savedState.filter:'all';search.value=typeof savedState.query==='string'?savedState.query:'';
    galleryEntries.forEach(entry=>entry.searchText=entry.searchText||[entry.state,entry.title,entry.subtitle].join(' ').toLocaleLowerCase());
    let batchFrame=0;
    const scheduleGalleryBatch=()=>{
      if(batchFrame||rendered>=matches.length||document.hidden)return;
      batchFrame=requestAnimationFrame(()=>{
        batchFrame=0;
        if(rendered<matches.length&&sentinel.getBoundingClientRect().top<=innerHeight+700)renderBatch();
      });
    };
    const persistGalleryState=()=>{savedState={query:search.value,filter,scrollY:scrollY,rendered};sessionStorage.setItem(stateKey,JSON.stringify(savedState))};
    const updateStatus=renderMs=>{empty.hidden=matches.length!==0;sentinel.hidden=rendered>=matches.length;status.textContent=rendered+' rendered · '+matches.length+' matching · '+galleryEntries.length+' total';buttons.forEach(button=>button.setAttribute('aria-pressed',String(button.dataset.galleryFilter===filter)));persistGalleryState();rememberPerformance(renderMs);scheduleGalleryBatch()};
    const renderBatch=(target=rendered+batchSize)=>{const started=performance.now(),fragment=document.createDocumentFragment(),limit=Math.min(matches.length,target);while(rendered<limit){const match=matches[rendered++];fragment.append(createGalleryCard(match.entry,match.index))}galleryItems.insertBefore(fragment,empty);updateStatus(performance.now()-started)};
    const collectGalleryMatches=()=>{const query=search.value.trim().toLocaleLowerCase();matches=[];galleryEntries.forEach((entry,index)=>{const playlist=entry.kind==='playlist',audio=entry.kind==='audio'||entry.kind==='downloading-audio',downloading=entry.kind.startsWith('downloading'),kindMatches=filter==='all'||filter==='playlists'&&playlist||filter==='audio'&&audio||filter==='video'&&!playlist&&!audio||filter==='downloading'&&downloading;if(kindMatches&&(!query||entry.searchText.includes(query)))matches.push({entry,index})})};
    const syncGalleryFilter=()=>{
      const started=performance.now();collectGalleryMatches();
      // Keep matching image elements attached so startup and filtering retain decoded thumbnails.
      const nodes=cardRoots(),existing=new Map(nodes.map(node=>[Number(cardLink(node).dataset.galleryIndex),node]));
      const limit=Math.min(matches.length,Math.max(batchSize,Number(savedState.rendered)||0)),kept=new Set();
      let position=galleryItems.firstElementChild;
      for(let index=0;index<limit;index++){
        const match=matches[index],node=existing.get(match.index)||createGalleryCard(match.entry,match.index);
        node.querySelectorAll('.media-art').forEach(wireThumbnail);kept.add(node);
        if(node!==position)galleryItems.insertBefore(node,position||empty);
        position=node.nextElementSibling;
      }
      nodes.forEach(node=>{if(!kept.has(node))node.remove()});rendered=limit;
      updateStatus(performance.now()-started);
    };

    // Import notifications update only gallery cards, preserving the visible anchor.
    const cardRoots=()=>[...galleryItems.querySelectorAll(':scope > .media-card-shell,:scope > .media-card')];
    const cardLink=node=>node.matches('.media-card')?node:node.querySelector('.media-card');
    const notice=document.createElement('div');notice.className='gallery-update-toast';notice.setAttribute('role','status');notice.hidden=true;document.body.append(notice);
    let refreshTimer=0,refreshPending=false,refreshAgain=false,noticeTimer=0,retryCount=0;
    const announce=message=>{notice.textContent=message;notice.hidden=false;clearTimeout(noticeTimer);noticeTimer=setTimeout(()=>notice.hidden=true,2500)};
    const applyGalleryUpdate=doc=>{
      const data=doc.getElementById('gallery-data'),next=data&&safeParse(data.textContent);
      if(!Array.isArray(next))throw new Error('Gallery data unavailable');
      next.forEach(entry=>entry.searchText=entry.searchText||[entry.state,entry.title,entry.subtitle].join(' ').toLocaleLowerCase());
      if(JSON.stringify(next)===JSON.stringify(galleryEntries))return;
      const nodes=cardRoots(),oldNodes=new Map(nodes.map(node=>{const entry=galleryEntries[Number(cardLink(node).dataset.galleryIndex)];return[entry.href,{node,entry}]}));
      const anchor=scrollY>0?nodes.find(node=>{const rect=node.getBoundingClientRect();return rect.bottom>0&&rect.top<innerHeight}):null;
      const anchorKey=anchor?cardLink(anchor).getAttribute('href'):null,anchorTop=anchor?.getBoundingClientRect().top,previousY=scrollY,previousX=scrollX,previousRendered=rendered,focused=document.activeElement;
      galleryEntries.splice(0,galleryEntries.length,...next);available.clear();next.forEach(entry=>{if(entry.filename)available.add(entry.filename)});galleryDataElement.textContent=data.textContent;
      collectGalleryMatches();const anchorIndex=matches.findIndex(match=>match.entry.href===anchorKey);
      const limit=Math.min(matches.length,Math.max(batchSize,previousRendered,anchorIndex<0?0:anchorIndex+batchSize));
      const kept=new Set();let position=galleryItems.firstElementChild;
      for(let index=0;index<limit;index++){
        const match=matches[index],old=oldNodes.get(match.entry.href);
        const node=old&&JSON.stringify(old.entry)===JSON.stringify(match.entry)?old.node:createGalleryCard(match.entry,match.index);
        cardLink(node).dataset.galleryIndex=String(match.index);kept.add(node);
        if(node!==position)galleryItems.insertBefore(node,position||empty);
        position=node.nextElementSibling;
      }
      nodes.forEach(node=>{if(!kept.has(node))node.remove()});rendered=limit;
      const fresh=doc.querySelector('#gallery-library .library-count'),count=gallery.querySelector('.library-count');if(fresh&&count)count.textContent=fresh.textContent;
      if(selectedButton&&!selectedButton.isConnected)closeActionMenu();
      if(focused?.isConnected&&document.activeElement!==focused)focused.focus({preventScroll:true});
      const updatedAnchor=anchorKey?cardRoots().find(node=>cardLink(node).getAttribute('href')===anchorKey):null;
      scrollTo({left:previousX,top:updatedAnchor?scrollY+updatedAnchor.getBoundingClientRect().top-anchorTop:previousY,behavior:'instant'});
      updateStatus(0);announce('Library updated');
    };
    const scheduleGalleryRefresh=(delay=200)=>{
      refreshAgain=true;
      if(refreshPending||refreshTimer||document.hidden)return;
      refreshTimer=setTimeout(()=>{refreshTimer=0;refreshGallery()},delay);
    };
    const refreshGallery=async()=>{
      if(refreshPending||document.hidden){refreshAgain=true;return}
      refreshPending=true;refreshAgain=false;
      const controller=new AbortController();let timeout=0;
      try{
        // Race the entire body read, and apply only the winning response. Late results cannot overwrite newer data.
        const deadline=new Promise((_,reject)=>{timeout=setTimeout(()=>{controller.abort();reject(new Error('Gallery refresh timed out'))},10000)});
        const request=fetch(location.pathname+location.search,{cache:'no-store',signal:controller.signal}).then(response=>{
          if(!response.ok)throw new Error('Gallery refresh failed');return response.text();
        });
        const html=await Promise.race([request,deadline]);
        applyGalleryUpdate(new DOMParser().parseFromString(html,'text/html'));retryCount=0;
      }catch(_error){
        refreshAgain=true;retryCount++;
        if(retryCount===3)announce('Library refresh delayed. Retrying automatically.');
      }finally{
        clearTimeout(timeout);refreshPending=false;
        if(refreshAgain)scheduleGalleryRefresh(retryCount?Math.min(30000,1500*2**Math.min(retryCount-1,5)):200);
      }
    };
    addEventListener('rustdl:gallery',()=>scheduleGalleryRefresh());
    document.addEventListener('visibilitychange',()=>{if(!document.hidden&&refreshAgain)scheduleGalleryRefresh()});

    let filterFrame=0;const scheduleGalleryFilter=()=>{if(filterFrame)return;filterFrame=requestAnimationFrame(()=>{filterFrame=0;syncGalleryFilter()})};
    search.addEventListener('input',scheduleGalleryFilter);buttons.forEach(button=>button.addEventListener('click',()=>{filter=button.dataset.galleryFilter;syncGalleryFilter()}));
    // An intersecting sentinel may stay intersecting after a batch: keep filling until it moves away.
    if(typeof IntersectionObserver==='function'){
      const observer=new IntersectionObserver(entries=>{if(entries.some(entry=>entry.isIntersecting))renderBatch()},{rootMargin:'700px 0px'});observer.observe(sentinel);
    }
    addEventListener('scroll',scheduleGalleryBatch,{passive:true});
    addEventListener('resize',scheduleGalleryBatch);
    document.addEventListener('visibilitychange',()=>{if(!document.hidden)scheduleGalleryBatch()});
    addEventListener('pagehide',persistGalleryState);syncGalleryFilter();if(restoreY>0)requestAnimationFrame(()=>requestAnimationFrame(()=>scrollTo({top:restoreY,behavior:'auto'})));
  }
  const updateActivityBadge=state=>{
    const badge=document.querySelector('#activity-count');if(!badge)return;const count=Number(state.activityActive||0)+Number(state.activityIssues||0);badge.hidden=count<=0;badge.textContent=String(count);
  };
  const updateQueueMini=state=>{
    stateCache=state;const jobs=state.jobs||[];
    const job=jobs.find(item=>['downloading','starting','queued','paused'].includes(item.phase));
    let mini=document.querySelector('.queue-mini');
    if(!job){mini?.remove();return}
    if(!mini){
      mini=document.createElement('aside');mini.className='queue-mini';mini.setAttribute('aria-label','Download queue');
      mini.innerHTML='<a href="/queue" aria-label="Open queue">↓</a><div class="queue-mini-info"><strong></strong><span></span><div class="queue-mini-progress"><i></i></div></div><button type="button"></button>';
      document.body.append(mini);
    }
    mini.querySelector('strong').textContent=job.filename;
    const total=Number(job.total||0),saved=Number(job.downloaded||0),percent=total?clamp(saved/total*100,0,100):0;
    mini.querySelector('.queue-mini-info span').textContent=job.phase+' · '+formatBytes(saved)+(total?' / '+formatBytes(total):'');
    mini.querySelector('.queue-mini-progress i').style.width=percent+'%';
    const action=job.phase==='paused'?'resume':'pause';const button=mini.querySelector('button');button.textContent=action==='pause'?'Pause':'Resume';
    button.onclick=()=>fetch('/queue/action?file='+encodeURIComponent(job.filename)+'&action='+action,{cache:'no-store'}).finally(refreshState);
  };
  let stateRefreshPending=false;
  const refreshState=()=>{
    if(stateRefreshPending)return;stateRefreshPending=true;
    fetchState().then(state=>{if(state){stateCache=state;updateQueueMini(state);updateActivityBadge(state)}}).finally(()=>stateRefreshPending=false);
  };
  addEventListener('rustdl:state',event=>{if(['queue','peer','activity','sync'].includes(event.detail?.type))refreshState()});
  refreshState();setInterval(()=>{if(!document.hidden)refreshState()},15000);
})();
