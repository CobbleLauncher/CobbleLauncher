// SPDX-License-Identifier: MIT
const escape=value=>String(value).replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;');
const box=(x,y,w,title,sub,active=false)=>`<rect x="${x}" y="${y}" width="${w}" height="76" rx="7" class="node ${active?'active-node':''}"/><text x="${x+w/2}" y="${y+31}" text-anchor="middle">${escape(title)}</text><text x="${x+w/2}" y="${y+54}" text-anchor="middle" class="small">${escape(sub)}</text>`;
const text=(x,y,s,cls='small')=>`<text x="${x}" y="${y}" text-anchor="middle" class="${cls}">${escape(s)}</text>`;
const line=(d,cls='wire')=>`<path d="${d}" class="${cls}"/>`;
function svg(id,title,body,height){return `<svg viewBox="0 0 960 ${height}" role="img" aria-labelledby="${id}-title"><title id="${id}-title">${escape(title)}</title>${body}</svg>`;}
function figure(id,title,body,caption,controls=''){return `<figure class="figure" id="${id}" data-view="all">${controls}<p class="diagram-hint">${title.match(/[\u4e00-\u9fff]/)?'左右滑动查看全图':'Scroll sideways to explore'} ↔</p><div class="diagram" tabindex="0" role="region" aria-label="${escape(title)}">${body}</div><figcaption>${caption}</figcaption></figure>`;}
export function meshContent(lang){
 const en=lang==='en';const t=(zh,enText)=>en?enText:zh;
 const header=(id,n,title)=>`<section class="mesh-section" id="${id}"><div class="mesh-heading"><span class="number">${n}</span><h2>${title}</h2></div>`;
 const prose=(...p)=>`<div class="mesh-prose">${p.map(s=>`<p>${s}</p>`).join('')}</div>`;
 const title=t('联机功能<br>与工作原理','Multiplayer<br>and how it works');
 const intro=t('Cobble Mesh 是圆石的可选联机层：本地身份、签名邀请、节点发现与游戏流桥接。基于 libp2p，把连接细节留给启动器。','Cobble Mesh is Cobble’s optional multiplayer layer: local identities, signed invitations, node discovery and game-stream bridges. Built on libp2p, with connection details handled by the launcher.');
 const nav=[['architecture',t('网络拓扑','Topology')],['identity',t('身份与信任','Identity')],['bridge',t('游戏连接','Game streams')],['groups',t('多人小组','Groups')],['protocols',t('协议与接口','Protocols')],['status',t('状态与进展','Status')]];
 let out=`<div class="mesh-lead"><div class="page-lead"><p class="section-index">Cobble Mesh</p><h1>${title}</h1><p>${intro}</p><p class="page-meta">${t('功能与原理','Features and architecture')} · 2026.10.25</p></div><div class="mesh-prose"><p>${t('目前的游戏入口采用显式中继。直连、AutoNAT 与 DCUtR 已有基础组件，仍在接入和验证。','Current game workflows use explicit relays. Direct connections, AutoNAT and DCUtR have underlying components and remain under integration and validation.')}</p></div></div><nav class="mesh-index" aria-label="${t('本页目录','On this page')}">${nav.map(([id,name],i)=>`<a href="#${id}"><span>0${i+1}</span>${name}</a>`).join('')}</nav>`;
 out+=header('architecture','01',t('节点发现与中继连接','Peer discovery and relays'));
 out+=prose(t('目录与游戏流量走两条路径。节点目录提供引导信息；启动器使用已配置的种子，建立经过身份认证的长期连接。目录暂时为空，仍可通过已知种子加入网络。','The directory and game traffic take separate paths. A node directory provides bootstrap information; the launcher authenticates a persistent connection to configured seeds. An empty directory does not prevent connections through known seeds.'));
 const topology=svg('topology-svg',t('节点目录与游戏流量拓扑','Directory and game traffic topology'),
 '<g class="control-plane">'+box(355,20,250,t('节点目录','Node directory'),'discovery / bootstrap')+
 line('M 355 58 H 140 V 195','discovery-wire')+line('M 605 58 H 820 V 195','discovery-wire')+
 text(140,113,t('节点目录','node directory'))+text(820,113,t('已知种子','known seeds'))+'</g>'+
 box(30,195,220,t('玩家 A · 启动器','Player A · launcher'),'local identity + bridge',true)+box(710,195,220,t('玩家 B · 启动器','Player B · launcher'),'local identity + bridge',true)+
 '<g class="data-plane">'+box(355,195,250,t('中继节点','Relay node'),'authenticated libp2p streams')+
 line('M 250 233 H 355')+line('M 605 233 H 710')+
 text(480,310,t('经中继建立的对端流','Relayed peer streams'))+'</g>'+
 line('M 140 271 V 362 H 820 V 271','muted-wire')+text(480,387,t('后续选路：经验证的直连 / DCUtR','Further routing: verified direct paths / DCUtR')),410);
 out+=figure('topology',t('网络拓扑','Network topology'),topology,t('实线：当前游戏中继路径。棕色虚线：节点发现与引导。底部虚线：仍在接入验证的直连路径。节点目录不转发游戏数据。','Solid: current game relay path. Brown dashed: discovery and bootstrap. Bottom dashed: direct routing under integration. The directory does not forward game data.'),`<div class="layer-controls">${[['all',t('全部路径','All paths')],['control',t('发现','Discovery')],['data',t('游戏流量','Game traffic')]].map(([id,label])=>`<button type="button" data-layer="${id}" aria-pressed="${id==='all'}">${label}</button>`).join('')}</div>`);
 out+='</section>';
 out+=header('identity','02',t('本地身份与签名验证','Local identity and signature verification'));
 out+=prose(t('身份在本地生成并保存。显示名称可以修改；完整 PeerId 与密钥关联，连接和签名验证以它为准。名称后的短后缀便于区分，不是全球唯一用户名。','An identity is generated and stored locally. The display name can change; the full PeerId is tied to the key and is used for connections and signature verification. The short suffix helps distinguish names; it is not a globally unique username.'));
 out+=figure('identity-figure',t('本地身份与签名档案','Local identity and signed profile'),svg('identity-svg',t('密钥生成 PeerId，PeerId 生成短后缀，档案通过私钥签名','The key derives a PeerId, the PeerId derives a short suffix, and the private key signs the profile'),
 box(25,38,235,t('本地 Ed25519 密钥','Local Ed25519 key'),'private key stays local',true)+line('M 260 76 H 340')+
 box(340,38,240,'PeerId',t('完整身份标识','full identity identifier'))+line('M 580 76 H 660')+box(660,38,275,t('名称 # 短后缀','name # short suffix'),'SHA-256(PeerId bytes)[:6]')+
 line('M 142 114 V 220 H 340')+box(340,182,440,'SignedProfile','name · revision · issued_at · expires_at',true)+text(180,207,t('签名','sign'))+text(560,295,'cobble-mesh/user-profile/v1'),320),
 t('短后缀使用 6 字节摘要，显示为 12 位十六进制。签名档案带有效期，验证时检查签名与时间。','The short suffix uses six digest bytes, displayed as 12 hexadecimal characters. Signed profiles carry an expiry and are checked for signature and time validity.'));
 out+=prose(t('签名证明“这段信息由控制该密钥的一方发出”，不证明真实姓名、唯一自然人或长期信誉。临时身份可以重新创建，因此身份本身不能阻止刷号。切换身份会改变当前会话使用的密钥，原身份与联系人仍可保留。','A signature proves that a message came from a party controlling the key, not a real name, unique person or lasting reputation. Temporary identities can be recreated, so identity alone does not prevent Sybil accounts. Switching identity changes the active session key while allowing the original identity and contacts to be retained.'))+'</section>';
 out+=header('bridge','03',t('通过邀请建立游戏连接','Connect through game invitations'));
 out+=prose(t('房主在 Minecraft 内开放局域网，确认端口后创建邀请。朋友接受邀请，启动器建立本地 TCP 入口；Minecraft 只需连接这个本地地址。桥接的目标限定在房主批准的游戏端口。','The host opens Minecraft to LAN and confirms the port to create an invitation. A friend accepts it, and the launcher exposes a local TCP endpoint. Minecraft connects to that local address; the bridge targets only the game port approved by the host.'));
 let seq='';const columns=[100,350,610,860];const nodes=[t('加入者的游戏','Guest game'),t('加入者启动器','Guest launcher'),t('房主启动器','Host launcher'),t('局域网世界','LAN world')];
 columns.forEach((x,i)=>{seq+=box(x-90,18,180,nodes[i],i===1||i===2?'Mesh bridge':'Minecraft');seq+=line(`M ${x} 94 V 370`,'muted-wire');});
 const steps=[[350,610,136,t('验证邀请 · PeerId / proof / expiry','Validate invite · PeerId / proof / expiry')],[350,610,205,t('经中继打开已认证的游戏流','Open authenticated game stream via relay')],[100,350,272,t('连接本地地址','Connect locally')],[610,860,272,t('连接批准的端口','Approved port')],[100,860,343,t('Minecraft TCP 字节流','Minecraft TCP byte stream')]];
 steps.forEach(([a,b,y,label],i)=>{seq+=line(`M ${a} ${y} H ${b-7}`)+`<path d="M ${b-7} ${y-4} l 7 4 -7 4 Z" class="arrow"/>`+text((a+b)/2,y-12,label);});
 out+=figure('bridge-figure',t('游戏桥接时序','Game bridge sequence'),svg('bridge-svg',t('邀请验证后，经中继连接两端的本地游戏端口','After invitation validation, a relay connects the local game endpoints'),seq,395),t('游戏数据路径：加入者 Minecraft → 本地桥接 → Mesh 中继流 → 房主桥接 → 已批准的局域网端口。','Data path: guest Minecraft → local bridge → Mesh relay stream → host bridge → approved LAN port.'));
 out+=`<div class="tech-grid"><div class="tech-item"><h3>${t('邀请验证','Invitation verification')}</h3><p>${t('邀请带身份、用途与有效期。接收端验证证明，再建立对应的游戏连接。不要将邀请发布到不可信的位置。','An invitation carries identity, purpose and expiry. The recipient verifies its proof before opening the corresponding game connection. Share invitations with trusted recipients.')}</p></div><div class="tech-item"><h3>${t('连接路径验证','Connection route verification')}</h3><p>${t('服务在线、预约中继成功和游戏流已建立是不同阶段。Inspector 记录连接标识、活动流与字节数，而不是仅凭一个直连事件判断游戏路径。','A service connection, relay reservation and established game stream are distinct stages. Inspector uses connection identifiers, active streams and byte counts instead of inferring a game route from one direct-connection event.')}</p></div></div></section>`;
 out+=header('groups','04',t('房主与多人小组','Hosts and multiplayer groups'));
 out+=prose(t('当前多人小组以房主为中心，支持房主和两位加入者。每位加入者获得自己的本地桥接地址，连接同一个房主游戏。它是应用层游戏小组，不创建虚拟网卡或整段局域网。','Current groups center on one host and support the host plus two guests. Each guest receives a local bridge address into the host’s game. This is an application-level game group, with no virtual adapter or shared LAN subnet.'));
 out+=figure('group-figure',t('三人游戏小组','Three-player group'),svg('group-svg',t('两位加入者分别经中继连接房主，房主连接局域网世界','Two guests connect through relay streams to the host, who connects to the LAN world'),
 box(30,30,225,t('朋友 A','Friend A'),'local TCP bridge')+box(30,175,225,t('朋友 B','Friend B'),'local TCP bridge')+
 line('M 255 68 H 345 V 140 H 440')+line('M 255 213 H 345 V 140')+text(345,275,t('独立中继流','separate relay streams'))+
 box(440,102,220,t('房主 · 小组','Host · group'),'host-owned session',true)+line('M 660 140 H 720')+
 box(720,102,215,t('游戏世界','Game world'),'approved LAN port'),300),t('当前容量：3 人。会话有生命周期；房主结束小组会结束对应连接。此图描述游戏应用拓扑，不代表任意成员之间已有全互联链路。','Current capacity: three people. Sessions have a lifecycle; ending the host group ends its connections. This is the game topology, not an all-to-all network between members.'))+'</section>';
 out+=header('protocols','05',t('传输协议与插件接口','Transport protocols and plugin APIs'));
 out+=prose(t('Rust / Tokio 负责后台运行，libp2p 提供身份、连接和流管理。TCP 传输使用 Noise 认证与 Yamux 多路复用。协议名称区分身份观察、游戏流与小组流。','Rust and Tokio run the background service; libp2p provides identity, connections and streams. TCP uses Noise authentication and Yamux multiplexing. Protocol names separate identification, game streams and group streams.'));
 const protocols=[
 ['Identify','/cobblemesh/identify/1',t('对端身份与连接信息','Peer identity and connection information')],
 [t('游戏流','Game stream'),'/cobblemesh/game-stream/0.1.0',t('邀请约束下的游戏桥接','Invitation-scoped game bridge')],
 [t('小组流','Group stream'),'/cobblemesh/group-stream/0.1.0/{peer}',t('按对端区分的小组桥接','Per-peer group bridge')],
 [t('插件应用事件','Plugin application events'),'/cobblemesh/events/1.0.0',t('认证联系人与精确命名空间收发权限','Authenticated contacts and exact namespace send/receive grants')],
 [t('签名域','Signature domain'),'cobble-mesh/user-profile/v1',t('公开档案签名的域分隔','Domain separation for profile signatures')]
 ];
 out+=`<table class="protocol-table"><thead><tr><th>${t('用途','Purpose')}</th><th>${t('协议 / 域','Protocol / domain')}</th><th>${t('边界','Scope')}</th></tr></thead><tbody>${protocols.map(row=>`<tr><td>${row[0]}</td><td><code>${row[1]}</code></td><td>${row[2]}</td></tr>`).join('')}</tbody></table>`;
 out+=`<div class="tech-grid"><div class="tech-item"><h3>${t('连接状态快照','Connection snapshots')}</h3><p>${t('原生服务推送状态快照，界面订阅 mesh-network-changed。检测结果和真实会话证据分别保存。','The native service emits snapshots through mesh-network-changed. Probe results and actual session evidence are recorded separately.')}</p></div><div class="code-window"><pre>phase · generation · attempt · retry_at
services · checks · last_error
session_route · session_evidence
  peer_scope · selected_connection
  bytes_sent · bytes_received
  active_streams · completed_streams</pre></div></div>`;
 out+=prose(t('TypeScript SDK 已提供精确命名空间的 on / send 应用事件。插件只能与已批准的联系人通信，收发权限独立声明；路由及中继策略由宿主决定。两个隔离原生插件视图已完成真实公网中继双向联调，送达回执表示对端宿主接收，不表示玩家已读。','The TypeScript SDK provides on / send application events within exact namespaces. Plugins communicate only with approved contacts, declare send and receive grants separately, and leave routing and relay policy to the host. Two isolated native plugin views have exchanged events through an actual public relay. A receipt means the remote host accepted the message, not that a player read it.'),t('公网应用事件与游戏／小组预约由宿主协调暂停和恢复。路由变化后，插件按 SDK 状态重建订阅；不承诺离线重放或恰好一次执行。','The host pauses and restores public application events around game/group reservations. After a route change, plugins rebuild subscriptions from SDK status; offline replay and exactly-once execution are not promised.'))+'</section>';
 out+=header('status','06',t('连接状态与功能进展','Connection status and development'));
 out+=prose(t('开启在线模式后，后台自动连接，不阻塞首页。首轮最多尝试三次，每次 10 秒，重试间隔为 2 秒和 5 秒并加少量抖动。失败后逐步延长后台重试间隔；关闭 Mesh 会取消当前连接与重试。','Online mode starts background connections without blocking the home screen. The first round makes up to three attempts, with a ten-second budget each, two- and five-second retry gaps, and jitter. Later retries back off. Turning Mesh off cancels active connections and retries.'));
 out+=figure('states-figure',t('后台连接状态','Background connection states'),svg('states-svg',t('离线模式开启后连接，成功在线，断开重连，失败继续后台恢复','Enabling online mode connects, authenticates, reconnects after disconnects, and recovers after failures'),
 box(15,50,180,t('离线','Offline'),'mode off')+line('M 195 88 H 270')+box(270,50,190,t('连接中','Connecting'),'attempt 1 → 3')+line('M 460 88 H 545')+box(545,50,175,t('在线','Online'),'authenticated',true)+line('M 720 88 H 775')+box(775,50,170,t('重连中','Reconnecting'),'connection lost')+
 line('M 365 126 V 235 H 545','muted-wire')+box(545,197,250,t('连接失败','Connection failed'),'background backoff')+
 line('M 795 235 H 860 V 126','muted-wire')+text(280,259,t('失败后继续后台恢复','recover in the background')),300),t('关闭模式会使当前连接代次失效，迟到的事件不能把状态重新设为在线。在线只表示已有认证服务连接，具体游戏路径另行记录。','Disabling the mode invalidates the connection generation, so late events cannot restore online state. Online means an authenticated service connection; the actual game route is tracked separately.'));
 const rows=[
 [t('已接入','Integrated'),t('本地签名身份、引导目录、长期认证连接、显式中继、游戏桥接、三人小组、Inspector 与精确授权的插件应用事件。','Local signed identities, bootstrap directory, persistent authenticated connections, explicit relay, game bridges, three-person groups, Inspector and scoped plugin application events.')],
 [t('继续验证','Under validation'),t('真实 Minecraft 多人小组完整流程、更多网络拓扑与断线恢复。页面中的结构图不是实时服务状态。','End-to-end real Minecraft group sessions, additional network topologies and reconnect behavior. These diagrams are architecture, not live service status.')],
 [t('接入中的能力','Integration work'),t('AutoNAT 外部地址验证、直达观察入口、DCUtR 与正式游戏入口的自动选路。双 NAT 公网打洞仍需要独立环境验证。','AutoNAT address verification, a direct observer endpoint, DCUtR and automatic route selection in game workflows. Public double-NAT hole punching still needs an independent test topology.')],
 [t('后续方向','Future directions'),t('节点软件构建版本展示与独立更新器仍属后续计划。已有协议版本与 Identify 能力信息；VLAN、群组聊天与评分网络不属于当前交付。','Node software build display and an independent updater remain planned. Protocol versions and Identify capability information already exist; VLANs, group chat and reputation networks are outside the current delivery.')]
 ];
 out+=`<ul class="status-list">${rows.map(([a,b])=>`<li><b>${a}</b><span>${b}</span></li>`).join('')}</ul></section><div class="mesh-end"><span class="mesh-prose">${t('继续读代码，也欢迎一起把协议磨好。','Explore the source. Help refine the protocol.')}</span><a class="button" href="https://github.com/libp2p/specs">${t('libp2p 协议文档','libp2p specifications')} ↗</a></div>`;
 return out;
}
