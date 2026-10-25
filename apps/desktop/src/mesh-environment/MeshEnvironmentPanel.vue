<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import {computed,onUnmounted,ref,watch} from 'vue';
import {invoke} from '@tauri-apps/api/core';
import {writeText} from '@tauri-apps/plugin-clipboard-manager';
import LoadingIndicator from '../ui/LoadingIndicator.vue';
import SettingsDisclosure from '../ui/settings/SettingsDisclosure.vue';
import {Boxes,Gamepad2,UserRound,ArrowLeftRight,Check,ArrowRight,SlidersHorizontal,Keyboard,HardDrive,Download,FolderPlus,TriangleAlert,FileCode2,CheckCheck} from '@lucide/vue';
import {readDownloadSettings} from '../download-settings';
import {environmentText as text} from './labels';
import type {PreparedEnvironment,ReceivedEnvironment,SharePreview,Requirements} from './types';
const props=defineProps<{gameId:string;hosting:boolean;invitation?:string;disabled?:boolean}>();
const emit=defineEmits<{invitation:[value:string];preparing:[gameId:string,invitation:string];created:[gameId:string];busy:[value:boolean]}>();
const receivedInvitation=ref('');
const share=ref<SharePreview|null>(null),received=ref<ReceivedEnvironment|null>(null),prepared=ref<PreparedEnvironment|null>(null),selected=ref<string[]>([]),draft=ref(''),published=ref(''),copied=ref(false),busy=ref(false),error=ref(''),name=ref(''),created=ref('');
let generation=0;
const ready=computed(()=>!!prepared.value?.preflight.requirements.ready&&!!prepared.value.preflight.space.can_proceed&&!busy.value&&!props.disabled&&!!name.value.trim());
const document=computed(()=>received.value?.document??share.value?.document??null);
const invitation=computed(()=>props.invitation?.trim()||draft.value.trim());
function report(cause:unknown){const code=String(cause);if(code.includes('LOCAL_OPTIONS_CHANGED')){prepared.value=null;error.value=text('personalChanged');return;}error.value=text(/SOURCE_CHANGED/.test(code)?'changed':/EXPIRED/.test(code)?'expired':/NOT_ATTACHED/.test(code)?'notAttached':/DIRECT_PATH_REQUIRED|OFFICIAL_COORDINATION|ROUTE_DENIED/.test(code)?'direct':/PLAN_CHANGED|DIGEST/.test(code)?'hash':/EXCLUDED_MODS/.test(code)?'excludedBlock':/MANUAL_FILE_MISMATCH/.test(code)?'fileMismatch':/SPACE_INSUFFICIENT/.test(code)?'storage':'failed');}
async function run(work:()=>Promise<void>){if(busy.value||props.disabled)return;const token=generation;busy.value=true;error.value='';try{await work();}catch(cause){if(token===generation)report(cause);}finally{if(token===generation)busy.value=false;}}
async function sharePreview(){await run(async()=>{const token=generation,value=await invoke<SharePreview>('mesh_environment_share_preview',{gameId:props.gameId});if(token!==generation)return;share.value=value;selected.value=[];published.value='';});}
async function publish(){if(!share.value)return;await run(async()=>{const token=generation,value=await invoke<{invitation:string}>('mesh_environment_share_publish',{previewId:share.value!.preview_id,configIds:[...selected.value]});if(token!==generation)return;published.value=value.invitation;copied.value=false;emit('invitation',value.invitation);});}
async function receive(){if(!invitation.value||invitation.value.length>6000)return;await run(async()=>{const token=generation,selectedInvitation=invitation.value,value=await invoke<ReceivedEnvironment>('mesh_environment_receive',{invitation:selectedInvitation,compareGameId:props.gameId||null});if(token!==generation)return;receivedInvitation.value=selectedInvitation;received.value=value;prepared.value=null;name.value=value.document.name;});}
async function prepare(){if(!received.value)return;await run(async()=>{const token=generation,source=await readDownloadSettings(),value=await invoke<PreparedEnvironment>('mesh_environment_prepare',{previewId:received.value!.preview_id,source,useDefaultDirectory:true});if(token===generation)prepared.value=value;});}
async function manual(projectId:string,fileId:string){if(!prepared.value)return;await run(async()=>{const token=generation,value=await invoke<Requirements|null>('mesh_environment_manual_file',{planId:prepared.value!.plan_id,projectId,fileId});if(token===generation&&value&&prepared.value)prepared.value.preflight.requirements=value;});}
async function create(){if(!ready.value||!prepared.value)return;const invitationForGame=receivedInvitation.value;await run(async()=>{const token=generation,source=await readDownloadSettings(),game=await invoke<{id:string}>('mesh_environment_create',{planId:prepared.value!.plan_id,name:name.value.trim(),source,useDefaultDirectory:true});if(token!==generation)return;created.value=game.id;prepared.value=null;emit('preparing',game.id,invitationForGame);emit('created',game.id);});}
async function copy(){await run(async()=>{await writeText(published.value);copied.value=true;});}
async function project(projectId:string){await run(async()=>{await invoke('open_curseforge_project',{projectId});});}
const bytes=(value:number)=>`${(value/1073741824).toFixed(1)} GiB`;
watch(busy,value=>emit('busy',value),{flush:'sync'});
watch(()=>[props.gameId,props.hosting],()=>{++generation;share.value=received.value=prepared.value=null;selected.value=[];published.value=created.value=error.value=receivedInvitation.value='';busy.value=false;});
watch(()=>props.invitation,()=>{if(!props.hosting&&props.invitation&&!busy.value)void receive();},{immediate:true});
onUnmounted(()=>{++generation;emit('busy',false);});
defineExpose({receive,canLeave:()=>!busy.value});
</script>
<template>
  <section
    class="mesh-environment"
    :aria-label="text('title')"
    :aria-busy="busy"
  >
    <button
      v-if="hosting&&!share"
      class="secondary-button"
      type="button"
      :disabled="busy||disabled"
      @click="sharePreview"
    >
      <Boxes
        :size="18"
        aria-hidden="true"
      />{{ text('share') }}
    </button>
    <form
      v-if="!hosting&&!received"
      class="environment-invitation"
      @submit.prevent="receive"
    >
      <label v-if="!props.invitation">{{ text('invitation') }}<textarea
        v-model="draft"
        :placeholder="text('paste')"
        :disabled="busy||disabled"
        maxlength="6000"
        rows="3"
      /></label>
      <button
        class="secondary-button"
        type="submit"
        :disabled="busy||disabled||!invitation"
      >
        {{ text('read') }}
      </button>
    </form>
    <LoadingIndicator
      v-if="busy"
      :label="text('busy')"
      compact
    />
    <p
      v-if="error"
      class="environment-error environment-alert"
      role="alert"
    >
      <TriangleAlert
        :size="18"
        aria-hidden="true"
      />{{ error }}
    </p>
    <template v-if="document">
      <header class="environment-identity">
        <div class="environment-game-icon">
          <Boxes
            :size="29"
            aria-hidden="true"
          />
        </div>
        <div class="environment-game-title">
          <h2>{{ document.name }}</h2>
          <div class="environment-metadata">
            <span><Gamepad2
              :size="14"
              aria-hidden="true"
            />Minecraft {{ document.game_version }}</span><span>{{ document.loader??text('vanilla') }} {{ document.loader_version }}</span><span
              v-if="received"
              :title="received.host_peer_id"
            ><UserRound
              :size="14"
              aria-hidden="true"
            />{{ received.host_peer_id.slice(0, 8) }}…{{ received.host_peer_id.slice(-6) }}</span>
          </div>
        </div>
      </header>
      <div
        class="environment-layout"
        :class="{'environment-layout-install':received&&!created}"
      >
        <div class="environment-review">
          <section
            v-if="received?.comparison?.available"
            class="environment-comparison"
            :aria-label="text('comparison')"
          >
            <h3>
              <ArrowLeftRight
                :size="18"
                aria-hidden="true"
              />{{ text('comparison') }}
            </h3>
            <div class="environment-compare-row">
              <span>{{ text('version') }}</span><span
                class="environment-compare-value"
                :class="received.comparison.version_matches?'is-match':'is-different'"
              ><Check
                v-if="received.comparison.version_matches"
                :size="15"
                aria-hidden="true"
              /><ArrowRight
                v-else
                :size="15"
                aria-hidden="true"
              />{{ text(received.comparison.version_matches?'matches':'differs') }}</span>
            </div>
            <div class="environment-compare-row">
              <span>{{ text('loader') }}</span><span
                class="environment-compare-value"
                :class="received.comparison.loader_matches?'is-match':'is-different'"
              ><Check
                v-if="received.comparison.loader_matches"
                :size="15"
                aria-hidden="true"
              /><ArrowRight
                v-else
                :size="15"
                aria-hidden="true"
              />{{ text(received.comparison.loader_matches?'matches':'differs') }}</span>
            </div>
            <div class="environment-compare-row">
              <span>{{ text('matched') }}</span><span class="environment-compare-count"><strong>{{ received.comparison.matching_mods }}<span> / {{ received.comparison.required_mods }}</span></strong><small v-if="received.comparison.additional_mods">+{{ received.comparison.additional_mods }} {{ text('localMods') }}</small></span>
            </div>
            <div
              v-if="received.comparison.required_configs"
              class="environment-compare-row"
            >
              <span>{{ text('configs') }}</span><strong class="environment-compare-count">{{ received.comparison.matching_configs }}<span> / {{ received.comparison.required_configs }}</span></strong>
            </div>
          </section>
          <div class="environment-files">
            <SettingsDisclosure
              v-if="document.mods.length"
              class="environment-count-disclosure"
              :title="text('mods')"
              icon="puzzle"
              :value="String(document.mods.length)"
            >
              <ul class="environment-mods">
                <li
                  v-for="mod in document.mods"
                  :key="`${mod.source}:${mod.project_id}`"
                >
                  <span>{{ mod.name }}</span><small>{{ mod.source }} · {{ mod.version_id }}</small>
                </li>
              </ul>
            </SettingsDisclosure>
            <SettingsDisclosure
              v-if="received?.document.configs.length"
              class="environment-count-disclosure"
              :title="text('configs')"
              :value="String(received.document.configs.length)"
            >
              <template #icon>
                <SlidersHorizontal
                  :size="18"
                  aria-hidden="true"
                />
              </template>
              <div
                v-for="config in received.document.configs"
                :key="config.path"
                class="environment-config-text"
              >
                <strong>{{ config.path }}</strong><pre>{{ config.text }}</pre>
              </div>
            </SettingsDisclosure>
            <SettingsDisclosure
              v-if="share?.configs.length"
              :title="text('configs')"
              :value="`${selected.length} / ${share.configs.length}`"
              default-open
            >
              <template #icon>
                <SlidersHorizontal
                  :size="18"
                  aria-hidden="true"
                />
              </template>
              <fieldset
                class="environment-configs"
                :aria-label="text('configs')"
              >
                <div
                  v-for="config in share.configs"
                  :key="config.id"
                >
                  <label><input
                    v-model="selected"
                    type="checkbox"
                    :value="config.id"
                    :disabled="busy||disabled||!!published"
                  >{{ config.name }}</label><SettingsDisclosure :title="text('reviewConfig')">
                    <template #icon>
                      <FileCode2
                        :size="16"
                        aria-hidden="true"
                      />
                    </template><pre>{{ config.text }}</pre>
                  </SettingsDisclosure>
                </div>
              </fieldset>
            </SettingsDisclosure>
          </div>
          <aside
            v-if="document.excluded.length"
            class="environment-excluded"
          >
            <TriangleAlert
              :size="19"
              aria-hidden="true"
            /><div>
              <strong>{{ text('excluded') }}</strong><ul>
                <li
                  v-for="file in document.excluded"
                  :key="file"
                >
                  {{ file }}
                </li>
              </ul><p>{{ text('excludedBlock') }}</p>
            </div>
          </aside>
          <div
            v-if="hosting&&share"
            class="environment-share-action"
          >
            <button
              v-if="!published"
              class="primary-button"
              type="button"
              :disabled="busy||disabled"
              @click="publish"
            >
              {{ text('createInvite') }}<ArrowRight
                :size="17"
                aria-hidden="true"
              />
            </button><button
              v-else
              class="secondary-button"
              type="button"
              :disabled="busy||disabled"
              @click="copy"
            >
              {{ text(copied?'copied':'copy') }}
            </button>
          </div>
        </div>
        <section
          v-if="received&&!created"
          class="environment-install"
          :aria-label="text('newGame')"
        >
          <h3>
            <FolderPlus
              :size="20"
              aria-hidden="true"
            />{{ text('newGame') }}
          </h3>
          <template v-if="!prepared">
            <p class="environment-hint">
              {{ text('sourceSafe') }}
            </p><button
              class="primary-button environment-create"
              type="button"
              :disabled="busy||disabled||!!document.excluded.length"
              @click="prepare"
            >
              <Download
                :size="18"
                aria-hidden="true"
              />{{ text('prepare') }}
            </button>
          </template>
          <template v-else>
            <div
              v-if="prepared.personal"
              class="environment-personal"
              role="status"
            >
              <Keyboard
                :size="20"
                aria-hidden="true"
              /><div>
                <strong>{{ text('personal') }}</strong><p v-if="prepared.personal.keys || prepared.personal.graphics">
                  {{ prepared.personal.keys }} {{ text('personalKeys') }} · {{ prepared.personal.graphics }} {{ text('personalGraphics') }}<small v-if="prepared.personal.skipped">{{ prepared.personal.skipped }} {{ text('personalSkipped') }}</small>
                </p><p v-else>
                  {{ text(prepared.personal.reason === 'differentVersion' ? 'personalVersion' : prepared.personal.reason === 'unsupportedFormat' ? 'personalUnsupported' : 'personalUnavailable') }}
                </p>
              </div>
            </div>
            <div
              v-for="volume in prepared.preflight.space.volumes"
              :key="volume.volume.id"
              class="environment-space"
            >
              <HardDrive
                :size="20"
                aria-hidden="true"
              /><div>
                <strong>{{ volume.volume.label }}</strong><dl>
                  <div><dt>{{ text('space') }}</dt><dd>{{ bytes(volume.additional_peak_bytes) }}</dd></div><div v-if="volume.volume.available_bytes!==null">
                    <dt>{{ text('available') }}</dt><dd>{{ bytes(volume.volume.available_bytes) }}</dd>
                  </div>
                </dl>
              </div>
            </div>
            <p
              v-if="!prepared.preflight.space.can_proceed"
              class="environment-error"
            >
              {{ text('storage') }}
            </p><p
              v-else-if="!prepared.preflight.space.exact"
              class="environment-hint"
            >
              {{ text('sizeUnknown') }}
            </p>
            <article
              v-for="file in prepared.preflight.requirements.missing"
              :key="`${file.project_id}:${file.file_id}`"
              class="environment-manual"
            >
              <div class="environment-manual-title">
                <Download
                  :size="18"
                  aria-hidden="true"
                /><strong>{{ file.name }}</strong>
              </div><span>{{ file.file_name }}</span><div class="environment-manual-actions">
                <button
                  type="button"
                  class="text-button"
                  :disabled="busy||disabled"
                  @click="project(file.project_id)"
                >
                  {{ text('project') }}
                </button><button
                  type="button"
                  class="secondary-button"
                  :disabled="busy||disabled"
                  @click="manual(file.project_id,file.file_id)"
                >
                  {{ text('manual') }}
                </button>
              </div>
            </article>
            <div
              v-if="prepared.preflight.requirements.ready"
              class="environment-ready"
            >
              <CheckCheck
                :size="17"
                aria-hidden="true"
              />{{ text('ready') }}
            </div>
            <div class="environment-destination">
              <label class="environment-name">{{ text('name') }}<input
                v-model="name"
                maxlength="80"
                :disabled="busy||disabled"
              ></label><button
                class="primary-button environment-create"
                type="button"
                :disabled="!ready"
                @click="create"
              >
                <FolderPlus
                  :size="18"
                  aria-hidden="true"
                />{{ text('create') }}
              </button>
            </div>
          </template>
        </section>
      </div>
      <p
        v-if="created"
        class="environment-created"
        role="status"
      >
        <CheckCheck
          :size="20"
          aria-hidden="true"
        />{{ text('installing') }}
      </p>
    </template>
  </section>
</template>
<style scoped>
.mesh-environment{min-width:0;color:var(--ui-text);font-size:var(--ui-text-body,14px);line-height:var(--ui-leading-body,1.6);display:grid;gap:20px;margin-top:24px}.environment-invitation,.environment-invitation label,.environment-name{display:grid;gap:8px}.environment-invitation textarea,.environment-name input{font:inherit;color:var(--ui-text);background:var(--ui-surface-1);border:1px solid var(--ui-border);border-radius:var(--ui-radius-md,8px);padding:10px 12px;min-width:0;width:100%;box-sizing:border-box}.environment-identity{display:flex;align-items:center;gap:16px;padding-bottom:22px;border-bottom:1px solid var(--ui-border)}.environment-game-icon{display:grid;place-items:center;width:58px;height:58px;border-radius:14px;flex:none;background:color-mix(in srgb,var(--ui-accent) 25%,var(--ui-surface-1));color:var(--ui-success)}.environment-game-title{min-width:0}.environment-game-title h2{margin:0 0 7px;font-size:22px;line-height:1.3;font-weight:650;letter-spacing:-.5px;overflow-wrap:anywhere}.environment-metadata{display:flex;gap:8px 18px;align-items:center;flex-wrap:wrap;color:var(--ui-text-muted);font-size:12px}.environment-metadata span{display:inline-flex;align-items:center;gap:6px}.environment-layout{display:grid;min-width:0;gap:32px}.environment-layout-install{grid-template-columns:minmax(0,1.15fr) minmax(0,1fr)}.environment-review{min-width:0}.environment-comparison{padding-bottom:18px}.mesh-environment h3{display:flex;align-items:center;gap:9px;font-size:14px;font-weight:650;margin:0 0 14px}.environment-comparison h3>svg{color:var(--ui-text-muted)}.environment-compare-row{display:flex;align-items:center;justify-content:space-between;gap:18px;min-height:38px;border-bottom:1px solid color-mix(in srgb,var(--ui-border) 60%,transparent)}.environment-compare-row>span:first-child{color:var(--ui-text-muted)}.environment-compare-value{display:inline-flex;align-items:center;gap:6px;font-size:12px;font-weight:550}.is-match{color:var(--ui-success)}.is-different{color:var(--ui-warning)}.environment-compare-count{display:flex;align-items:baseline;gap:10px;white-space:nowrap;font-size:14px}.environment-compare-count strong{font-weight:600}.environment-compare-count span,.environment-compare-count small{color:var(--ui-text-muted);font-size:12px;font-weight:400}.environment-files{border-top:1px solid var(--ui-border)}.environment-files :deep(.settings-disclosure){margin:0;border:0;border-bottom:1px solid var(--ui-border)}.environment-files :deep(.settings-disclosure-trigger){min-height:52px}.mesh-environment pre{font:12px/1.6 ui-monospace,monospace;white-space:pre-wrap;overflow-wrap:anywhere;max-height:220px;overflow:auto;background:var(--ui-surface-2);border:1px solid var(--ui-border);padding:12px;border-radius:8px;margin:8px 0}.mesh-environment ul{list-style:none;padding:0;margin:0}.environment-mods{max-height:240px;overflow:auto}.environment-mods li{display:grid;gap:2px;padding:9px 0;overflow-wrap:anywhere}.environment-mods li+li{border-top:1px solid var(--ui-border)}.environment-mods small,.environment-hint{color:var(--ui-text-muted);font-size:12px}.environment-config-text{margin:10px 0}.environment-config-text>strong{font-size:12px;overflow-wrap:anywhere}.environment-configs{border:0;padding:0;margin:0;display:grid;gap:12px;min-width:0}.environment-configs label{display:flex;gap:9px;align-items:center;overflow-wrap:anywhere}.environment-configs input{accent-color:var(--ui-accent);flex:none}.environment-configs :deep(.settings-disclosure-trigger){font-size:12px;min-height:36px}.environment-excluded{display:flex;align-items:flex-start;gap:10px;margin-top:20px;padding:14px;border-left:3px solid var(--ui-warning);background:var(--ui-warning-soft);color:var(--ui-warning)}.environment-excluded>svg{flex:none;margin-top:2px}.environment-excluded ul{font-size:12px;margin:8px 0;overflow-wrap:anywhere}.environment-excluded p{margin:6px 0 0;font-size:12px}.environment-share-action{margin-top:20px}.environment-install{min-width:0;align-self:start;padding:22px;border-radius:var(--ui-radius-lg,12px);background:var(--ui-surface-2);border:1px solid var(--ui-border)}.environment-install>h3{margin-bottom:20px}.environment-install>h3>svg{color:var(--ui-text-muted)}.environment-personal,.environment-space{display:flex;align-items:flex-start;gap:12px;padding:0 0 18px;margin-bottom:18px;border-bottom:1px solid var(--ui-border)}.environment-personal>svg,.environment-space>svg{color:var(--ui-text-muted);flex:none;margin-top:2px}.environment-personal>div,.environment-space>div{min-width:0;flex:1}.environment-personal strong,.environment-space strong{font-size:13px;font-weight:600}.environment-personal p{margin:4px 0 0;font-size:12px;color:var(--ui-text-muted)}.environment-personal small{display:block;font-size:12px;margin-top:3px}.environment-space dl{display:grid;gap:3px;margin:6px 0 0;font-size:12px}.environment-space dl>div{display:flex;justify-content:space-between;gap:8px}.environment-space dt{color:var(--ui-text-muted)}.environment-space dd{margin:0;font-variant-numeric:tabular-nums}.environment-manual{display:grid;gap:6px;border-left:3px solid var(--ui-warning);background:var(--ui-warning-soft);padding:12px;margin:16px 0}.environment-manual-title{display:flex;align-items:center;gap:8px}.environment-manual-title svg{color:var(--ui-warning);flex:none}.environment-manual>span{font-size:12px;color:var(--ui-text-muted);overflow-wrap:anywhere}.environment-manual-actions{display:flex;flex-wrap:wrap;align-items:center;gap:8px;margin-top:4px}.environment-manual-actions button{font-size:12px}.environment-ready{display:flex;align-items:center;gap:7px;color:var(--ui-success);font-size:12px;margin-bottom:14px}.environment-destination{display:grid;gap:16px}.environment-name{font-size:12px;font-weight:550}.environment-name input{font-size:14px;font-weight:400}.environment-create{width:100%;justify-content:center}.environment-install>.environment-create{margin-top:18px}.environment-error{color:var(--ui-danger);font-size:13px;margin:0}.environment-alert{display:flex;align-items:center;gap:9px;padding:12px;background:color-mix(in srgb,var(--ui-danger) 8%,var(--ui-surface-1));border-left:3px solid var(--ui-danger)}.environment-alert svg{flex:none}.environment-created{display:flex;align-items:center;gap:10px;color:var(--ui-success);margin:0}.mesh-environment button{display:inline-flex;align-items:center;gap:8px}.mesh-environment>button,.environment-invitation>button{justify-self:start}.environment-hint{margin:0}@media(max-width:820px){.environment-layout-install{grid-template-columns:minmax(0,1fr)}.environment-layout{gap:24px}.environment-install{padding:20px}}@media(max-width:600px){.environment-count-disclosure :deep(.settings-disclosure-title){align-self:center}.environment-count-disclosure :deep(.settings-disclosure-meta){grid-column:-2;grid-row:1;text-align:end}.environment-count-disclosure :deep(.settings-disclosure-chevron){grid-row:1}}@media(max-width:560px){.environment-game-icon{width:48px;height:48px;border-radius:12px}.environment-identity{gap:12px}.environment-game-title h2{font-size:20px}.environment-metadata{gap:5px 12px}.environment-install{padding:18px}.environment-compare-row{min-height:40px}}
</style>
