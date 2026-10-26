<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { ArrowLeft, ArrowRight, Check, Folder, Gamepad2, Monitor, RefreshCw, Send, ShieldCheck, TriangleAlert } from '@lucide/vue';
import TransferWorldScene from '../ui/TransferWorldScene.vue';
import LoadingIndicator from '../ui/LoadingIndicator.vue';
import ConfirmationDialog from '../ui/ConfirmationDialog.vue';
import TransferContentPicker from './TransferContentPicker.vue';
import TransferPeerPicker from './TransferPeerPicker.vue';
import TransferOfferList from './TransferOfferList.vue';
import TransferStatus from './TransferStatus.vue';
import { transferText as text, formatTransferBytes } from './labels';
import { transferScopes } from './types';
import { useGameTransfer, type GameTransferEntry } from './use-game-transfer';
const props=defineProps<GameTransferEntry>();
const emit=defineEmits<{back:[];'open-mesh':[];'open-game':[id:string];task:[id:string];discarded:[id:string]}>();
const {context,peers,offers,preview,offer,outgoing,stage,selection,peerId,name,busy,error,meshAvailable,loaded,sendMode,selectedPeer,active,record,refresh,selectOffer,prepare,send,accept,reject,cancel,discard,failure}=useGameTransfer(props,id=>emit('task',id),id=>emit('discarded',id));
const confirmDiscard=ref(false);
const heading=ref<HTMLElement|null>(null);
const steps=computed(()=>sendMode.value?[text('target'),text('content'),text('review'),text('transfer')]:[text('receive'),text('review'),text('transfer')]);
const step=computed(()=>sendMode.value?['target','content','review','transfer'].indexOf(stage.value):['inbox','receive','transfer'].indexOf(stage.value));
const title=computed(()=>text(stage.value==='target'?'chooseTarget':stage.value==='content'?'chooseContent':stage.value==='review'?'reviewSend':stage.value==='receive'?'reviewReceive':'chooseOffer'));
const subject=computed(()=>sendMode.value?context.value:offer.value);
const sourceName=computed(()=>context.value?.name??outgoing.value?.gameName??offer.value?.gameName??record.value?.gameName??text('title'));
const peerName=computed(()=>selectedPeer.value?.displayName??peers.value.find(peer=>peer.peerId===offer.value?.peerId)?.displayName??offer.value?.peerId??text(sendMode.value?'target':'sourceComputer'));
const scopes=computed(()=>stage.value==='receive'?offer.value?.scopes??[]:transferScopes.filter(item=>selection.value[item]));
const metrics=computed(()=>stage.value==='receive'?offer.value:preview.value);
const canContinue=computed(()=>meshAvailable.value&&context.value?.stopped!==false&&!!selectedPeer.value&&!busy.value);
watch(stage,async()=>{await nextTick();heading.value?.focus();});
defineExpose({confirmLeave:async()=>!busy.value});
</script>
<template>
  <section
    class="game-transfer-page page-content"
    :aria-busy="busy"
  >
    <ol
      class="game-transfer-steps"
      :aria-label="text('title')"
    >
      <li
        v-for="(label,index) in steps"
        :key="label"
        :class="{current:index===step,finished:index<step}"
        :aria-current="index===step?'step':undefined"
      >
        <span><Check
          v-if="index<step"
          :size="13"
          aria-hidden="true"
        /><template v-else>{{ index+1 }}</template></span>{{ label }}
      </li>
    </ol>
    <LoadingIndicator
      v-if="!loaded&&!error"
      :label="text('loading')"
    />
    <p
      v-if="error||record?.state==='failed'&&record.error"
      class="game-transfer-alert"
      role="alert"
    >
      <TriangleAlert
        :size="19"
        aria-hidden="true"
      />{{ error||failure(record?.error) }}
    </p>
    <div
      v-if="stage==='transfer'"
      class="game-transfer-progress"
    >
      <TransferStatus
        v-if="record"
        :record="record"
        :source-label="sendMode?sourceName:peerName"
        :target-label="sendMode?peerName:sourceName"
      />
      <LoadingIndicator
        v-else-if="!error"
        :label="text('preparing')"
      />
      <div class="game-transfer-actions progress-actions">
        <template v-if="active">
          <button
            type="button"
            class="secondary-button"
            :disabled="busy||record?.state==='cancelling'"
            @click="cancel"
          >
            {{ text('cancel') }}
          </button><button
            type="button"
            class="primary-button"
            :disabled="busy"
            @click="emit('back')"
          >
            {{ text('background') }}<ArrowRight
              :size="17"
              aria-hidden="true"
            />
          </button>
        </template>
        <template v-else>
          <button
            v-if="record?.direction==='receive'&&['failed','cancelled','interrupted'].includes(record.state)&&record.gameId"
            type="button"
            class="secondary-button transfer-discard-button"
            :disabled="busy"
            @click="confirmDiscard=true"
          >
            {{ text('discard') }}
          </button>
          <button
            type="button"
            class="secondary-button"
            @click="emit('back')"
          >
            {{ text('done') }}
          </button><button
            v-if="record?.state==='succeeded'&&record.gameId"
            type="button"
            class="primary-button"
            @click="emit('open-game',record.gameId)"
          >
            {{ text('open') }}<ArrowRight
              :size="17"
              aria-hidden="true"
            />
          </button>
        </template>
      </div>
    </div>
    <div
      v-else-if="loaded"
      class="game-transfer-workspace"
    >
      <header class="game-transfer-step-heading">
        <h2
          ref="heading"
          tabindex="-1"
        >
          {{ title }}
        </h2><button
          v-if="stage==='target'||stage==='inbox'"
          type="button"
          class="secondary-button transfer-refresh"
          :disabled="busy"
          :aria-label="text('refresh')"
          @click="refresh"
        >
          <RefreshCw
            :size="17"
            aria-hidden="true"
          />
        </button>
      </header>
      <div
        v-if="subject"
        class="game-transfer-identity"
      >
        <span class="game-transfer-identity-icon"><Gamepad2
          :size="28"
          :stroke-width="1.5"
          aria-hidden="true"
        /></span><div><strong>{{ sourceName }}</strong><span>Minecraft {{ subject.version }} · {{ subject.loaderKind||text('vanilla') }} {{ subject.loaderVersion }}</span></div>
      </div>
      <template v-if="stage==='target'">
        <div
          v-if="!meshAvailable||!peers.length"
          class="game-transfer-empty"
        >
          <Monitor
            :size="40"
            :stroke-width="1.3"
            aria-hidden="true"
          /><p>{{ text(!meshAvailable?'meshUnavailable':'noPeers') }}</p><button
            type="button"
            class="primary-button"
            @click="emit('open-mesh')"
          >
            {{ text('connect') }}<ArrowRight
              :size="17"
              aria-hidden="true"
            />
          </button>
        </div>
        <template v-else>
          <TransferPeerPicker
            v-model="peerId"
            :peers="peers"
            :disabled="busy"
          /><p class="game-transfer-hint">
            {{ text('anotherComputer') }}
          </p>
        </template>
        <p
          v-if="context?.stopped===false"
          class="game-transfer-alert"
          role="status"
        >
          {{ text('busyGame') }}
        </p>
        <div class="game-transfer-actions">
          <button
            v-if="selectedPeer&&!selectedPeer.connected"
            type="button"
            class="secondary-button"
            @click="emit('open-mesh')"
          >
            {{ text('connect') }}
          </button><button
            type="button"
            class="primary-button"
            :disabled="!canContinue"
            @click="stage='content'"
          >
            {{ text('next') }}<ArrowRight
              :size="17"
              aria-hidden="true"
            />
          </button>
        </div>
      </template>
      <template v-else-if="stage==='content'">
        <TransferContentPicker
          v-model="selection"
          :available="context?.availableScopes??[]"
          :disabled="busy"
        />
        <div class="game-transfer-actions">
          <button
            type="button"
            class="secondary-button"
            :disabled="busy"
            @click="stage='target'"
          >
            <ArrowLeft
              :size="17"
              aria-hidden="true"
            />{{ text('back') }}
          </button><button
            type="button"
            class="primary-button"
            :disabled="busy||!Object.values(selection).some(Boolean)"
            @click="prepare"
          >
            {{ busy?text('preparing'):text('next') }}<ArrowRight
              :size="17"
              aria-hidden="true"
            />
          </button>
        </div>
      </template>
      <template v-else-if="stage==='inbox'">
        <TransferOfferList
          v-if="offers.length"
          :offers="offers"
          :peers="peers"
          :disabled="busy"
          @select="selectOffer"
        />
        <div
          v-else
          class="game-transfer-empty"
        >
          <Send
            :size="36"
            :stroke-width="1.3"
            aria-hidden="true"
          /><p>{{ text(meshAvailable?'noOffers':'meshUnavailable') }}</p><button
            v-if="!meshAvailable"
            type="button"
            class="primary-button"
            @click="emit('open-mesh')"
          >
            {{ text('connect') }}
          </button>
        </div>
      </template>
      <template v-else>
        <TransferWorldScene
          class="game-transfer-review-scene"
          :progress="null"
          state="waiting"
          :source-label="sendMode?sourceName:peerName"
          :target-label="sendMode?peerName:sourceName"
        />
        <dl class="game-transfer-review">
          <div><dt>{{ text(sendMode?'recipient':'sender') }}</dt><dd>{{ peerName }}</dd></div><div><dt>{{ text('size') }}</dt><dd>{{ formatTransferBytes(metrics?.totalBytes??0) }}</dd></div><div><dt>{{ text('files') }}</dt><dd>{{ metrics?.fileCount }}</dd></div><div><dt>{{ text('content') }}</dt><dd>{{ scopes.map(scope=>text(scope)).join(' · ') }}</dd></div>
        </dl>
        <template v-if="stage==='receive'">
          <label class="game-transfer-name">{{ text('name') }}<input
            v-model="name"
            type="text"
            maxlength="80"
            :disabled="busy"
            autocomplete="off"
          ></label><p class="game-transfer-location">
            <Folder
              :size="17"
              aria-hidden="true"
            />{{ text('defaultLocation') }}
          </p>
        </template>
        <p class="game-transfer-consent">
          <ShieldCheck
            :size="18"
            aria-hidden="true"
          />{{ text(sendMode?'sendConsent':'receiveConsent') }}
        </p>
        <p
          v-if="!sendMode&&scopes.includes('mods')"
          class="game-transfer-code-warning"
        >
          <TriangleAlert
            :size="17"
            aria-hidden="true"
          />{{ text('codeWarning') }}
        </p>
        <div class="game-transfer-actions">
          <button
            type="button"
            class="secondary-button"
            :disabled="busy"
            @click="stage=sendMode?'content':'inbox'"
          >
            <ArrowLeft
              :size="17"
              aria-hidden="true"
            />{{ text('back') }}
          </button><button
            v-if="!sendMode"
            type="button"
            class="secondary-button"
            :disabled="busy"
            @click="reject"
          >
            {{ text('decline') }}
          </button><button
            type="button"
            class="primary-button"
            :disabled="busy||!sendMode&&!name.trim()"
            @click="sendMode?send():accept()"
          >
            {{ text(sendMode?'send':'accept') }}<ArrowRight
              :size="17"
              aria-hidden="true"
            />
          </button>
        </div>
      </template>
    </div>
    <ConfirmationDialog
      v-model:open="confirmDiscard"
      :title="text('discardTitle')"
      :description="text('discardDescription')"
      :confirm-label="text('discard')"
      :busy="busy"
      danger
      @confirm="discard"
    >
      <strong>{{ sourceName }}</strong>
    </ConfirmationDialog>
  </section>
</template>
<style scoped>
.transfer-discard-button{color:var(--ui-danger);border-color:var(--ui-danger)}.game-transfer-page{max-width:1060px;margin:0 auto;padding:var(--ui-space-5) var(--ui-space-5) var(--ui-space-7);color:var(--ui-text)}.game-transfer-steps{display:flex;justify-content:center;align-items:center;gap:var(--ui-space-6);list-style:none;margin:0 0 var(--ui-space-7);padding:0}.game-transfer-steps li{display:flex;align-items:center;gap:var(--ui-space-2);font-size:var(--ui-text-caption);color:var(--ui-text-muted)}.game-transfer-steps li>span{width:25px;height:25px;display:grid;place-items:center;border:1px solid var(--ui-border);border-radius:50%;font-variant-numeric:tabular-nums}.game-transfer-steps li.current{color:var(--ui-text);font-weight:650}.game-transfer-steps li.current>span{background:var(--ui-accent);color:var(--ui-on-accent);border-color:transparent}.game-transfer-steps li.finished>span{color:var(--ui-focus);border-color:var(--ui-focus)}.game-transfer-workspace{max-width:680px;margin:0 auto}.game-transfer-step-heading{display:flex;align-items:center;justify-content:space-between;gap:var(--ui-space-4);margin-bottom:var(--ui-space-5)}.game-transfer-step-heading h2{font-size:var(--ui-text-page);font-weight:650;line-height:1.3;margin:0}.game-transfer-step-heading h2:focus{outline:none}.transfer-refresh{padding:var(--ui-space-2);min-width:var(--ui-control-height)}.game-transfer-identity{display:flex;align-items:center;gap:var(--ui-space-4);padding-bottom:var(--ui-space-5);margin-bottom:var(--ui-space-4);border-bottom:1px solid var(--ui-border)}.game-transfer-identity-icon{width:56px;height:56px;display:grid;place-items:center;background:var(--tint);color:var(--ui-focus);border-radius:var(--ui-radius-lg);flex:none}.game-transfer-identity>div{display:grid;gap:var(--ui-space-1);min-width:0}.game-transfer-identity strong{font-size:var(--ui-text-section);overflow-wrap:anywhere}.game-transfer-identity>div>span{color:var(--ui-text-muted);font-size:var(--ui-text-caption)}.game-transfer-empty{padding:var(--ui-space-7) 0;display:flex;flex-direction:column;align-items:center;gap:var(--ui-space-4);text-align:center;color:var(--ui-text-muted)}.game-transfer-empty p{margin:0;font-size:var(--ui-text-body)}.game-transfer-actions{display:flex;justify-content:flex-end;align-items:center;gap:var(--ui-space-3);flex-wrap:wrap;margin-top:var(--ui-space-6)}.game-transfer-actions>button:first-child:not(:last-child){margin-right:auto}.game-transfer-actions button{white-space:nowrap;display:inline-flex;align-items:center;justify-content:center;gap:var(--ui-space-2)}.game-transfer-hint{font-size:var(--ui-text-caption);color:var(--ui-text-muted);margin:var(--ui-space-4) 0 0}.game-transfer-review-scene{margin:var(--ui-space-4) -8%}.game-transfer-review{display:grid;grid-template-columns:1fr 1fr;gap:var(--ui-space-4);margin:var(--ui-space-5) 0}.game-transfer-review>div{min-width:0}.game-transfer-review dt{font-size:var(--ui-text-caption);color:var(--ui-text-muted);margin-bottom:var(--ui-space-1)}.game-transfer-review dd{margin:0;font-size:var(--ui-text-body);overflow-wrap:anywhere}.game-transfer-review>div:last-child{grid-column:1/-1}.game-transfer-name{display:grid;gap:var(--ui-space-2);font-size:var(--ui-text-label);margin-top:var(--ui-space-5)}.game-transfer-name input{box-sizing:border-box;width:100%;min-width:0;border:1px solid var(--ui-border-strong);border-radius:var(--ui-radius-md);background:var(--ui-surface-1);color:var(--ui-text);font:inherit;padding:var(--ui-space-3);min-height:var(--ui-control-height)}.game-transfer-location,.game-transfer-consent,.game-transfer-code-warning,.game-transfer-alert{display:flex;align-items:flex-start;gap:var(--ui-space-2);font-size:var(--ui-text-caption);line-height:1.6;color:var(--ui-text-muted)}.game-transfer-location svg,.game-transfer-consent svg,.game-transfer-code-warning svg,.game-transfer-alert svg{flex:none;margin-top:2px}.game-transfer-code-warning{color:var(--ui-warning)}.game-transfer-alert{padding:var(--ui-space-3) var(--ui-space-4);border-radius:var(--ui-radius-md);background:var(--ui-warning-soft);color:var(--ui-warning);margin:0 0 var(--ui-space-4)}.game-transfer-consent{margin-top:var(--ui-space-5)}.progress-actions{max-width:680px;margin:var(--ui-space-6) auto 0}@media(max-width:600px){.game-transfer-page{padding:var(--ui-space-4) var(--ui-space-3) var(--ui-space-6)}.game-transfer-steps{gap:var(--ui-space-3);justify-content:space-between;margin-bottom:var(--ui-space-6)}.game-transfer-steps li{flex-direction:column;gap:var(--ui-space-2);text-align:center;font-size:var(--ui-text-caption)}.game-transfer-step-heading h2{font-size:var(--ui-text-section)}.game-transfer-review-scene{margin:var(--ui-space-4) -4%}.game-transfer-actions{gap:var(--ui-space-2)}.game-transfer-actions button{flex:1}.game-transfer-actions>button:first-child:not(:last-child){margin-right:0}.game-transfer-review{gap:var(--ui-space-3)}}
</style>
