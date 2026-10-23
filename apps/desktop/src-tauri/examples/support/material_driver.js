// SPDX-License-Identifier: MIT
// This driver only operates the real bundled Vue UI and observes native results.
localStorage.setItem('cobble-language','en-US');
window.addEventListener('DOMContentLoaded',async()=>{
 const invoke=(command,args)=>window.__TAURI_INTERNALS__.invoke(command,args);
 const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
 async function wait(test){for(let i=0;i<800;i++){const result=await test();if(result)return result;await pause(25);}throw Error('material driver timeout');}
 const button=text=>[...document.querySelectorAll('button')].find(node=>node.textContent.trim()===text);
 const proof={passed:false};
 try{
  await wait(()=>button('Tools'));button('Tools').click();
  const tool=await wait(()=>[...document.querySelectorAll('button')].find(node=>node.textContent.includes('Blueprint library')));tool.click();
  const item=await wait(()=>document.querySelector('.blueprint-open'));proof.libraryName=item.textContent.trim();item.click();
  await wait(()=>document.querySelector('.texture-controls [role="status"]')?.textContent.includes('Textures for 2 /'));
  const first=await wait(async()=>{const state=await invoke('get_app_info');return state.saves>=1?state:null;});
  proof.texturedStatus=document.querySelector('.texture-controls [role="status"]').textContent;
  const checkbox=document.querySelector('.texture-controls input');proof.enabled=checkbox.checked;
  // A second material application exercises replacement and the automatic Vue thumbnail refresh.
  checkbox.click();await pause(100);checkbox.click();
  await wait(async()=>{const state=await invoke('get_app_info');return state.saves>first.saves;});
  proof.canvasSize={width:document.querySelector('.blueprint-viewer canvas').width,height:document.querySelector('.blueprint-viewer canvas').height};
  button('Close preview').click();
  const image=await wait(()=>[...document.querySelectorAll('.blueprint-library img')].find(image=>image.complete&&image.naturalWidth>0));
  proof.thumbnailDisplayed=!!image;proof.thumbnailSize={width:image.naturalWidth,height:image.naturalHeight};
  proof.passed=proof.enabled&&proof.thumbnailDisplayed&&proof.canvasSize.width>0;
 }catch(error){proof.error=String(error);proof.body=document.body.innerText.slice(-5000);}
 await invoke('get_app_info',{proof});
});
