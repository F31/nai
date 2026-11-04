import {useEffect,useState} from 'react';
import type {Invoke} from './client';
import {parseObserveView,unknownDest,unknownObs,type DestSurface,type ObsDetection} from './live-readiness';

export function useObsObserve(enabled:boolean){
 const [view,setView]=useState<{obs:ObsDetection;dest:DestSurface}>({obs:unknownObs(),dest:unknownDest()});
 useEffect(()=>{
  if(!enabled){setView({obs:unknownObs(),dest:unknownDest()});return;}
  const native=window as Window&{__TAURI__?:{core:{invoke:Invoke}}};
  if(!native.__TAURI__){setView({obs:unknownObs(),dest:unknownDest()});return;}
  let active=true,pending=false;
  const invoke=native.__TAURI__.core.invoke;
  function tick(){
   if(pending)return;
   pending=true;
   void invoke('workbench_obs_observe').then(raw=>{
    if(!active)return;
    const parsed=parseObserveView(raw);
    setView(parsed??{obs:unknownObs(),dest:unknownDest()});
   }).catch(()=>{
    if(active)setView({obs:unknownObs(),dest:unknownDest()});
   }).finally(()=>{pending=false;});
  }
  tick();
  const timer=setInterval(tick,2000);
  return()=>{active=false;clearInterval(timer);};
 },[enabled]);
 return view;
}
