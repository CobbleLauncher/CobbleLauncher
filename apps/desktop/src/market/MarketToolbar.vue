<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed } from 'vue';
import SearchField from '../ui/SearchField.vue';
import SelectField from '../ui/SelectField.vue';
import MarketVersionFilter from './MarketVersionFilter.vue';
import { PopoverRoot, PopoverTrigger, PopoverPortal, PopoverContent, RadioGroupRoot, RadioGroupItem } from 'reka-ui';
import AppIcon from '../ui/AppIcon.vue';
import { loaderLabel, type Game } from '../games';
import type { MarketKind } from '../market';
import { t } from '../i18n';
const props = defineProps<{ kind: MarketKind; categories: { value: MarketKind; label: string }[]; targets: Game[]; gameId: string; provider: string; query: string; minecraftVersion: string; loader: string; busy: boolean; cfAvailable: boolean | null }>();
const emit = defineEmits<{ kind: [value: MarketKind]; target: [value: string]; provider: [value: string]; query: [value: string]; version: [value: string]; loader: [value: string]; search: [] }>();
function selectVersion(version: string) { emit('version', version); emit('search'); }
const loaderOptions = computed(() => [{ value:'',label:t('market.anyLoader') },{value:'fabric',label:'Fabric'},{value:'forge',label:'Forge'},{value:'neoforge',label:'NeoForge'}]);
const providerOptions = computed(() => [{value:'all',label:t('market.allSources')},{value:'modrinth',label:'Modrinth'},{value:'curseforge',label:props.cfAvailable === false ? t('market.sourceUnavailable',{source:'CurseForge'}) : 'CurseForge',disabled:props.cfAvailable === false}]);
</script>
<template>
  <div class="market-layout">
    <aside class="market-sidebar">
      <nav
        class="market-categories market-category-list"
        :aria-label="t('market.content')"
      >
        <button
          v-for="category in categories"
          :key="category.value"
          type="button"
          :aria-pressed="kind === category.value"
          :disabled="busy"
          @click="$emit('kind', category.value)"
        >
          {{ category.label }}
        </button>
      </nav>
      <div
        v-if="kind === 'modpack'"
        class="market-pack-filters market-filter-fields"
      >
        <div class="market-filter-field">
          <span>{{ t('createGame.version') }}</span><MarketVersionFilter
            :model-value="minecraftVersion"
            :installed-versions="targets.map(game => game.version)"
            :disabled="busy"
            @update:model-value="selectVersion"
          />
        </div>
        <label class="market-filter-field"><span>{{ t('createGame.type') }}</span><SelectField
          id="market-filter-loader-wide"
          :model-value="loader"
          :label="t('createGame.type')"
          :disabled="busy"
          :options="loaderOptions"
          @update:model-value="$emit('loader', $event)"
        /></label>
      </div>
    </aside>
    <div class="market-main">
      <div class="market-controlbar">
        <div class="market-target-bar">
          <template v-if="kind !== 'modpack'">
            <label for="market-game-desktop">{{ t('market.target') }}</label><SelectField
              id="market-game-desktop"
              :model-value="gameId"
              :label="t('market.target')"
              :disabled="busy"
              :options="[{value:'',label:t('market.chooseGame')}, ...targets.map(game => ({value:game.id,label:`${game.name} · ${game.version} · ${game.loader_version ? loaderLabel(game.loader_kind ?? 'fabric') : t('home.vanilla')}`}))]"
              @update:model-value="$emit('target', $event)"
            />
          </template><span
            v-else
            class="market-new-game-target"
          >{{ t('market.newGameTarget') }}</span>
        </div>
        <form
          class="market-query-row"
          @submit.prevent="$emit('search')"
        >
          <SearchField
            id="market-search"
            :model-value="query"
            :label="t('market.searchContent')"
            :placeholder="t('market.searchContent')"
            :disabled="busy"
            maxlength="120"
            @update:model-value="$emit('query', $event)"
            @submit="$emit('search')"
          /><button
            type="submit"
            class="secondary-button"
            :disabled="busy"
          >
            {{ t('curseforge.search') }}
          </button>
          <div
            v-if="kind === 'mod' || kind === 'modpack'"
            class="market-source-control"
          >
            <PopoverRoot>
              <PopoverTrigger as-child>
                <button
                  type="button"
                  class="text-button market-source-trigger"
                  :disabled="busy"
                  :aria-label="t('market.sources')"
                >
                  <AppIcon name="network" />{{ provider === 'all' ? t('market.sources') : provider === 'curseforge' ? 'CurseForge' : 'Modrinth' }}<AppIcon name="chevron-down" />
                </button>
              </PopoverTrigger>
              <PopoverPortal>
                <PopoverContent
                  class="market-source-popover"
                  align="end"
                  :side-offset="8"
                  :collision-padding="12"
                >
                  <RadioGroupRoot
                    :model-value="provider"
                    :aria-label="t('market.sources')"
                    :disabled="busy"
                    class="market-source-options"
                    @update:model-value="$emit('provider', String($event))"
                  >
                    <RadioGroupItem
                      v-for="option in providerOptions"
                      :key="option.value"
                      :value="option.value"
                      :disabled="option.disabled"
                      class="market-source-option"
                    >
                      <span>{{ option.label }}</span><AppIcon
                        v-if="provider === option.value"
                        name="check"
                      />
                    </RadioGroupItem>
                  </RadioGroupRoot>
                </PopoverContent>
              </PopoverPortal>
            </PopoverRoot>
          </div>
        </form>
      </div>
      <div class="market-main-content">
        <slot />
      </div>
    </div>
  </div>
</template>
<style scoped>
.market-layout{display:grid;grid-template-columns:176px minmax(0,1fr);gap:var(--ui-space-5);align-items:start}.market-sidebar{min-width:0;position:sticky;top:var(--ui-space-3)}.market-categories{display:grid;gap:var(--ui-space-1)}.market-categories button{min-height:var(--ui-control-height);padding:var(--ui-space-2) var(--ui-space-3);border:0;border-radius:var(--ui-radius-md);background:transparent;color:var(--ui-text-muted);font:inherit;font-size:var(--ui-text-label);text-align:left}.market-categories button[aria-pressed=true]{color:var(--ui-text);background:var(--tint);font-weight:600}.market-categories button:hover{background:var(--ui-surface-hover)}.market-main{min-width:0}.market-controlbar{display:grid;grid-template-columns:minmax(0,1fr) minmax(220px,320px);align-items:center;gap:var(--ui-space-4);margin-bottom:var(--ui-space-3)}.market-query-row{display:flex;align-items:center;gap:var(--ui-space-2);grid-column:1;grid-row:1;min-width:0}.market-query-row .search-field{flex:1;min-width:0}.market-target-bar{grid-column:2;grid-row:1;min-width:0;font-size:var(--ui-text-label)}.market-target-bar>label{display:none}.market-target-bar :deep(.select-field-trigger){width:100%;min-width:0}.market-new-game-target{display:block;color:var(--ui-text-muted);text-align:right}.market-filter-fields{display:grid;gap:var(--ui-space-3);padding-top:var(--ui-space-4)}.market-pack-filters{margin-top:var(--ui-space-4);border-top:1px solid var(--ui-border)}.market-filter-field{display:grid;gap:var(--ui-space-2);font-size:var(--ui-text-label);min-width:0}.market-filter-field>span{color:var(--ui-text-muted)}.market-source-control{display:flex;flex:none}.market-source-trigger{gap:var(--ui-space-2);font-size:var(--ui-text-caption);min-height:var(--ui-control-height)}
@media(max-width:999px){.market-layout{display:flex;flex-direction:column;gap:var(--ui-space-3)}.market-sidebar{position:static;width:100%}.market-main{width:100%}.market-categories{display:flex;flex-wrap:wrap;border-bottom:1px solid var(--ui-border);padding-bottom:var(--ui-space-3)}.market-pack-filters{grid-template-columns:repeat(2,minmax(0,1fr));margin-top:var(--ui-space-3);padding-top:0;border-top:0}.market-controlbar{grid-template-columns:minmax(0,1fr)}.market-target-bar{grid-column:1;grid-row:1}.market-query-row{grid-column:1;grid-row:2}.market-new-game-target{text-align:left}}
</style>
<style>
.market-source-popover{z-index:80;min-width:220px;max-width:calc(100vw - 24px);padding:var(--ui-space-1);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);box-shadow:var(--ui-shadow-popup,0 8px 24px #0002)}.market-source-options{display:grid}.market-source-option{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-3);min-height:var(--ui-control-height);padding:var(--ui-space-2) var(--ui-space-3);border:0;border-radius:var(--ui-radius-sm);background:transparent;color:inherit;font:inherit;font-size:var(--ui-text-label);text-align:left}.market-source-option:hover:not(:disabled){background:var(--ui-surface-hover)}.market-source-option[data-state=checked]{background:var(--tint);font-weight:600}.market-source-option:focus-visible{outline:var(--ui-focus-ring);outline-offset:-2px}.market-source-option:disabled{opacity:.5}
</style>
