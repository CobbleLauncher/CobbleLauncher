// SPDX-License-Identifier: MIT
import { locale } from '../i18n';
const en = {
  title: 'Continue on another computer', receive: 'Receive a game', target: 'Computer', content: 'Content', review: 'Review', transfer: 'Transfer', back: 'Back', next: 'Continue', refresh: 'Refresh',
  chooseTarget: 'Choose a computer', chooseContent: 'Choose what to send', reviewSend: 'Ready to send', reviewReceive: 'Receive this game', chooseOffer: 'Games sent to you',
  anotherComputer: 'Add your other computer as a Mesh contact first.', connect: 'Open connections', meshUnavailable: 'Connect with Mesh to transfer a game.', noPeers: 'No contacts yet', noOffers: 'No games waiting', connected: 'Connected', notConnected: 'Not connected',
  saves: 'Worlds', mods: 'Mods', resourcepacks: 'Resource packs', shaderpacks: 'Shader packs', config: 'Mod configuration', options: 'Game options',
  send: 'Send game', accept: 'Receive game', decline: 'Decline', sending: 'Sending game', receiving: 'Receiving game', waiting: 'Waiting for the other computer', preparing: 'Checking files…',
  sender: 'From', recipient: 'To', files: 'Files', size: 'Transfer size', version: 'Minecraft', vanilla: 'Vanilla', name: 'New game name', location: 'Save location', defaultLocation: 'Default games folder', chooseLocation: 'Choose folder',sourceComputer:'Source computer',
  sendConsent: 'Selected files and their names will be shared with this contact. The original game is kept.', receiveConsent: 'An independent game will be created. Your existing games stay unchanged.', codeWarning: 'Mods contain executable code. Receive them only from someone you trust.',
  background: 'Continue in background', cancel: 'Cancel transfer', complete: 'Transfer complete', sent: 'Game sent', received: 'Game received', open: 'Open new game', done: 'Done', cancelled: 'Transfer cancelled', failed: 'Transfer did not finish', interrupted: 'Transfer interrupted', sourceKept: 'The original game is kept.',verifying:'Checking received files',runtimePreparing:'Preparing the game',cancelling:'Cancelling…',
  failedGeneric: 'Could not complete the transfer. Try again.', busyGame: 'Close the source game before sending it.', offline: 'The other computer is not connected. Open connections and try again.', identity: 'Set up Mesh and add the other computer as a contact.', unavailable: 'Transfers are unavailable right now. Check connections.', changed: 'The source files changed. Review the content again.', space: 'Not enough space in the destination.', expired: 'This transfer is no longer available. Ask for a new one.', invalidName: 'Enter a name for the new game.', loading: 'Loading…', tasks: 'Transfers', noTask: 'This transfer is no longer available.',
  discard:'Delete unfinished copy',discardTitle:'Delete this unfinished copy?',discardDescription:'The unfinished local game and received files will be deleted. The source game is kept.',
};
const zh: Record<keyof typeof en,string> = {
  title:'换电脑继续玩',receive:'接收游戏',target:'目标电脑',content:'内容',review:'确认',transfer:'传送',back:'返回',next:'继续',refresh:'刷新',
  chooseTarget:'选择目标电脑',chooseContent:'选择要发送的内容',reviewSend:'确认发送',reviewReceive:'接收这个游戏',chooseOffer:'发给你的游戏',
  anotherComputer:'自己的另一台电脑也需要先添加为 Mesh 联系人。',connect:'打开连接',meshUnavailable:'连接 Mesh 后即可传送游戏。',noPeers:'还没有联系人',noOffers:'暂无待接收的游戏',connected:'已连接',notConnected:'未连接',
  saves:'存档',mods:'Mods',resourcepacks:'资源包',shaderpacks:'光影包',config:'Mod 配置',options:'游戏设置',
  send:'发送游戏',accept:'接收游戏',decline:'拒绝',sending:'正在发送游戏',receiving:'正在接收游戏',waiting:'等待另一台电脑',preparing:'正在检查文件…',
  sender:'来自',recipient:'发送至',files:'文件',size:'传送大小',version:'Minecraft',vanilla:'原版',name:'新游戏名称',location:'保存位置',defaultLocation:'默认游戏目录',chooseLocation:'选择文件夹',sourceComputer:'来源电脑',
  sendConsent:'所选文件及文件名会分享给这位联系人，原游戏保留。',receiveConsent:'将创建独立游戏，已有游戏不变。',codeWarning:'Mod 包含可执行代码，请只接收可信联系人的内容。',
  background:'后台继续',cancel:'取消传送',complete:'传送完成',sent:'游戏已发送',received:'游戏已接收',open:'打开新游戏',done:'完成',cancelled:'已取消传送',failed:'传送未完成',interrupted:'传送已中断',sourceKept:'原游戏保留。',verifying:'正在校验收到的文件',runtimePreparing:'正在准备游戏',cancelling:'正在取消…',
  failedGeneric:'传送未完成，请重试。',busyGame:'请先关闭来源游戏，再发送。',offline:'另一台电脑尚未连接，请打开连接后重试。',identity:'请先设置 Mesh，并添加另一台电脑为联系人。',unavailable:'暂时无法传送，请检查连接。',changed:'来源文件已变化，请重新确认内容。',space:'目标位置空间不足。',expired:'这次传送已不可用，请重新发送。',invalidName:'请填写新游戏名称。',loading:'正在读取…',tasks:'传送任务',noTask:'找不到这次传送。',
  discard:'删除未完成副本',discardTitle:'删除这个未完成副本？',discardDescription:'将删除本机未完成的游戏及已接收文件，来源游戏保留。',
};
export type TransferLabel = keyof typeof en;
export function transferText(key: TransferLabel) { return (locale.value === 'zh-CN' ? zh : en)[key]; }
export function formatTransferBytes(value: number) {
  if (!Number.isFinite(value) || value < 0) return '—';
  const unit = value >= 1073741824 ? 'GB' : value >= 1048576 ? 'MB' : 'KB';
  const divisor = unit === 'GB' ? 1073741824 : unit === 'MB' ? 1048576 : 1024;
  return `${new Intl.NumberFormat(locale.value,{ maximumFractionDigits: 1 }).format(value / divisor)} ${unit}`;
}
