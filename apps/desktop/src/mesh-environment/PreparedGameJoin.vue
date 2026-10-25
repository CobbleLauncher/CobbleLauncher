<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import {computed} from 'vue';
import type {Game} from '../games';
import {locale} from '../i18n';
const props=defineProps<{game:Game|null;invitation:string;disabled?:boolean}>();
const emit=defineEmits<{join:[value:{gameId:string;invitation:string}]}>();
const ready=computed(()=>!!props.game&&props.game.status==='installed'&&!props.game.content_installing&&!props.game.error&&!!props.invitation.trim()&&props.invitation.length<=6000);
const label=computed(()=>locale.value==='zh-CN'?'使用准备好的游戏加入':'Join with prepared game');
function join(){if(!ready.value||props.disabled||!props.game)return;emit('join',{gameId:props.game.id,invitation:props.invitation});}
</script>
<template>
  <div
    v-if="ready"
    class="prepared-game-join"
  >
    <strong>{{ game!.name }}</strong><button
      type="button"
      class="primary-button"
      :disabled="disabled"
      @click="join"
    >
      {{ label }}
    </button>
  </div>
</template>
<style scoped>
.prepared-game-join{display:flex;align-items:center;justify-content:space-between;gap:16px;flex-wrap:wrap;margin:16px 0;color:var(--ui-text);min-width:0}.prepared-game-join strong{font-size:14px;font-weight:600;overflow-wrap:anywhere}.prepared-game-join button{flex:none}@media(max-width:560px){.prepared-game-join{align-items:flex-start;flex-direction:column;gap:8px}}
</style>
