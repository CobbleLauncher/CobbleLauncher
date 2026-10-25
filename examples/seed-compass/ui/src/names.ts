// SPDX-License-Identifier: MIT

export type CompassLocale = 'en' | 'zh-CN';
export type CompassNameKind = 'biome' | 'structure';

const biomeNames: Record<string, [string, string]> = {
  wooded_mountains: ['Wooded Mountains', '繁茂的山地'], desert_lakes: ['Desert Lakes', '沙漠湖泊'],
  gravelly_mountains: ['Gravelly Mountains', '砂砾山地'], modified_gravelly_mountains: ['Modified Gravelly Mountains', '变种砂砾山地'],
  taiga_mountains: ['Taiga Mountains', '针叶林山地'], snowy_taiga_mountains: ['Snowy Taiga Mountains', '积雪针叶林山地'], swamp_hills: ['Swamp Hills', '沼泽丘陵'],
  modified_jungle: ['Modified Jungle', '变种丛林'], modified_jungle_edge: ['Modified Jungle Edge', '变种丛林边缘'],
  tall_birch_hills: ['Tall Birch Hills', '高大桦木丘陵'], giant_spruce_taiga_hills: ['Giant Spruce Taiga Hills', '巨型云杉针叶林丘陵'],
  modified_wooded_badlands_plateau: ['Modified Wooded Badlands Plateau', '变种繁茂恶地高原'], modified_badlands_plateau: ['Modified Badlands Plateau', '变种恶地高原'],
  the_void: ['The Void', '虚空'],
  ocean: ['Ocean', '海洋'], plains: ['Plains', '平原'], desert: ['Desert', '沙漠'], mountains: ['Mountains', '山地'],
  forest: ['Forest', '森林'], taiga: ['Taiga', '针叶林'], swamp: ['Swamp', '沼泽'], river: ['River', '河流'],
  nether_wastes: ['Nether Wastes', '下界荒地'], the_end: ['The End', '末地'], frozen_ocean: ['Frozen Ocean', '冻洋'],
  frozen_river: ['Frozen River', '冻河'], snowy_plains: ['Snowy Plains', '雪原'], snowy_tundra: ['Snowy Tundra', '积雪冻原'],
  snowy_mountains: ['Snowy Mountains', '雪山'], mushroom_fields: ['Mushroom Fields', '蘑菇岛'], mushroom_field_shore: ['Mushroom Field Shore', '蘑菇岛岸'],
  beach: ['Beach', '沙滩'], snowy_beach: ['Snowy Beach', '积雪沙滩'], desert_hills: ['Desert Hills', '沙漠丘陵'], wooded_hills: ['Wooded Hills', '森林丘陵'],
  taiga_hills: ['Taiga Hills', '针叶林丘陵'], mountain_edge: ['Mountain Edge', '山地边缘'], jungle: ['Jungle', '丛林'], jungle_hills: ['Jungle Hills', '丛林丘陵'],
  jungle_edge: ['Jungle Edge', '丛林边缘'], sparse_jungle: ['Sparse Jungle', '稀疏丛林'], deep_ocean: ['Deep Ocean', '深海'], stony_shore: ['Stony Shore', '石岸'],
  stone_shore: ['Stone Shore', '石岸'], birch_forest: ['Birch Forest', '桦木森林'], birch_forest_hills: ['Birch Forest Hills', '桦木森林丘陵'],
  dark_forest: ['Dark Forest', '黑森林'], dark_forest_hills: ['Dark Forest Hills', '黑森林丘陵'], snowy_taiga: ['Snowy Taiga', '积雪针叶林'],
  snowy_taiga_hills: ['Snowy Taiga Hills', '积雪针叶林丘陵'], old_growth_pine_taiga: ['Old Growth Pine Taiga', '原始松木针叶林'],
  giant_tree_taiga: ['Giant Tree Taiga', '巨型针叶林'], old_growth_spruce_taiga: ['Old Growth Spruce Taiga', '原始云杉针叶林'],
  giant_spruce_taiga: ['Giant Spruce Taiga', '巨型云杉针叶林'], giant_tree_taiga_hills: ['Giant Tree Taiga Hills', '巨型针叶林丘陵'],
  savanna: ['Savanna', '热带草原'], savanna_plateau: ['Savanna Plateau', '热带高原'], windswept_savanna: ['Windswept Savanna', '风袭热带草原'],
  badlands: ['Badlands', '恶地'], wooded_badlands: ['Wooded Badlands', '疏林恶地'], wooded_badlands_plateau: ['Wooded Badlands Plateau', '繁茂恶地高原'],
  badlands_plateau: ['Badlands Plateau', '恶地高原'], eroded_badlands: ['Eroded Badlands', '风蚀恶地'], small_end_islands: ['Small End Islands', '末地小型岛屿'],
  end_midlands: ['End Midlands', '末地中岛'], end_highlands: ['End Highlands', '末地高岛'], end_barrens: ['End Barrens', '末地荒地'],
  warm_ocean: ['Warm Ocean', '暖水海洋'], lukewarm_ocean: ['Lukewarm Ocean', '温水海洋'], cold_ocean: ['Cold Ocean', '冷水海洋'],
  deep_warm_ocean: ['Deep Warm Ocean', '深暖水海洋'], deep_lukewarm_ocean: ['Deep Lukewarm Ocean', '深温水海洋'], deep_cold_ocean: ['Deep Cold Ocean', '深冷水海洋'],
  deep_frozen_ocean: ['Deep Frozen Ocean', '深冻洋'], sunflower_plains: ['Sunflower Plains', '向日葵平原'], ice_spikes: ['Ice Spikes', '冰刺之地'],
  flower_forest: ['Flower Forest', '繁花森林'], old_growth_birch_forest: ['Old Growth Birch Forest', '原始桦木森林'], tall_birch_forest: ['Tall Birch Forest', '高大桦木森林'],
  old_growth_birch_forest_hills: ['Old Growth Birch Forest Hills', '原始桦木森林丘陵'], windswept_hills: ['Windswept Hills', '风袭丘陵'],
  windswept_forest: ['Windswept Forest', '风袭森林'], windswept_gravelly_hills: ['Windswept Gravelly Hills', '风袭砂砾丘陵'],
  shattered_savanna: ['Shattered Savanna', '破碎的热带草原'], shattered_savanna_plateau: ['Shattered Savanna Plateau', '破碎的热带高原'],
  bamboo_jungle: ['Bamboo Jungle', '竹林'], bamboo_jungle_hills: ['Bamboo Jungle Hills', '竹林丘陵'], soul_sand_valley: ['Soul Sand Valley', '灵魂沙峡谷'],
  crimson_forest: ['Crimson Forest', '绯红森林'], warped_forest: ['Warped Forest', '诡异森林'], basalt_deltas: ['Basalt Deltas', '玄武岩三角洲'],
  dripstone_caves: ['Dripstone Caves', '钟乳石洞穴'], lush_caves: ['Lush Caves', '繁茂洞穴'], meadow: ['Meadow', '草甸'], grove: ['Grove', '雪林'],
  snowy_slopes: ['Snowy Slopes', '积雪山坡'], stony_peaks: ['Stony Peaks', '裸岩山峰'], jagged_peaks: ['Jagged Peaks', '尖峭山峰'], frozen_peaks: ['Frozen Peaks', '冰封山峰'],
  deep_dark: ['Deep Dark', '深暗之域'], mangrove_swamp: ['Mangrove Swamp', '红树林沼泽'], cherry_grove: ['Cherry Grove', '樱花树林'], pale_garden: ['Pale Garden', '苍白之园'],
};

const structureNames: Record<string, [string, string]> = {
  desert_pyramid: ['Desert Pyramid', '沙漠神殿'], jungle_pyramid: ['Jungle Pyramid', '丛林神庙'], swamp_hut: ['Swamp Hut', '沼泽小屋'],
  igloo: ['Igloo', '雪屋'], village: ['Village', '村庄'], ocean_ruin: ['Ocean Ruin', '海底废墟'], shipwreck: ['Shipwreck', '沉船'],
  monument: ['Ocean Monument', '海底神殿'], mansion: ['Woodland Mansion', '林地府邸'], pillager_outpost: ['Pillager Outpost', '掠夺者前哨站'],
  ruined_portal: ['Ruined Portal', '废弃传送门'], ruined_portal_nether: ['Ruined Portal (Nether)', '下界废弃传送门'], ancient_city: ['Ancient City', '远古城市'],
  fortress: ['Nether Fortress', '下界要塞'], bastion_remnant: ['Bastion Remnant', '堡垒遗迹'], end_city: ['End City', '末地城'],
  trail_ruins: ['Trail Ruins', '古迹废墟'], trial_chambers: ['Trial Chambers', '试炼密室'],
};

function humanize(id: string): string {
  return id.replace(/^minecraft:/, '').replace(/[_-]+/g, ' ').trim().replace(/\b\w/g, letter => letter.toUpperCase()) || id;
}

/** Display labels only; retain the original identifier for all engine requests. */
export function displayName(id: string, locale: CompassLocale, kind: CompassNameKind): string {
  const key = id.replace(/^minecraft:/, '');
  const known = (kind === 'biome' ? biomeNames : structureNames)[key];
  return known ? known[locale === 'zh-CN' ? 1 : 0] : humanize(key);
}
