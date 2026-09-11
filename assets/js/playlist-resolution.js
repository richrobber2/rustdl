(()=>{
  const root=document.getElementById('playlist-resolution'),token=root.dataset.job;
  const status=document.getElementById('resolution-status'),progress=document.getElementById('resolution-progress'),elapsed=document.getElementById('resolution-time'),next=document.getElementById('resolution-continue'),cancel=document.getElementById('resolution-cancel'),errors=document.getElementById('resolution-errors'),errorList=document.getElementById('resolution-error-list');
  let timer=null,loading=false,finished=false,failureCount=-1;
  const poll=async()=>{
    if(loading||finished||document.hidden)return;
    loading=true;
    try{
      const response=await fetch('/__app/playlist-resolution.json?job='+encodeURIComponent(token),{cache:'no-store'});
      if(!response.ok){if(response.status===404)finished=true;throw Error(response.status===404?'This preparation has expired. Open the playlist again.':'Progress is temporarily unavailable. Retrying…');}
      const data=await response.json();finished=data.finished;
      progress.max=Math.max(1,data.total);progress.value=data.completed;
      status.textContent=data.cancelled?(finished?'Preparation cancelled.':'Cancelling; finishing current requests…'):(finished?data.ready+' videos ready · '+data.failures.length+' unavailable':data.completed+' of '+data.total+' checked · '+data.ready+' ready · '+data.failures.length+' unavailable');
      elapsed.textContent=Math.floor(data.elapsedSeconds/60)+'m '+data.elapsedSeconds%60+'s elapsed';
      next.hidden=!data.canContinue;next.textContent='Choose formats for '+data.ready+' videos';cancel.hidden=finished||data.cancelled;
      if(data.failures.length!==failureCount){failureCount=data.failures.length;errors.hidden=failureCount===0;document.getElementById('resolution-errors-summary').textContent=failureCount+' unavailable videos';errorList.replaceChildren(...data.failures.map(message=>{const li=document.createElement('li');li.textContent=message;return li;}));}
      if(finished&&data.canContinue&&failureCount===0)location.replace(next.href);
    }catch(error){status.textContent=error.message;}
    finally{loading=false;if(!finished&&!document.hidden)timer=setTimeout(poll,1000);}
  };
  document.addEventListener('visibilitychange',()=>{clearTimeout(timer);if(!document.hidden)poll();});
  poll();
})();
