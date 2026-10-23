<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { t } from './i18n';
import { computed, ref, watch } from 'vue';
import type { CurseForgePackPreview } from './curseforge';
import CurseForgeDiscover from './CurseForgeDiscover.vue';
import DiscoverPacksPage from './DiscoverPacksPage.vue';
import DiscoverMods from './DiscoverMods.vue';
import DiscoverPacks from './DiscoverPacks.vue';
import { gameLoaderLabel, type Game, type PackPreview } from './games';
import { marketGames, marketGameLocked, type MarketKind } from './market';
import AppIcon from './ui/AppIcon.vue';
import { marketTarget, rememberMarketTarget } from './market-preferences';

const props = defineProps<{ games: Game[]; initialKind: MarketKind; initialGameId?: string; initialQuery?: string }>();
const emit = defineEmits<{ busy: [value: boolean]; selectedPack: [preview: PackPreview]; selectedCfPack: [preview: CurseForgePackPreview]; installed: [gameId: string]; locate: [gameId: string, kind: MarketKind] }>();
const categories: Exclude<MarketKind, 'blueprint'>[] = ['modpack', 'mod', 'resource_pack', 'shader_pack'];
const kind = ref<MarketKind>(props.initialKind);
const gameId = ref(marketTarget(props.games, props.initialKind, props.initialGameId));
const busy = ref(false);
const provider = ref('modrinth');
const installedName = ref('');
const status = computed(() => installedName.value ? t('market.installed', { name: installedName.value }) : '');
function categoryLabel(category: MarketKind) {
  const key = category === 'resource_pack' ? 'resourcePack' : category === 'shader_pack' ? 'shaderPack' : category;
  return t(`market.${key}`);
}
const targets = computed(() => marketGames(props.games, kind.value));
const game = computed(() => targets.value.find(game => game.id === gameId.value));
const browseComponent = computed(() => kind.value === 'modpack'
  ? provider.value === 'curseforge' ? CurseForgeDiscover : DiscoverPacksPage
  : kind.value === 'mod' ? provider.value === 'curseforge' ? CurseForgeDiscover : DiscoverMods : DiscoverPacks);
const browseKey = computed(() => `${kind.value}-${provider.value}-${gameId.value}`);
const browseProps = computed(() => {
  if (kind.value === 'modpack') return provider.value === 'curseforge' ? { kind: 'modpack' } : {};
  const locked = game.value ? marketGameLocked(game.value) : false;
  if (kind.value === 'mod') return provider.value === 'curseforge' ? { kind: 'mod', game: game.value, locked, initialQuery: props.initialQuery } : { game: game.value, locked, initialQuery: props.initialQuery };
  return { gameId: gameId.value, kind: kind.value, locked, browseOnMount: true };
});
function setBusy(value: boolean) { busy.value = value; emit('busy', value); }
function installed() {
  if (!game.value) return;
  installedName.value = game.value.name;
  emit('installed', game.value.id);
}
function selectKind(value: MarketKind) {
  if (!busy.value && kind.value !== value) {
    kind.value = value;
    gameId.value = marketTarget(props.games, value);
  }
}
watch([() => props.initialKind, () => props.initialGameId], () => {
  kind.value = props.initialKind; gameId.value = marketTarget(props.games, props.initialKind, props.initialGameId); installedName.value = '';
});
watch([kind, gameId], () => { installedName.value = ''; });
watch(targets, () => { if (gameId.value && !game.value) gameId.value = ''; });
function rememberTarget() { rememberMarketTarget(kind.value, gameId.value); }
</script>

<template>
  <div class="page-content market-page">
    <div class="market-layout">
      <aside class="market-sidebar">
        <nav
          class="market-category-list"
          :aria-label="t('market.content')"
        >
          <button
            v-for="category in categories"
            :key="category"
            :aria-pressed="kind === category"
            :disabled="busy"
            @click="selectKind(category)"
          >
            {{ categoryLabel(category) }}
          </button>
        </nav>
        <details
          v-if="kind === 'mod' || kind === 'modpack'"
          class="market-source-filter"
        >
          <summary><AppIcon name="settings" />{{ t('market.sources') }}</summary>
          <div class="market-source-filter-content form-field">
            <label for="market-provider-desktop">{{ t('curseforge.provider') }}</label>
            <select
              id="market-provider-desktop"
              v-model="provider"
              :disabled="busy"
            >
              <option value="modrinth">
                Modrinth
              </option>
              <option value="curseforge">
                CurseForge
              </option>
            </select>
          </div>
        </details>
        <div class="market-filter-fields">
          <div
            v-if="kind !== 'modpack' && kind !== 'blueprint'"
            class="form-field"
          >
            <label for="market-game-desktop">{{ t('market.target') }}</label>
            <select
              id="market-game-desktop"
              v-model="gameId"
              :disabled="busy"
              @change="rememberTarget"
            >
              <option value="">
                {{ t('market.chooseGame') }}
              </option>
              <option
                v-for="target in targets"
                :key="target.id"
                :value="target.id"
              >
                {{ target.name }} · {{ target.version }} · {{ gameLoaderLabel(target) }}
              </option>
            </select>
          </div>
        </div>
      </aside>

      <section class="market-results">
        <div class="market-mobile-controls">
          <label class="market-mobile-category">
            <span class="visually-hidden">{{ t('market.content') }}</span>
            <select
              :value="kind"
              :disabled="busy"
              @change="selectKind(($event.target as HTMLSelectElement).value as MarketKind)"
            >
              <option
                v-for="category in categories"
                :key="category"
                :value="category"
              >{{ categoryLabel(category) }}</option>
            </select>
          </label>
          <details class="market-mobile-filters">
            <summary><AppIcon name="settings" />{{ t('market.filters') }}</summary>
            <div class="market-mobile-filter-content">
              <div
                v-if="kind === 'mod' || kind === 'modpack'"
                class="form-field"
              >
                <label for="market-provider-mobile">{{ t('curseforge.provider') }}</label>
                <select
                  id="market-provider-mobile"
                  v-model="provider"
                  :disabled="busy"
                >
                  <option value="modrinth">
                    Modrinth
                  </option>
                  <option value="curseforge">
                    CurseForge
                  </option>
                </select>
              </div>
              <div
                v-if="kind !== 'modpack' && kind !== 'blueprint'"
                class="form-field"
              >
                <label for="market-game-mobile">{{ t('market.target') }}</label>
                <select
                  id="market-game-mobile"
                  v-model="gameId"
                  :disabled="busy"
                  @change="rememberTarget"
                >
                  <option value="">
                    {{ t('market.chooseGame') }}
                  </option>
                  <option
                    v-for="target in targets"
                    :key="target.id"
                    :value="target.id"
                  >
                    {{ target.name }} · {{ target.version }} · {{ gameLoaderLabel(target) }}
                  </option>
                </select>
              </div>
            </div>
          </details>
        </div>

        <div class="market-workflow">
          <p
            v-if="kind !== 'modpack' && !targets.length"
            class="content-empty"
          >
            {{ kind === 'mod' ? t('market.noModGame') : t('market.noGame') }}
          </p>
          <KeepAlive :max="8">
            <component
              :is="browseComponent"
              v-if="kind === 'modpack' || game"
              :key="browseKey"
              v-bind="browseProps"
              @busy="setBusy"
              @installing="setBusy"
              @installed="installed"
              @selected-pack="emit('selectedPack', $event)"
              @selected-cf-pack="emit('selectedCfPack', $event)"
            />
          </KeepAlive>
          <p
            v-if="status"
            role="status"
            class="setting-hint"
          >
            {{ status }}
            <button
              class="text-button"
              @click="emit('locate', gameId, kind)"
            >
              {{ t('market.viewInstalled') }}
            </button>
          </p>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.market-layout{display:grid;grid-template-columns:158px minmax(0,1fr);gap:24px;align-items:start}.market-sidebar{display:flex;flex-direction:column;gap:21px;min-width:0}.market-category-list{display:grid;gap:3px}.market-category-list button{min-height:40px;padding:9px 10px;border:0;border-radius:8px;background:transparent;color:var(--muted);text-align:left;font:inherit;font-size:13px}.market-category-list button:hover{background:var(--hover);color:var(--ink)}.market-category-list button[aria-pressed=true]{background:var(--hover);color:var(--ink);font-weight:600}.market-category-list button:disabled{opacity:.6}.market-filter-fields{display:grid;gap:14px}.market-filter-fields .form-field{min-width:0}.market-filter-fields label,.market-mobile-filter-content label{display:block;margin-bottom:6px;font-size:12px;color:var(--muted)}.market-filter-fields select,.market-mobile-filter-content select{width:100%;min-width:0;min-height:40px;padding:9px 10px;border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--ink);font:inherit;font-size:13px}.market-results{min-width:0}.market-workflow{min-width:0}.market-mobile-controls{display:none}.market-page :deep(.discover-list){display:grid;gap:8px}.market-page :deep(.discover-packs-page){padding:0}.market-page :deep(.curseforge-discover){padding-top:0}@media(max-width:680px){.market-layout{display:block}.market-sidebar{display:none}.market-mobile-controls{display:grid;grid-template-columns:minmax(0,1fr) auto;align-items:start;gap:8px;margin-bottom:12px}.market-mobile-category select{width:100%;min-width:0;min-height:38px;padding:8px 10px;border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--ink);font:inherit;font-size:13px}.market-mobile-filters{position:relative}.market-mobile-filters>summary{display:flex;align-items:center;justify-content:center;gap:7px;max-width:220px;min-height:38px;padding:7px 10px;border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--muted);font-size:12px;cursor:pointer;list-style:none;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.market-mobile-filters>summary::-webkit-details-marker{display:none}.market-mobile-filters>summary svg{width:15px;height:15px;flex:none}.market-mobile-filter-content{position:absolute;z-index:5;right:0;top:calc(100% + 5px);display:grid;gap:13px;width:min(280px,calc(100vw - 48px));padding:14px;border:1px solid var(--line);border-radius:10px;background:var(--panel);box-shadow:0 10px 28px #0002}.market-mobile-filter-content .form-field{min-width:0}}
.market-source-filter{position:relative}.market-source-filter>summary{display:flex;align-items:center;gap:7px;width:fit-content;min-height:34px;padding:6px 8px;border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--muted);font-size:12px;cursor:pointer;list-style:none}.market-source-filter>summary::-webkit-details-marker{display:none}.market-source-filter>summary svg{width:14px;height:14px}.market-source-filter-content{position:absolute;z-index:5;left:0;top:calc(100% + 5px);width:190px;padding:12px;border:1px solid var(--line);border-radius:10px;background:var(--panel);box-shadow:0 10px 28px #0002}.market-source-filter-content label{display:block;margin-bottom:6px;font-size:12px;color:var(--muted)}.market-source-filter-content select{width:100%;min-height:40px;padding:9px 10px;border:1px solid var(--line);border-radius:8px;background:var(--panel);color:var(--ink);font:inherit;font-size:13px}
.market-page .visually-hidden{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}
</style>

