<!-- SPDX-License-Identifier: MIT -->
<script setup lang="ts">
import { computed, ref } from 'vue';
import { ArrowRight, FileText, LockKeyhole, ShieldCheck } from '@lucide/vue';
import { locale } from '../i18n';
import { FIRST_RUN_CONSENT_VERSION } from '../first-run';
import LoadingIndicator from '../ui/LoadingIndicator.vue';
defineProps<{ busy?: boolean; error?: string }>();
const emit = defineEmits<{ accept: [version: string] }>();
const checked = ref(false);
const legal = ref<'terms' | 'privacy'>('terms');
const zh = computed(() => locale.value === 'zh-CN');
</script>
<template>
  <section
    class="onboarding-page consent-page"
    :aria-busy="busy"
  >
    <header class="onboarding-heading">
      <span class="onboarding-mark"><ShieldCheck aria-hidden="true" /></span><h1
        tabindex="-1"
        data-step-heading
      >
        {{ zh ? '使用条款与隐私' : 'Terms and privacy' }}
      </h1>
    </header>
    <div
      class="consent-tabs"
      :aria-label="zh ? '阅读说明' : 'Read documents'"
    >
      <button
        class="secondary-button"
        :aria-pressed="legal === 'terms'"
        @click="legal = 'terms'"
      >
        <FileText aria-hidden="true" />{{ zh ? '使用条款' : 'Terms of use' }}
      </button>
      <button
        class="secondary-button"
        :aria-pressed="legal === 'privacy'"
        @click="legal = 'privacy'"
      >
        <LockKeyhole aria-hidden="true" />{{ zh ? '隐私说明' : 'Privacy notice' }}
      </button>
    </div>
    <article
      class="consent-document selectable"
      tabindex="0"
      :aria-label="legal === 'terms' ? (zh ? '使用条款' : 'Terms of use') : (zh ? '隐私说明' : 'Privacy notice')"
    >
      <template v-if="legal === 'terms'">
        <h2>{{ zh ? '使用条款' : 'Terms of use' }}</h2>
        <p>{{ zh ? 'Cobble 是独立的 Minecraft Java 版启动器，与 Mojang 或 Microsoft 无隶属关系。Cobble 自有代码采用 MIT 许可证；依赖、扩展和游戏内容各自保留原许可证。MIT 许可证不授予 Minecraft 使用权。' : 'Cobble is an independent Minecraft: Java Edition launcher, unaffiliated with Mojang or Microsoft. Cobble code uses the MIT license; dependencies, extensions and game content retain their own licenses. MIT does not grant a Minecraft license.' }}</p>
        <p>{{ zh ? '使用 Microsoft 登录和正版服务器需要符合相应服务的要求。离线档案不提供正版授权，不能加入要求 Microsoft 验证的服务器。模组和安装器会执行代码，请自行选择可信内容，并在重要更改前备份。' : 'Microsoft sign-in and authenticated servers require compliance with the relevant service requirements. Offline profiles do not provide a Minecraft license or access to servers requiring Microsoft authentication. Mods and installers execute code; choose trusted content and back up important files before changes.' }}</p>
        <p>{{ zh ? 'Cobble 按 MIT 许可证原样提供，不附带保证。完整版权和许可文本见安装包中的 LICENSE 和 THIRD_PARTY_NOTICES。' : 'Cobble is provided as-is under the MIT license, without warranty. See LICENSE and THIRD_PARTY_NOTICES in the distribution for full copyright and license terms.' }}</p>
      </template>
      <template v-else>
        <h2>{{ zh ? '隐私说明' : 'Privacy notice' }}</h2>
        <p>{{ zh ? '游戏配置、档案资料、下载记录、日志和备份保存在本机。Microsoft 登录会联系 Microsoft、Xbox 和 Minecraft 服务；登录凭证由系统凭证库管理。凭证库不可用时，需恢复后重试登录。' : 'Game settings, profile information, download records, logs and backups are stored locally. Microsoft sign-in contacts Microsoft, Xbox and Minecraft services. Sign-in credentials use the system credential store. If it is unavailable, restore access and retry sign-in.' }}</p>
        <p>{{ zh ? '下载、内容搜索、目录订阅和更新检查会向所选服务发送相应请求；这些服务可见网络地址和请求内容。扩展按批准的权限访问数据。分享日志或配置前请检查其中的私人信息。' : 'Downloads, content searches, catalog subscriptions and update checks send requests to the selected services, which can see network addresses and request contents. Extensions access data through approved permissions. Review logs and settings for private information before sharing.' }}</p>
        <p>{{ zh ? 'Cobble Mesh 为可选功能。开启后会连接入口或中继服务，使用公开身份处理联系人、邀请和联机流量；对端和服务可见连接所需的信息。你可以在设置中关闭。' : 'Cobble Mesh is optional. Enabling it connects to seed or relay services and uses a public identity for contacts, invitations and game traffic. Peers and services can see information needed for those connections. You can turn it off in Settings.' }}</p>
      </template>
    </article>
    <footer class="onboarding-footer consent-footer">
      <p
        v-if="error"
        role="alert"
        class="settings-error"
      >
        {{ error }}
      </p>
      <label class="consent-check"><input
        v-model="checked"
        type="checkbox"
        :disabled="busy"
      >{{ zh ? '我已阅读并同意使用条款，了解隐私说明' : 'I agree to the terms and understand the privacy notice' }}</label>
      <div class="onboarding-actions">
        <span class="consent-version">{{ FIRST_RUN_CONSENT_VERSION }}</span><button
          class="primary-button"
          :disabled="!checked || busy"
          @click="emit('accept', FIRST_RUN_CONSENT_VERSION)"
        >
          <LoadingIndicator
            v-if="busy"
            compact
            :label="zh ? '正在保存' : 'Saving'"
          /><template v-else>
            {{ zh ? '同意并继续' : 'Agree and continue' }}<ArrowRight aria-hidden="true" />
          </template>
        </button>
      </div>
    </footer>
  </section>
</template>
