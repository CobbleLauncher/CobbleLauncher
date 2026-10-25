// SPDX-License-Identifier: MIT
// Actual optional repository tools; artwork uses the licensed local Lucide sprite.
export const communityTools = [
  ['coordinate-book','compass','坐标簿','Coordinate Book','#d5e5aa','#587344'],
  ['geometry-builder','shapes','建筑几何助手','Geometry Builder','#e6b68c','#8d582e'],
  ['seed-compass','mountain','种子罗盘','Seed Compass','#a8c7ad','#3c6952'],
  ['schematic-viewer','blocks','蓝图预览','Schematic Viewer','#a6c4d1','#446b82'],
  ['crash-doctor','scan-line','Crash Doctor','Crash Doctor','#d4b5ba','#854c58'],
  ['material-planner','layers','材料清单','Material Planner','#d6ce9f','#7e7241'],
  ['world-chronicle','history','世界编年史','World Chronicle','#b6bacf','#5a6081'],
  ['config-doctor','code-2','配置医生','Config Doctor','#b7d1cd','#427e76'],
  ['anvil-planner','hammer','铁砧规划','Anvil Planner','#bcbfb1','#62694c'],
  ['screenshot-timeline','image','截图时间线','Screenshot Timeline','#d0b9d4','#7d5486'],
  ['command-pocket','terminal','命令口袋','Command Pocket','#a7c2b2','#315f48'],
  ['storage-lens','hard-drive','存储透镜','Storage Lens','#d0c2a8','#806c4d'],
  ['mod-conflicts','git-compare-arrows','Mod 关系图','Mod Relations','#b8cbd5','#4b6e82'],
  ['backup-auditor','shield-check','备份检查','Backup Auditor','#c3d2a3','#658048'],
  ['lang-doctor','languages','语言包医生','Language Doctor','#d7bfa2','#987447'],
  ['session-card','users-round','游玩小记','Session Card','#d9b4a4','#9a6250'],
  ['world-dashboard','globe','世界仪表盘','World Dashboard','#aac7c3','#39706a'],
  ['pack-metadata','coffee','整合包信息','Pack Metadata','#c3bbd2','#73618a'],
];
export function renderCommunity(container, lang) {
  const document=container.ownerDocument;
  container.replaceChildren(...communityTools.map(([id,icon,zh,en,tile,glyph])=>{
    const link=document.createElement('a'); link.className='tool-icon';
    link.href=`https://github.com/CobbleLauncher/CobbleLauncher/tree/main/examples/${id}`;
    link.target='_blank';link.rel='noopener noreferrer';link.style.setProperty('--tile',tile);link.style.setProperty('--glyph',glyph);
    const art=document.createElement('span');art.className='tool-glyph';art.setAttribute('aria-hidden','true');
    const svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.classList.add('icon');
    const use=document.createElementNS('http://www.w3.org/2000/svg','use');use.setAttribute('href',`./icons.svg#${icon}`);svg.append(use);art.append(svg);
    const name=document.createElement('span');name.className='tool-name';name.textContent=lang==='zh'?zh:en;
    link.append(art,name);return link;
  }));
}
