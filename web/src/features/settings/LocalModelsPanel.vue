<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { platformClient } from "@/platform/client";
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import { loadLocalModels } from "./api";
import type { LocalModelItem } from "@/platform/types";

const { t, locale } = useI18n();
const {
  data: localModels,
  loading: localModelsLoading,
  error: localModelsError,
  reload: reloadLocalModels,
  refresh: refreshLocalModels,
} = useResource(loadLocalModels);
const localModelNotice = ref("");
const localModelError = ref("");
const downloadingModelId = ref("");
const activatingModelId = ref("");
const hasWriteToken = ref(!!platformClient.getWriteToken());
let localModelTimer: number | null = null;

const localModelCopy = computed(() =>
  locale.value === "zh-CN"
    ? {
        title: "本地模型",
        note: "内置推荐模型，下载后由程序自带适配层加载；默认 MiniCPM5-1B，1.08B 参数、128K 上下文。",
        root: "模型目录",
        download: "下载",
        downloading: "下载中",
        activate: "启用",
        activating: "启用中…",
        active: "已启用",
        notActive: "未启用",
        stop: "停止",
        retry: "重试",
        tokenRequired: "先在“安全与数据”填写平台写令牌，才能下载或启用模型。",
        runtimeHint: "启用会启动本地推理 sidecar，默认 8K 运行时上下文；可用 DIGILEGACY_LOCAL_MODEL_CTX 调整，并把规划器切到该模型；无需外部 Ollama。",
        quality: "质量档",
        strict: "<2B",
        notStrict: "实际 >2B",
        verified: "SHA256 已校验",
        empty: "暂无推荐模型。",
        downloaded: "已下载",
        failed: "失败",
        partial: "未完成",
        missing: "未下载",
        recommended: "推荐",
        context: "上下文",
        items: {
          "minicpm5-1b": {
            params: "1.08B 稠密",
            description: "1B 级开源 SOTA，128K 上下文，标准 Llama 架构；中文规划、工具选择和日常生成均衡，默认推荐。",
          },
          "minicpm5-2b": {
            params: "2.52B 稠密",
            description: "2B 级质量档，代码、数学、长文本和工具调用更强；实际参数 2.52B，不属于严格 <2B。",
          },
        },
      }
    : {
        title: "Local models",
        note: "Curated models with a bundled adapter. Default is MiniCPM5-1B: 1.08B parameters, 128K context.",
        root: "Model directory",
        download: "Download",
        downloading: "Downloading",
        activate: "Enable",
        activating: "Enabling…",
        active: "Enabled",
        notActive: "Not enabled",
        stop: "Stop",
        retry: "Retry",
        tokenRequired: "Set the platform write token in Security & data before downloading or enabling a model.",
        runtimeHint: "Enable starts the local inference sidecar (8K runtime context by default; set DIGILEGACY_LOCAL_MODEL_CTX to raise it) and switches the planner to this model; no external Ollama needed.",
        quality: "Quality tier",
        strict: "<2B",
        notStrict: "Actually >2B",
        verified: "SHA256 verified",
        empty: "No curated models.",
        downloaded: "Downloaded",
        failed: "Failed",
        partial: "Incomplete",
        missing: "Not downloaded",
        recommended: "Recommended",
        context: "context",
        items: {
          "minicpm5-1b": {
            params: "1.08B dense",
            description: "1B-class open-source SOTA with 128K context and a standard Llama architecture; balanced Chinese planning, tool selection, and everyday generation. Default recommendation.",
          },
          "minicpm5-2b": {
            params: "2.52B dense",
            description: "Quality tier above 2B with stronger code, math, long-text, and tool-calling; at 2.52B parameters it is not strictly under 2B.",
          },
        },
      },
);

function formatBytes(value: number): string {
  if (value >= 1024 * 1024 * 1024) return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GB`;
  if (value >= 1024 * 1024) return `${(value / (1024 * 1024)).toFixed(0)} MB`;
  if (value >= 1024) return `${(value / 1024).toFixed(0)} KB`;
  return `${value} B`;
}

function contextLabel(value: number): string {
  if (value >= 1024 * 1024) return `${Math.round(value / (1024 * 1024))}M`;
  return `${Math.round(value / 1024)}K`;
}

function itemCopy(item: LocalModelItem): { params: string; description: string } {
  const items = localModelCopy.value.items as Record<string, { params: string; description: string }>;
  return items[item.id] ?? { params: item.params, description: item.description };
}

function localModelStatusLabel(item: LocalModelItem): string {
  if (item.active) return localModelCopy.value.active;
  if (item.status === "downloading") return localModelCopy.value.downloading;
  if (item.status === "ready") return localModelCopy.value.downloaded;
  if (item.status === "failed") return localModelCopy.value.failed;
  if (item.status === "partial") return localModelCopy.value.partial;
  return localModelCopy.value.missing;
}

function localModelTone(item: LocalModelItem): "ready" | "degraded" | "danger" | "planned" {
  if (item.active || item.status === "ready") return "ready";
  if (item.status === "downloading") return "degraded";
  if (item.status === "failed") return "danger";
  return "planned";
}

function syncWriteToken() {
  hasWriteToken.value = !!platformClient.getWriteToken();
}

function stopLocalModelPolling() {
  if (localModelTimer !== null) {
    window.clearInterval(localModelTimer);
    localModelTimer = null;
  }
}

function startLocalModelPolling() {
  if (localModelTimer !== null) return;
  localModelTimer = window.setInterval(async () => {
    await refreshLocalModels();
    const busy = localModels.value?.items.some((item: LocalModelItem) => item.status === "downloading");
    if (!busy) {
      localModelNotice.value = "";
      stopLocalModelPolling();
    }
  }, 1500);
}

async function downloadLocalModel(item: LocalModelItem) {
  localModelNotice.value = "";
  localModelError.value = "";
  downloadingModelId.value = item.id;
  try {
    const envelope = await platformClient.downloadLocalModel(item.id);
    if (!envelope.success) {
      localModelError.value =
        envelope.code === "AUTH_REQUIRED" || envelope.code === "FORBIDDEN"
          ? localModelCopy.value.tokenRequired
          : envelope.message || envelope.code;
      return;
    }
    localModelNotice.value = `${item.name} · ${localModelCopy.value.downloading}`;
    await refreshLocalModels();
    startLocalModelPolling();
  } catch (cause) {
    localModelError.value = cause instanceof Error ? cause.message : "error";
  } finally {
    downloadingModelId.value = "";
  }
}

async function activateLocalModel(item: LocalModelItem) {
  localModelNotice.value = "";
  localModelError.value = "";
  activatingModelId.value = item.id;
  try {
    const envelope = await platformClient.activateLocalModel(item.id);
    if (!envelope.success || !envelope.data) {
      localModelError.value =
        envelope.code === "AUTH_REQUIRED" || envelope.code === "FORBIDDEN"
          ? localModelCopy.value.tokenRequired
          : envelope.message || envelope.code;
      return;
    }
    localModels.value = envelope.data;
    localModelNotice.value = `${item.name} · ${localModelCopy.value.active}`;
  } catch (cause) {
    localModelError.value = cause instanceof Error ? cause.message : "error";
  } finally {
    activatingModelId.value = "";
  }
}

async function stopLocalModel() {
  localModelNotice.value = "";
  localModelError.value = "";
  try {
    const envelope = await platformClient.stopLocalModel();
    if (!envelope.success) {
      localModelError.value = envelope.message || envelope.code;
      return;
    }
    localModelNotice.value = localModelCopy.value.stop;
    await refreshLocalModels();
  } catch (cause) {
    localModelError.value = cause instanceof Error ? cause.message : "error";
  }
}

onMounted(() => window.addEventListener("digilegacy:write-token", syncWriteToken));

onUnmounted(() => {
  stopLocalModelPolling();
  window.removeEventListener("digilegacy:write-token", syncWriteToken);
});
</script>

<template>
  <div class="local-model-block">
    <div class="settings-block-head">
      <div>
        <strong>{{ localModelCopy.title }}</strong>
        <p class="setting-hint">{{ localModelCopy.note }}</p>
      </div>
      <div class="local-model-head-actions">
        <StatusPill
          :tone="localModels?.runtime.running ? 'ready' : 'planned'"
          :label="localModels?.runtime.running ? localModelCopy.active : localModelCopy.notActive"
        />
        <button type="button" class="link-button" @click="reloadLocalModels()">{{ t("common.refresh") }}</button>
      </div>
    </div>
    <p class="setting-hint local-model-root">
      {{ localModelCopy.root }}: <span class="mono">{{ localModels?.root || "data/models" }}</span>
    </p>
    <p class="setting-hint">{{ localModelCopy.runtimeHint }}</p>
    <div v-if="localModelsLoading" class="empty-note">{{ t("common.loading") }}</div>
    <div v-else-if="localModelsError" class="empty-note">{{ localModelsError }}</div>
    <div v-else-if="!localModels?.items.length" class="empty-note">{{ localModelCopy.empty }}</div>
    <div v-else class="local-model-list">
      <article v-for="item in localModels?.items" :key="item.id" class="local-model-card">
        <div class="local-model-card-head">
          <div>
            <strong>{{ item.name }}</strong>
            <div class="local-model-badges">
              <span v-if="item.recommended" class="model-badge recommended">{{ localModelCopy.recommended }}</span>
              <span class="model-badge" :class="{ 'under-2b': item.strictUnder2b }">{{ item.strictUnder2b ? localModelCopy.strict : localModelCopy.notStrict }}</span>
              <span v-if="!item.strictUnder2b" class="model-badge quality">{{ localModelCopy.quality }}</span>
            </div>
          </div>
          <StatusPill :tone="localModelTone(item)" :label="localModelStatusLabel(item)" />
        </div>
        <p class="local-model-description">{{ itemCopy(item).description }}</p>
        <div class="local-model-meta">
          <span>{{ itemCopy(item).params }}</span>
          <span>{{ contextLabel(item.context) }} {{ localModelCopy.context }}</span>
          <span>{{ formatBytes(item.sizeBytes) }}</span>
          <span>{{ item.license }}</span>
        </div>
        <div v-if="item.status === 'downloading'" class="local-model-progress">
          <div class="local-model-progress-track">
            <div class="local-model-progress-bar" :style="{ width: `${Math.round(item.progress * 100)}%` }"></div>
          </div>
          <span class="mono">{{ Math.round(item.progress * 100) }}% · {{ formatBytes(item.downloadedBytes) }} / {{ formatBytes(item.sizeBytes) }}</span>
        </div>
        <p v-if="item.error" class="connection-error">{{ item.error }}</p>
        <div class="local-model-actions">
          <span v-if="item.verified" class="model-verified">{{ localModelCopy.verified }}</span>
          <span v-else-if="!hasWriteToken" class="setting-hint">{{ localModelCopy.tokenRequired }}</span>
          <button
            v-if="item.status !== 'ready' && item.status !== 'downloading'"
            type="button"
            class="link-button"
            :disabled="!hasWriteToken || downloadingModelId === item.id"
            @click="downloadLocalModel(item)"
          >
            {{ item.status === 'failed' || item.status === 'partial' ? localModelCopy.retry : localModelCopy.download }}
          </button>
          <button
            v-if="item.status === 'ready' && !item.active"
            type="button"
            class="link-button"
            :disabled="!hasWriteToken || activatingModelId === item.id"
            @click="activateLocalModel(item)"
          >
            {{ activatingModelId === item.id ? localModelCopy.activating : localModelCopy.activate }}
          </button>
          <button v-if="item.active" type="button" class="link-button danger-link" :disabled="!hasWriteToken" @click="stopLocalModel">
            {{ localModelCopy.stop }}
          </button>
        </div>
      </article>
    </div>
    <div v-if="localModelNotice" class="connection-notice">{{ localModelNotice }}</div>
    <div v-if="localModelError" class="connection-error">{{ localModelError }}</div>
  </div>
</template>

<style scoped>
.local-model-block { display: grid; gap: 10px; margin: 0 0 18px; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: color-mix(in srgb, var(--accent) 4%, var(--surface)); }
.local-model-head-actions { display: flex; align-items: center; gap: 10px; }
.local-model-root { margin-top: 0; overflow-wrap: anywhere; }
.local-model-list { display: grid; gap: 10px; margin-top: 4px; }
.local-model-card { display: grid; gap: 8px; padding: 12px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); }
.local-model-card-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.local-model-card-head strong { color: var(--ink); font-size: 13px; }
.local-model-badges { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 5px; }
.model-badge { padding: 2px 6px; border: 1px solid var(--line); border-radius: 999px; color: var(--muted); font-size: 9px; line-height: 1.4; }
.model-badge.recommended { border-color: color-mix(in srgb, var(--accent) 45%, transparent); background: var(--accent-soft); color: var(--accent-strong); }
.model-badge.under-2b { border-color: color-mix(in srgb, var(--accent) 40%, transparent); color: var(--accent); }
.model-badge.quality { border-color: color-mix(in srgb, var(--amber) 45%, transparent); color: var(--amber); }
.local-model-description { margin: 0; color: var(--ink-soft); font-size: 11px; line-height: 1.6; }
.local-model-meta { display: flex; flex-wrap: wrap; gap: 4px 12px; color: var(--muted); font-size: 10px; }
.local-model-progress { display: flex; align-items: center; gap: 10px; }
.local-model-progress-track { flex: 1; height: 6px; overflow: hidden; border-radius: 999px; background: var(--line); }
.local-model-progress-bar { height: 100%; border-radius: inherit; background: var(--accent); transition: width var(--fast) var(--ease); }
.local-model-progress .mono { color: var(--muted); font-size: 9px; white-space: nowrap; }
.local-model-actions { display: flex; align-items: center; justify-content: flex-end; flex-wrap: wrap; gap: 10px; min-height: 28px; }
.local-model-actions .setting-hint { margin: 0; max-width: 46ch; }
.model-verified { color: var(--accent); font-size: 10px; }
.local-model-block .empty-note { margin: 8px 0 2px; padding: 18px; border: 1px dashed var(--line-strong); border-radius: var(--radius-sm); color: var(--muted); font-size: 12px; }
@media (max-width: 640px) {
  .local-model-card-head { flex-direction: column; }
  .local-model-head-actions { flex-wrap: wrap; }
  .local-model-progress { flex-direction: column; align-items: stretch; }
}
</style>
