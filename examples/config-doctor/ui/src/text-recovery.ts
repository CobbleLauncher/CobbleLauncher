// SPDX-License-Identifier: MIT
export type LineChange={kind:'added'|'removed';line:number;text:string};

/** Bounded LCS line diff for text review; large inputs fall back to the full preview. */
export function lineChanges(beforeText:string, currentText:string):LineChange[]|null{
  const before=beforeText.split(/\r?\n/u),current=currentText.split(/\r?\n/u);
  if(before.length>1000||current.length>1000)return null;
  const width=current.length+1;
  const lengths=new Uint16Array((before.length+1)*width);
  for(let i=before.length-1;i>=0;i--)for(let j=current.length-1;j>=0;j--){
    lengths[i*width+j]=before[i]===current[j]?lengths[(i+1)*width+j+1]+1:Math.max(lengths[(i+1)*width+j],lengths[i*width+j+1]);
  }
  const changes:LineChange[]=[];let i=0,j=0;
  while(i<before.length||j<current.length){
    if(i<before.length&&j<current.length&&before[i]===current[j]){i++;j++;continue;}
    if(i<before.length&&(j===current.length||lengths[(i+1)*width+j]>=lengths[i*width+j+1])){changes.push({kind:'removed',line:i+1,text:before[i]});i++;}
    else{changes.push({kind:'added',line:j+1,text:current[j]});j++;}
  }
  return changes;
}
