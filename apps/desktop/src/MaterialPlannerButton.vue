<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import {onMounted,ref,watch,computed} from 'vue';
import {invoke} from '@tauri-apps/api/core';
import {locale} from './i18n';
import type {Game} from './games';
const props=defineProps<{resourceId:string;games:Game[];initialGameId?:string}>();
const emit=defineEmits<{error:[message:string]}>();
const receivers=ref<{pluginId:string;name:string}[]>([]);const receiver=ref('');const game=ref(props.initialGameId??'');const busy=ref(false);
const zh=computed(()=>locale.value==='zh-CN');
const installed=computed(()=>props.games.filter(g=>g.status==='installed'));
async function load(){try{const items=await invoke<{pluginId:string;name:string}[]>('blueprint_material_receivers',{resourceId:props.resourceId});receivers.value=Array.isArray(items)?items:[];receiver.value=receivers.value[0]?.pluginId??'';}catch{receivers.value=[]}}
watch(()=>props.resourceId,()=>void load());onMounted(()=>void load());
async function send(){busy.value=true;try{await invoke('send_blueprint_materials',{resourceId:props.resourceId,targetPluginId:receiver.value,gameId:game.value||null});}catch{emit('error',zh.value?'无法交接材料清单':'Could not send materials');}finally{busy.value=false}}
</script>
<template>
  <template v-if="receivers.length">
    <select
      v-if="receivers.length>1"
      v-model="receiver"
      :aria-label="zh?'接收插件':'Receiver'"
    >
      <option
        v-for="item in receivers"
        :key="item.pluginId"
        :value="item.pluginId"
      >
        {{ item.name }}
      </option>
    </select>
    <select
      v-model="game"
      data-field="material-game"
      :aria-label="zh?'材料计划游戏':'Plan game'"
    >
      <option value="">
        {{ zh?'通用计划':'General plan' }}
      </option><option
        v-for="item in installed"
        :key="item.id"
        :value="item.id"
      >
        {{ item.name }}
      </option>
    </select>
    <button
      class="text-button"
      data-action="send-materials"
      :disabled="busy||!receiver"
      @click="send"
    >
      {{ zh?'制定材料计划':'Plan materials' }}
    </button>
  </template>
</template>
