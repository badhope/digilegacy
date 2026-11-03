<template>
  <div class="asset-detail fade-in">
    <div class="mb-24">
      <button class="btn btn-secondary btn-sm" @click="$router.back()">← {{ t('common.back') }}</button>
    </div>

    <div v-if="loading" class="card" aria-busy="true">
      <div class="skeleton skeleton-line w-40"></div>
      <div class="skeleton skeleton-line w-60"></div>
      <div class="skeleton skeleton-line w-40"></div>
    </div>

    <div v-else-if="loadError" class="card" role="alert" style="border-color: var(--danger);">
      <h3 style="font-size: 16px; margin-bottom: 8px; color: var(--danger-strong);">{{ t('detail.loadFailed') }}</h3>
      <p class="text-muted" style="font-size: 13px;">{{ loadError }}</p>
      <router-link to="/assets" class="btn btn-primary btn-sm mt-16">{{ t('detail.backToList') }}</router-link>
    </div>

    <div v-if="asset" class="card">
      <div class="flex items-center gap-12 mb-24">
        <div style="width: 56px; height: 56px; border-radius: 14px; background: var(--primary-light); display: flex; align-items: center; justify-content: center; font-size: 28px;"><PlatformIcon :name="asset.platform_name" :size="28" /></div>
        <div><h2 style="font-size: 20px; font-weight: 700;">{{ asset.platform_name }}</h2><p class="text-muted" style="font-size: 14px;">{{ asset.account_identifier }}</p></div>
      </div>

      <div class="card mb-24" style="background: var(--gray-50);">
        <div class="card-title">{{ t('detail.policyTitle') }}</div>
        <p style="font-size: 14px; color: var(--gray-700); line-height: 1.6; margin-top: 8px;">{{ platformInfo?.process_description || t('detail.noPolicy') }}</p>
        <div v-if="platformInfo?.required_documents" class="mt-16"><strong style="font-size: 13px;">{{ t('detail.requiredDocs') }}</strong><span class="text-muted" style="font-size: 13px; margin-left: 8px;">{{ platformInfo.required_documents }}</span></div>
      </div>

      <div v-if="hasPlaceholder" class="card mb-24" role="alert" style="border-color: var(--warning); background: var(--warning-light);">
        <strong style="font-size: 14px;">{{ t('detail.placeholderTitle') }}</strong>
        <p style="font-size: 13px; margin-top: 6px;">{{ t('detail.placeholderDesc', { fields: placeholderFields.join('、') }) }}</p>
      </div>

      <div class="card mb-24" style="background: var(--gray-50);">
        <div class="card-title">{{ t('detail.credentialTitle') }}</div>
        <template v-if="security.hasMasterPassword">
          <div class="flex items-center gap-8 mt-16">
            <input class="form-input" :type="showCredential ? 'text' : 'password'" v-model="credentialInput" :aria-label="t('detail.credentialInputLabel')" :placeholder="t('detail.credentialPlaceholder')" style="flex: 1;" />
            <button class="btn btn-secondary btn-sm" @click="showCredential = !showCredential">{{ showCredential ? t('detail.hide') : t('detail.show') }}</button>
          </div>
          <div class="flex items-center gap-8 mt-12" style="flex-wrap: wrap;">
            <button class="btn btn-primary btn-sm" @click="saveCredential">{{ t('detail.saveCredential') }}</button>
            <button class="btn btn-secondary btn-sm" @click="copyCredential">{{ t('detail.copyCredential') }}</button>
            <button class="btn btn-danger btn-sm" v-if="asset?.has_credential" @click="clearCredential">{{ t('detail.clearCredential') }}</button>
            <span class="tag tag-success" v-if="asset?.has_credential">{{ t('detail.encryptedSaved') }}</span>
            <span class="text-muted" style="font-size: 12px;" aria-live="polite">{{ credentialStatus }}</span>
          </div>
        </template>
        <template v-else>
          <p class="text-muted" style="font-size: 13px; margin-top: 8px;">
            {{ t('detail.noEncryptionTip1') }}
            <router-link to="/profile" style="color: var(--primary); text-decoration: underline; text-underline-offset: 2px; font-weight: 600;">{{ t('detail.noEncryptionLink') }}</router-link>
            {{ t('detail.noEncryptionTip2') }}
          </p>
        </template>
        <p v-if="isFinancialPlatform" style="font-size: 12px; color: var(--warning-strong); margin-top: 12px;"><WarningOutline aria-hidden="true" /> {{ t('detail.financeWarning') }}</p>
        <div class="mt-16" style="padding-top: 12px; border-top: 1px solid var(--gray-200, #e8e4de);">
          <button class="btn btn-secondary btn-sm" @click="openOfficial">{{ t('detail.openOfficial') }}</button>
          <span class="text-muted" style="font-size: 12px; margin-left: 8px;">{{ t('detail.openOfficialTip') }}</span>
          <div v-if="linkStatus" class="text-muted" style="font-size: 12px; margin-top: 6px;" role="alert">{{ linkStatus }}</div>
        </div>
      </div>

      <div class="grid grid-2">
        <div class="form-group"><label class="form-label" for="detail-identifier">{{ t('detail.identifierLabel') }}</label><input class="form-input" id="detail-identifier" v-model="asset.account_identifier" /></div>
        <div class="form-group"><label class="form-label" for="detail-category">{{ t('detail.categoryLabel') }}</label><select class="form-select" id="detail-category" v-model="asset.category"><option value="property">{{ t('discover.catProperty') }}</option><option value="personality">{{ t('discover.catPersonality') }}</option><option value="mixed">{{ t('discover.catMixed') }}</option></select></div>
        <div class="form-group"><label class="form-label" for="detail-value">{{ t('detail.valueLabel') }}</label><input class="form-input" id="detail-value" type="number" v-model.number="asset.estimated_value" /></div>
        <div class="form-group"><label class="form-label" for="detail-disposal">{{ t('detail.disposalLabel') }}</label><select class="form-select" id="detail-disposal" v-model="asset.disposal_method"><option value="inherit">{{ t('discover.disposalInherit') }}</option><option value="liquidate">{{ t('discover.disposalLiquidate') }}</option><option value="memorialize">{{ t('discover.disposalMemorialize') }}</option><option value="authorize_view">{{ t('discover.disposalAuthorize') }}</option><option value="delete">{{ t('discover.disposalDelete') }}</option><option value="no_action">{{ t('discover.disposalNone') }}</option></select></div>
      </div>

      <div v-if="asset.disposal_method === 'inherit'" class="grid grid-2" style="margin-top: 16px;">
        <div class="form-group"><label class="form-label" for="detail-heir-name">{{ t('detail.heirNameLabel') }}</label><input class="form-input" id="detail-heir-name" v-model="asset.heir_name" :placeholder="t('detail.heirNameLabel')" /></div>
        <div class="form-group"><label class="form-label" for="detail-heir-contact">{{ t('detail.heirContactLabel') }}</label><input class="form-input" id="detail-heir-contact" v-model="asset.heir_contact" :placeholder="t('discover.heirContactPlaceholder')" /></div>
      </div>

      <div class="form-group mt-16"><label class="form-label" for="detail-notes">{{ t('detail.notesLabel') }}</label><textarea class="form-textarea" id="detail-notes" v-model="asset.notes" :placeholder="t('detail.notesPlaceholder')"></textarea></div>

      <p v-if="saveError" role="alert" style="font-size: 13px; color: var(--danger-strong); margin-top: 16px;">{{ saveError }}</p>

      <div style="display: flex; gap: 12px; justify-content: space-between; margin-top: 24px;">
        <button class="btn btn-danger btn-sm" :disabled="saving" @click="handleDelete">{{ t('detail.deleteAsset') }}</button>
        <div style="display: flex; gap: 12px;">
          <button class="btn btn-secondary btn-sm" :disabled="saving" @click="$router.back()">{{ t('common.cancel') }}</button>
          <button class="btn btn-primary btn-sm" :disabled="saving" @click="handleSave">{{ saving ? t('common.saving') : t('detail.saveChanges') }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { invoke } from '../api/invoke';

import { useI18n } from '../i18n';
import WarningOutline from '@vicons/ionicons5/es/WarningOutline';
import { useAssetsStore } from '../stores/assets';
import { usePlatformsStore } from '../stores/platforms';
import { useSecurityStore } from '../stores/security';
import PlatformIcon from '../components/PlatformIcon.vue';
import { confirmDialog } from '../composables/feedback';
import type { ApiResponse, DigitalAsset } from '../types';

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const assetsStore = useAssetsStore();
const platformsStore = usePlatformsStore();
const security = useSecurityStore();

const asset = ref<DigitalAsset | null>(null);
const credentialInput = ref('');
const showCredential = ref(false);
const credentialStatus = ref('');
const linkStatus = ref('');
const saveError = ref('');
const saving = ref(false);
const loadError = ref('');
const loading = ref(true);

const platformInfo = computed(() => {
  if (!asset.value) return undefined;
  return platformsStore.getById(asset.value.platform_id);
});

const isFinancialPlatform = computed(() =>
  ['支付金融', '金融证券'].includes(platformInfo.value?.category || '')
);

const CREDENTIAL_PLACEHOLDER = '【加密数据 · 需解锁】';
/** 记录加载到的资产里哪些字段是「解密失败占位符」。
 *  这类字段不能被保存回后端（后端会拒绝），界面必须先说清楚原因。 */
const placeholderFields = computed(() => {
  const a = asset.value;
  if (!a) return [] as string[];
  const hits: string[] = [];
  for (const [label, value] of [
    ['账号标识', a.account_identifier],
    ['账号描述', a.account_description],
    ['备注', a.notes],
    ['继承人姓名', a.heir_name ?? ''],
    ['继承人联系方式', a.heir_contact ?? ''],
  ] as const) {
    if (value === CREDENTIAL_PLACEHOLDER) hits.push(label);
  }
  return hits;
});

const hasPlaceholder = computed(() => placeholderFields.value.length > 0);

async function refreshAsset() {
  if (!asset.value?.id) return;
  const res = await invoke<ApiResponse<DigitalAsset | null>>('get_asset_by_id', { id: asset.value.id });
  if (res.success && res.data && asset.value) {
    asset.value.has_credential = res.data.has_credential;
  }
}

async function saveCredential() {
  if (!asset.value?.id) return;
  const text = credentialInput.value.trim();
  if (!text) {
    credentialStatus.value = t('detail.credentialEmpty');
    return;
  }
  try {
    const res = await invoke<ApiResponse<null>>('set_asset_credential', { id: asset.value.id, credential: text });
    credentialStatus.value = res.message;
    if (res.success) {
      credentialInput.value = '';
      showCredential.value = false;
      await refreshAsset();
    }
  } catch (e) {
    credentialStatus.value = String(e);
  }
}

async function copyCredential() {
  if (!asset.value?.id) return;
  try {
    const res = await invoke<ApiResponse<string>>('get_asset_credential', { id: asset.value.id });
    const text = res.data || '';
    if (!text) {
      credentialStatus.value = t('detail.credentialNone');
      return;
    }
    await navigator.clipboard.writeText(text);
    credentialStatus.value = t('detail.credentialCopied');
    window.setTimeout(async () => {
      try {
        const current = await navigator.clipboard.readText();
        if (current === text) {
          await navigator.clipboard.writeText('');
        }
      } catch {
        credentialStatus.value = t('detail.clipboardManual');
      }
    }, 30_000);
  } catch (e) {
    credentialStatus.value = t('detail.copyFailed', { e: String(e) });
  }
}

async function clearCredential() {
  if (!asset.value?.id) return;
  const ok = await confirmDialog({
    title: t('detail.clearCredentialTitle'),
    message: t('detail.clearCredentialMessage'),
    confirmText: t('detail.clearCredential'),
    danger: true,
  });
  if (!ok) return;
  try {
    const res = await invoke<ApiResponse<null>>('set_asset_credential', { id: asset.value.id, credential: '' });
    credentialStatus.value = res.message;
    await refreshAsset();
  } catch (e) {
    credentialStatus.value = String(e);
  }
}

async function openOfficial() {
  const url = platformInfo.value?.official_policy_url;
  if (!url) {
    linkStatus.value = t('detail.noOfficialUrl');
    return;
  }
  try {
    const res = await invoke<ApiResponse<null>>('open_external', { url });
    linkStatus.value = res.success ? '' : res.message;
  } catch (e) {
    linkStatus.value = t('detail.openFailed', { e: String(e) });
  }
}

async function handleSave() {
  if (!asset.value?.id || saving.value) return;
  // 占位符表示这些字段根本没被解密（锁定或密钥不匹配）。
  // 直接提交会让后端拒绝，或在没有后端防护的旧版本上覆盖真实数据。
  if (hasPlaceholder.value) {
    saveError.value = t('detail.placeholderBlocked', { fields: placeholderFields.value.join('、') });
    return;
  }
  // 「指定继承」却没写继承人：报告里会出现「继承人：未指定」，家人无从执行。
  // 后端也有同样的兜底校验（IPC 可绕过前端）。
  if (asset.value.disposal_method === 'inherit' && !(asset.value.heir_name || '').trim()) {
    saveError.value = t('detail.heirRequired');
    return;
  }
  saving.value = true;
  saveError.value = '';
  try {
    await assetsStore.updateAsset(asset.value.id, {
      platform_id: asset.value.platform_id,
      account_identifier: asset.value.account_identifier,
      account_description: asset.value.account_description,
      category: asset.value.category,
      estimated_value: asset.value.estimated_value,
      disposal_method: asset.value.disposal_method,
      heir_name: asset.value.heir_name || undefined,
      heir_contact: asset.value.heir_contact || undefined,
      notes: asset.value.notes,
    });
    router.back();
  } catch (e) {
    // 保存失败必须留在页面并提示：旧实现直接 await 后 back()，
    // 失败时抛出的拒绝无人处理，用户既没保存成功也不知道为什么。
    console.error('保存资产失败:', e);
    saveError.value = t('detail.saveFailed', { e: String(e) });
  } finally {
    saving.value = false;
  }
}

async function handleDelete() {
  if (!asset.value?.id) return;
  const ok = await confirmDialog({
    title: t('detail.deleteTitle'),
    message: t('detail.deleteMessage'),
    confirmText: t('common.delete'),
    danger: true,
  });
  if (!ok) return;
  try {
    await assetsStore.deleteAsset(asset.value.id);
    router.back();
  } catch (e) {
    console.error('删除资产失败:', e);
    saveError.value = t('detail.deleteFailed', { e: String(e) });
  }
}

onMounted(async () => {
  try {
    await Promise.all([platformsStore.loadPlatforms(), security.refresh()]);
    const id = Number(route.params.id);
    // 直接向后端取单条：列表可能尚未加载、也可能已过期。
    // 关键：拿深拷贝，编辑只改本地草稿，「取消」才不会把未保存的值留在 store/报告里。
    const res = await invoke<ApiResponse<DigitalAsset | null>>('get_asset_by_id', { id });
    if (!res.success) {
      loadError.value = res.message || t('detail.loadFail');
      return;
    }
    if (!res.data) {
      loadError.value = t('detail.notFound');
      return;
    }
    asset.value = JSON.parse(JSON.stringify(res.data)) as DigitalAsset;
  } catch (e) {
    loadError.value = t('detail.loadFail') + String(e);
  } finally {
    loading.value = false;
  }
});
</script>
