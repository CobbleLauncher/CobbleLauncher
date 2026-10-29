// SPDX-License-Identifier: MIT
export function commandCopy(button:HTMLButtonElement,error:HTMLElement,options:{
 write:(text:string)=>Promise<unknown>;
 text:()=>string|null;
 label:(state:'copy'|'copying'|'copied'|'retry'|'failed')=>string;
}){
 let busy=false,epoch=0,timer:ReturnType<typeof setTimeout>|undefined;
 const reset=()=>{++epoch;clearTimeout(timer);error.hidden=true;button.textContent=options.label('copy');button.disabled=busy||options.text()===null;};
 const run=async()=>{
  const text=options.text();if(busy||text===null)return;
  const current=++epoch;clearTimeout(timer);busy=true;error.hidden=true;button.disabled=true;button.textContent=options.label('copying');
  try{
   await options.write(text);
   if(current===epoch){button.textContent=options.label('copied');timer=setTimeout(()=>{button.textContent=options.label('copy');},1800);}
  }catch{
   if(current===epoch){button.textContent=options.label('retry');error.textContent=options.label('failed');error.hidden=false;}
  }finally{busy=false;button.disabled=options.text()===null;}
 };
 button.onclick=()=>void run();
 window.addEventListener('pagehide',reset,{once:true});
 return{reset,run};
}
