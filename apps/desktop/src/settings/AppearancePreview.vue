<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, useId } from 'vue';
import GameCard from '../GameCard.vue';
import SelectField from '../ui/SelectField.vue';
import SwitchField from '../ui/SwitchField.vue';
import AppIcon from '../ui/AppIcon.vue';
import type { Game, PlayerProfile } from '../games';
import { t } from '../i18n';
import { appearancePreviewStyle, resolvedTheme, type Appearance } from '../theme';
const descriptionId = useId();
const props = defineProps<{ appearance: Appearance }>();
const dark = ref(false);
let media: MediaQueryList | undefined;
function followSystem() { dark.value = media?.matches ?? false; }
onMounted(() => {
  if (typeof matchMedia !== 'function') return;
  media = matchMedia('(prefers-color-scheme: dark)');
  followSystem(); media.addEventListener('change', followSystem);
});
onUnmounted(() => media?.removeEventListener('change', followSystem));
const theme = computed(() => resolvedTheme(props.appearance.theme, dark.value));
const game = computed<Game>(() => ({ id: 'appearance-preview', name: t('settingsLayout.previewGame'), version: '1.21.1', loader_kind: 'fabric', loader_version: '0.16.14', directory: '', status: 'installed', error: null, progress: null }));
const profile: PlayerProfile = { id: 'preview', name: 'Alex', uuid: 'preview', kind: 'offline', note: null };
</script>
<template>
  <section
    class="appearance-preview"
    :aria-label="t('settingsLayout.preview')"
    :aria-describedby="descriptionId"
    @click.capture.stop.prevent
    @contextmenu.capture.stop.prevent
    @keydown.capture.stop.prevent
  >
    <div class="appearance-preview-caption">
      <span class="appearance-preview-title"><AppIcon
        name="monitor"
        :size="16"
      />{{ t('settingsLayout.preview') }}</span>
      <span class="appearance-preview-badge">{{ t('settingsLayout.editor.previewOnly') }}</span>
    </div>
    <p
      :id="descriptionId"
      class="sr-only"
    >
      {{ t('settingsLayout.editor.previewDescription') }}
    </p>
    <div class="appearance-preview-viewport">
      <div
        class="appearance-scope appearance-preview-canvas"
        :data-theme="theme"
        :data-accent="appearance.accent"
        :data-density="appearance.density"
        :data-motion="appearance.motion"
        :style="appearancePreviewStyle(appearance)"
      >
        <div
          inert
          aria-hidden="true"
          class="appearance-preview-content"
        >
          <GameCard
            :game="game"
            :profile="profile"
            :starting="false"
            :retrying="false"
            variant="row"
          />
          <div class="appearance-preview-controls">
            <div class="appearance-preview-field">
              <SelectField
                id="appearance-preview-profile"
                model-value="alex"
                :label="t('settingsLayout.previewProfile')"
                :options="[{ value: 'alex', label: 'Alex' }]"
              />
            </div>
            <div class="appearance-preview-switch">
              <span>{{ t('settingsLayout.previewBackups') }}</span><SwitchField
                :model-value="true"
                :label="t('settingsLayout.previewBackups')"
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  </section>
</template>
<style scoped>
.appearance-preview{border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-lg);overflow:hidden;margin-bottom:var(--ui-space-5)}
.appearance-preview-caption{display:flex;justify-content:space-between;align-items:center;padding:10px 16px;color:var(--ui-text-muted);font-size:var(--ui-text-caption);border-bottom:1px solid var(--ui-border);background:var(--ui-surface-2)}
.appearance-preview-title{display:flex;align-items:center;gap:var(--ui-space-2)}.appearance-preview-badge{padding:3px 8px;border:1px solid var(--ui-border);border-radius:var(--ui-radius-sm);background:var(--ui-surface-1);font-weight:var(--ui-weight-medium)}
.appearance-preview-viewport{height:auto;cursor:default;user-select:none}.appearance-preview-content{pointer-events:none}
.appearance-preview-canvas{min-height:100%;background:var(--ui-bg);color:var(--ui-text);font-family:var(--ui-font-sans);font-size:var(--ui-text-body);box-sizing:border-box}
.appearance-preview-content{padding:var(--ui-space-4);display:grid;gap:var(--ui-space-3)}
.appearance-preview-controls{display:grid;grid-template-columns:minmax(0,1fr) minmax(0,1fr);gap:var(--ui-space-5);align-items:end}
.appearance-preview-field{display:grid;gap:var(--ui-space-2);font-size:var(--ui-text-label)}
.appearance-preview-switch{display:flex;justify-content:space-between;align-items:center;gap:var(--ui-space-3);font-size:var(--ui-text-label);min-height:var(--ui-control-height)}
@media(max-width:400px){.appearance-preview-controls{grid-template-columns:1fr}}
@media(min-width:1200px){.appearance-preview-viewport{height:auto;min-height:160px}.appearance-preview-content{grid-template-columns:minmax(0,2fr) minmax(180px,1fr);align-items:center;gap:var(--ui-space-4)}.appearance-preview-controls{grid-template-columns:1fr;gap:var(--ui-space-3)}}
</style>
