// SPDX-License-Identifier: MIT
// Catalog facts adapted from iamcal/enchant-order, MIT Copyright (c) 2021 Cal Henderson.
// Source: https://github.com/iamcal/enchant-order/blob/9345bd097a300266e347d874feb46ef7e1bd4158/data.js
// See ../../README.md and /third-party/licenses/enchant-order-9345bd0/LICENSE.
export const equipment: Record<string, [string,string]> = {
 sword:['Sword','剑'],axe:['Axe','斧'],pickaxe:['Pickaxe','镐'],shovel:['Shovel','锹'],hoe:['Hoe','锄'],
 helmet:['Helmet','头盔'],chestplate:['Chestplate','胸甲'],leggings:['Leggings','护腿'],boots:['Boots','靴子'],
 bow:['Bow','弓'],crossbow:['Crossbow','弩'],trident:['Trident','三叉戟'],fishing_rod:['Fishing rod','钓鱼竿'],
 elytra:['Elytra','鞘翅'],shield:['Shield','盾牌'],shears:['Shears','剪刀'],turtle_shell:['Turtle shell','海龟壳'],
};
export type Enchantment = {id:string;en:string;zh:string;max:number;weight:number;items:string[];conflicts:string[]};
const armor='helmet chestplate leggings boots turtle_shell';
const tools='axe pickaxe shovel hoe';
const all=Object.keys(equipment).join(' ');
const make=(id:string,en:string,zh:string,max:number,weight:number,items:string,conflicts=''):Enchantment=>({id,en,zh,max,weight,items:items.split(' '),conflicts:conflicts?conflicts.split(' '):[]});
export const enchantments:Enchantment[] = [
 make('sharpness','Sharpness','锋利',5,1,'sword axe','smite bane_of_arthropods'),
 make('smite','Smite','亡灵杀手',5,1,'sword axe','sharpness bane_of_arthropods'),
 make('bane_of_arthropods','Bane of Arthropods','节肢杀手',5,1,'sword axe','sharpness smite'),
 make('knockback','Knockback','击退',2,1,'sword'),make('fire_aspect','Fire Aspect','火焰附加',2,2,'sword'),
 make('looting','Looting','抢夺',3,2,'sword'),make('sweeping','Sweeping Edge','横扫之刃',3,2,'sword'),
 make('efficiency','Efficiency','效率',5,1,tools+' shears'),make('fortune','Fortune','时运',3,2,tools,'silk_touch'),
 make('silk_touch','Silk Touch','精准采集',1,4,tools,'fortune'),
 make('protection','Protection','保护',4,1,armor,'blast_protection fire_protection projectile_protection'),
 make('blast_protection','Blast Protection','爆炸保护',4,2,armor,'protection fire_protection projectile_protection'),
 make('fire_protection','Fire Protection','火焰保护',4,1,armor,'protection blast_protection projectile_protection'),
 make('projectile_protection','Projectile Protection','弹射物保护',4,1,armor,'protection blast_protection fire_protection'),
 make('thorns','Thorns','荆棘',3,4,armor),make('respiration','Respiration','水下呼吸',3,2,'helmet turtle_shell'),
 make('aqua_affinity','Aqua Affinity','水下速掘',1,2,'helmet turtle_shell'),
 make('feather_falling','Feather Falling','摔落缓冲',4,1,'boots'),
 make('depth_strider','Depth Strider','深海探索者',3,2,'boots','frost_walker'),
 make('frost_walker','Frost Walker','冰霜行者',2,2,'boots','depth_strider'),
 make('soul_speed','Soul Speed','灵魂疾行',3,4,'boots'),make('swift_sneak','Swift Sneak','迅捷潜行',3,4,'leggings'),
 make('power','Power','力量',5,1,'bow'),make('punch','Punch','冲击',2,2,'bow'),make('flame','Flame','火矢',1,2,'bow'),
 make('infinity','Infinity','无限',1,4,'bow','mending'),
 make('multishot','Multishot','多重射击',1,2,'crossbow','piercing'),make('piercing','Piercing','穿透',4,1,'crossbow','multishot'),
 make('quick_charge','Quick Charge','快速装填',3,1,'crossbow'),
 make('impaling','Impaling','穿刺',5,2,'trident'),make('loyalty','Loyalty','忠诚',3,1,'trident','riptide'),
 make('riptide','Riptide','激流',3,2,'trident','loyalty channeling'),make('channeling','Channeling','引雷',1,4,'trident','riptide'),
 make('luck_of_the_sea','Luck of the Sea','海之眷顾',3,2,'fishing_rod'),make('lure','Lure','饵钓',3,2,'fishing_rod'),
 make('unbreaking','Unbreaking','耐久',3,1,all),make('mending','Mending','经验修补',1,2,all,'infinity'),
 make('binding_curse','Curse of Binding','绑定诅咒',1,4,armor+' elytra'),make('vanishing_curse','Curse of Vanishing','消失诅咒',1,4,all),
];
export const byId=new Map(enchantments.map(e=>[e.id,e]));
export const applicable=(item:string)=>enchantments.filter(e=>e.items.includes(item));
