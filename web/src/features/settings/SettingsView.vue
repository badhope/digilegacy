<script setup lang="ts">
import { computed, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import AppIcon from "@/shared/icons/AppIcon.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SectionCard from "@/shared/ui/SectionCard.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { COMMAND_NAMES } from "@/platform/generated/ipc";
import { platformClient } from "@/platform/client";
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import { useTheme } from "@/platform/theme";
import { formatDateTime } from "@/shared/format";
import { loadAudit, loadConnections, loadSecurity } from "./api";
import type { ConnectionItem, ConnectionKind, ConnectionStatus, ConnectionUpsertInput } from "@/platform/types";

const route = useRoute();
const router = useRouter();
const { t, locale, setLocale } = useI18n();
const { theme, setTheme } = useTheme();
const { data: security, loading: securityLoading, error: securityError, reload: reloadSecurity } = useResource(loadSecurity);
const auditKind = ref("");
const audit = useResource(() => loadAudit({ limit: 20, ...(auditKind.value ? { kind: auditKind.value } : {}) }));
const {
  data: connections,
  loading: connectionsLoading,
  error: connectionsLoadError,
  reload: reloadConnections,
  refresh: refreshConnections,
} = useResource(loadConnections);
const CONNECTION_KIND_LABELS: Record<ConnectionKind, { zh: string; en: string }> = {
  llm: { zh: "LLM", en: "LLM" },
  policy: { zh: "政策", en: "Policy" },
  smtp: { zh: "SMTP", en: "SMTP" },
  imap: { zh: "IMAP", en: "IMAP" },
  heartbeat: { zh: "心跳", en: "Heartbeat" },
  webhook: { zh: "Webhook", en: "Webhook" },
  feishu: { zh: "飞书", en: "Feishu" },
  wecom: { zh: "企业微信", en: "WeCom" },
};
const CONNECTION_STATUS_LABELS: Record<ConnectionStatus, { zh: string; en: string }> = {
  unconfigured: { zh: "未配置", en: "Unconfigured" },
  ok: { zh: "正常", en: "OK" },
  degraded: { zh: "降级", en: "Degraded" },
  error: { zh: "错误", en: "Error" },
  disabled: { zh: "已停用", en: "Disabled" },
};
const connectionCopy = computed(() =>
  locale.value === "zh-CN"
    ? {
        test: "测试",
        testing: "测试中…",
        tokenRequired: "缺少平台写令牌",
        add: "新增连接",
        edit: "编辑",
        remove: "删除",
        save: "保存",
        saving: "保存中…",
        cancel: "取消",
        name: "名称",
        kind: "类型",
        endpoint: "连接地址",
        endpointHint: "HTTP 类型填完整 URL；SMTP/IMAP 填 host:port。",
        token: "凭据",
        tokenHint: "可选；编辑时必须重新填写地址和凭据，旧值不会回显。",
        username: "用户名",
        from: "发件人",
        security: "加密方式",
        enabled: "启用",
        envReadonly: "环境变量连接只读",
        editHint: "编辑已保存连接需要重新填写地址与凭据。",
        configKeyMissing: "服务端未配置连接配置密钥；请设置 DIGILEGACY_CONFIG_KEY_FILE 或 DIGILEGACY_DB_KEY_FILE。",
        deleteConfirm: "确定删除这条连接配置？",
        formTitleAdd: "新增连接",
        formTitleEdit: "编辑连接",
      }
    : {
        test: "Test",
        testing: "Testing…",
        tokenRequired: "Write token required",
        add: "Add connection",
        edit: "Edit",
        remove: "Delete",
        save: "Save",
        saving: "Saving…",
        cancel: "Cancel",
        name: "Label",
        kind: "Kind",
        endpoint: "Endpoint",
        endpointHint: "HTTP kinds use a full URL; SMTP/IMAP use host:port.",
        token: "Credential",
        tokenHint: "Optional; editing requires re-entering endpoint and credential. Old values are never echoed.",
        username: "Username",
        from: "From",
        security: "Security",
        enabled: "Enabled",
        envReadonly: "Environment connection is read-only",
        editHint: "Editing a saved connection requires re-entering its endpoint and credential.",
        configKeyMissing: "Server connection key is not configured; set DIGILEGACY_CONFIG_KEY_FILE or DIGILEGACY_DB_KEY_FILE.",
        deleteConfirm: "Delete this connection config?",
        formTitleAdd: "Add connection",
        formTitleEdit: "Edit connection",
      },
);
function connectionKindLabel(kind: ConnectionKind): string {
  const label = CONNECTION_KIND_LABELS[kind] ?? { zh: kind, en: kind };
  return locale.value === "zh-CN" ? label.zh : label.en;
}
function connectionStatusLabel(status: ConnectionStatus): string {
  const label = CONNECTION_STATUS_LABELS[status] ?? { zh: status, en: status };
  return locale.value === "zh-CN" ? label.zh : label.en;
}
function connectionSourceLabel(item: ConnectionItem): string {
  return item.source === "env" ? `${item.label} · ENV` : `${item.label} · STORED`;
}
function connectionRowStatus(item: ConnectionItem): string {
  return item.enabled ? connectionStatusLabel(item.lastStatus) : CONNECTION_STATUS_LABELS.disabled[locale.value === "zh-CN" ? "zh" : "en"];
}

const testingId = ref<string | null>(null);
const connectionNotice = ref("");
const connectionError = ref("");

const connectionFormOpen = ref(false);
const connectionSaving = ref(false);
const connectionFormError = ref("");
const editingConnectionId = ref("");
const connectionForm = ref<{
  kind: ConnectionKind;
  label: string;
  endpoint: string;
  token: string;
  username: string;
  from: string;
  security: "tls" | "starttls" | "none";
  enabled: boolean;
}>({
  kind: "webhook",
  label: "",
  endpoint: "",
  token: "",
  username: "",
  from: "",
  security: "starttls",
  enabled: true,
});
const isMailKind = computed(() => connectionForm.value.kind === "smtp" || connectionForm.value.kind === "imap");

function resetConnectionForm() {
  connectionForm.value = {
    kind: "webhook",
    label: "",
    endpoint: "",
    token: "",
    username: "",
    from: "",
    security: "starttls",
    enabled: true,
  };
  editingConnectionId.value = "";
  connectionFormError.value = "";
}

function openAddConnection() {
  resetConnectionForm();
  connectionFormOpen.value = true;
}

const guideNotice = ref("");

function startGuidedConnection(kind: ConnectionKind) {
  resetConnectionForm();
  connectionForm.value.kind = kind;
  connectionForm.value.label = kind === "smtp" ? "work-mail" : kind;
  connectionFormOpen.value = true;
  guideNotice.value = "";
  window.requestAnimationFrame(() => {
    document.getElementById("connection-form")?.scrollIntoView({ behavior: "smooth", block: "start" });
  });
}

async function copyConnectionGuide() {
  const origin = typeof window === "undefined" ? "" : window.location.origin;
  const text =
    locale.value === "zh-CN"
      ? `DigiLegacy 账号接入填写说明
填写入口：${origin}/settings/connections

给填写人：用自己的账号填写，不要把密码或令牌发到群里；平台保存后不会回显凭据。

需要准备：
- SMTP：服务器地址 host:port、用户名、密码或授权码、发件人地址、加密方式（465 用 TLS，587 用 STARTTLS）
- Webhook：完整 URL、访问令牌（如服务商要求）
- 飞书 / 企业微信：机器人 Webhook 完整 URL

步骤：
1. 在页面顶部填平台写令牌。
2. 点“新增连接”，选择对应类型。
3. 填写名称、地址、凭据；SMTP 还要填用户名、发件人、加密方式。
4. 保存后点“测试”，确认显示 OK。
5. 把连接名称和状态截图交给管理员；不要回传凭据。`
      : `DigiLegacy account setup note
Entry: ${origin}/settings/connections

For the filler: use your own account. Do not post passwords or tokens in chat; the platform never echoes the credential after saving.

Prepare:
- SMTP: host:port, username, password or app password, From address, security (465 uses TLS, 587 uses STARTTLS)
- Webhook: full URL and access token if the provider requires one
- Feishu / WeCom: full bot webhook URL

Steps:
1. Enter the platform write token at the top of the page.
2. Press Add connection and pick the kind.
3. Fill the label, endpoint and credential; for SMTP also fill username, From and security.
4. Save, press Test, and confirm it reports OK.
5. Send the connection label and status screenshot back; never send the credential.`;
  guideNotice.value = "";
  try {
    if (!navigator.clipboard) throw new Error("clipboard unavailable");
    await navigator.clipboard.writeText(text);
    guideNotice.value = t("settings.connections.guide.copied");
  } catch {
    guideNotice.value = t("settings.connections.guide.copyFailed");
  }
}

function openEditConnection(item: ConnectionItem) {
  resetConnectionForm();
  editingConnectionId.value = item.id;
  connectionForm.value.kind = item.kind;
  connectionForm.value.label = item.label;
  connectionForm.value.enabled = item.enabled;
  connectionFormOpen.value = true;
}

function closeConnectionForm() {
  connectionFormOpen.value = false;
  connectionFormError.value = "";
}

async function saveConnection() {
  connectionSaving.value = true;
  connectionFormError.value = "";
  connectionNotice.value = "";
  try {
    const payload: ConnectionUpsertInput = {
      kind: connectionForm.value.kind,
      label: connectionForm.value.label.trim() || "default",
      endpoint: connectionForm.value.endpoint.trim(),
      token: connectionForm.value.token.trim() || undefined,
      enabled: connectionForm.value.enabled,
    };
    if (editingConnectionId.value) payload.id = editingConnectionId.value;
    if (connectionForm.value.kind === "smtp" || connectionForm.value.kind === "imap") {
      payload.username = connectionForm.value.username.trim() || undefined;
      payload.from = connectionForm.value.from.trim() || undefined;
      payload.security = connectionForm.value.security;
    }
    const envelope = await platformClient.saveConnection(payload);
    if (!envelope.success || !envelope.data) {
      connectionFormError.value =
        envelope.code === "CONFIG_KEY_MISSING"
          ? connectionCopy.value.configKeyMissing
          : envelope.code === "AUTH_REQUIRED" || envelope.code === "FORBIDDEN"
            ? connectionCopy.value.tokenRequired
            : envelope.message || envelope.code;
      return;
    }
    connectionNotice.value = `${envelope.data.kind} / ${envelope.data.label} · ${t("common.ready")}`;
    connectionFormOpen.value = false;
    await refreshConnections();
  } catch (cause) {
    connectionFormError.value = cause instanceof Error ? cause.message : "error";
  } finally {
    connectionSaving.value = false;
  }
}

async function removeConnection(item: ConnectionItem) {
  if (!window.confirm(connectionCopy.value.deleteConfirm)) return;
  connectionNotice.value = "";
  connectionError.value = "";
  try {
    const envelope = await platformClient.deleteConnection(item.id);
    if (!envelope.success) {
      connectionError.value =
        envelope.code === "AUTH_REQUIRED" || envelope.code === "FORBIDDEN"
          ? connectionCopy.value.tokenRequired
          : envelope.message || envelope.code;
      return;
    }
    connectionNotice.value = `${item.kind} / ${item.label} · ${connectionCopy.value.remove}`;
    await reloadConnections();
  } catch (cause) {
    connectionError.value = cause instanceof Error ? cause.message : "error";
  }
}

const writeToken = ref(platformClient.getWriteToken());
const tokenSaved = ref(false);
function saveWriteToken() {
  platformClient.setWriteToken(writeToken.value.trim());
  tokenSaved.value = true;
}
function clearWriteToken() {
  writeToken.value = "";
  platformClient.setWriteToken("");
  tokenSaved.value = true;
}

function connectionTone(status: ConnectionItem["lastStatus"]): "ready" | "degraded" | "danger" | "planned" {
  if (status === "ok") return "ready";
  if (status === "degraded") return "degraded";
  if (status === "error") return "danger";
  return "planned";
}

async function testConnection(item: ConnectionItem) {
  testingId.value = item.id;
  connectionNotice.value = "";
  connectionError.value = "";
  try {
    const envelope = await platformClient.testConnection(
      item.source === "env" ? { kind: item.kind } : { kind: item.kind, id: Number(item.id) },
    );
    if (!envelope.success || !envelope.data) {
      connectionError.value =
        envelope.code === "AUTH_REQUIRED" || envelope.code === "FORBIDDEN"
          ? connectionCopy.value.tokenRequired
          : envelope.message || envelope.code;
      return;
    }
    const result = envelope.data;
    connectionNotice.value = `${result.status} · ${result.latencyMs} ms · ${result.message}`;
    await refreshConnections();
  } catch (cause) {
    connectionError.value = cause instanceof Error ? cause.message : "error";
  } finally {
    testingId.value = null;
  }
}

const sections = ["workspace", "appearance", "security", "data", "connections", "audit", "about"] as const;
type Section = (typeof sections)[number];
const active = computed<Section>(() => {
  const value = route.params.section;
  return typeof value === "string" && sections.includes(value as Section) ? (value as Section) : "workspace";
});

function go(section: Section) {
  router.push(`/settings/${section}`);
}
</script>

<template>
  <div class="settings-page">
    <PageHeader eyebrow="SETTINGS / GLOBAL TOOLS" :title="t('settings.title')" :description="t('settings.description')">
      <template #actions>
        <StatusPill tone="ready" :label="t('app.mode')" />
      </template>
    </PageHeader>

    <div class="settings-shell">
      <nav class="settings-tabs" :aria-label="t('settings.title')">
        <button
          v-for="section in sections"
          :key="section"
          type="button"
          class="settings-tab"
          :class="{ active: active === section }"
          @click="go(section)"
        >
          {{ t(`settings.tab.${section}`) }}
        </button>
      </nav>

      <div class="settings-content">
        <SectionCard v-if="active === 'workspace'" :title="t('settings.tab.workspace')" :subtitle="t('settings.description')">
          <div class="setting-row">
            <span>{{ t("settings.workspace.name") }}</span>
            <strong>{{ t("app.workspace") }}</strong>
          </div>
          <div class="setting-row">
            <span>{{ t("settings.workspace.mode") }}</span>
            <span>{{ t("settings.workspace.mode.value") }}</span>
          </div>
          <div class="setting-row">
            <span>{{ t("app.mode") }}</span>
            <StatusPill tone="planned" :label="t('common.planned')" />
          </div>
        </SectionCard>

        <SectionCard v-else-if="active === 'appearance'" :title="t('settings.tab.appearance')" :subtitle="t('settings.description')">
          <div class="setting-row">
            <span>{{ t("settings.appearance.theme") }}</span>
            <div class="segmented">
              <button type="button" :class="{ selected: theme === 'light' }" @click="setTheme('light')">
                <AppIcon name="sun" :size="14" />
                {{ t("settings.appearance.theme.light") }}
              </button>
              <button type="button" :class="{ selected: theme === 'dark' }" @click="setTheme('dark')">
                <AppIcon name="moon" :size="14" />
                {{ t("settings.appearance.theme.dark") }}
              </button>
            </div>
          </div>
          <div class="setting-row">
            <span>{{ t("settings.appearance.language") }}</span>
            <div class="segmented">
              <button type="button" :class="{ selected: locale === 'zh-CN' }" @click="setLocale('zh-CN')">中文</button>
              <button type="button" :class="{ selected: locale === 'en-US' }" @click="setLocale('en-US')">English</button>
            </div>
          </div>
        </SectionCard>

        <SectionCard v-else-if="active === 'security'" :title="t('settings.tab.security')" :subtitle="t('settings.tab.security.note')">
          <div v-if="securityLoading" class="empty-note">{{ t("common.loading") }}</div>
          <div v-else-if="securityError" class="empty-note">{{ securityError }}</div>
          <template v-else-if="security">
            <div class="setting-row">
              <span>{{ t("settings.security.locked") }}</span>
              <StatusPill :tone="security.locked ? 'planned' : 'ready'" :label="security.locked ? t('settings.security.locked.yes') : t('settings.security.locked.no')" />
            </div>
            <div class="setting-row">
              <span>{{ t("settings.security.recovery") }}</span>
              <strong>{{ security.recoveryConfigured ? t("common.ready") : t("settings.security.notConfigured") }}</strong>
            </div>
            <div class="setting-row">
              <span>{{ t("settings.security.keySource") }}</span>
              <strong class="mono">{{ security.keySource === "passphrase" ? t("settings.security.key.passphrase") : security.keySource }}</strong>
            </div>
            <div class="setting-row">
              <span>{{ t("settings.security.lastBackup") }}</span>
              <strong class="mono">{{ security.lastBackupAt || t("settings.security.none") }}</strong>
            </div>
            <div class="setting-row">
              <span>{{ t("settings.security.refresh") }}</span>
              <button type="button" class="link-button" @click="reloadSecurity">{{ t("common.retry") }}</button>
            </div>
          </template>
        </SectionCard>

        <SectionCard v-else-if="active === 'data'" :title="t('settings.tab.data')" :subtitle="t('settings.tab.data.note')">
          <div class="empty-note">{{ t("settings.tab.data.empty") }}</div>
        </SectionCard>

        <SectionCard v-else-if="active === 'connections'" :title="t('settings.tab.connections')" :subtitle="t('settings.tab.connections.note')">
          <div class="connection-guide">
            <div class="connection-guide-head">
              <div>
                <strong>{{ t("settings.connections.guide.title") }}</strong>
                <p class="setting-hint">{{ t("settings.connections.guide.note") }}</p>
              </div>
              <StatusPill tone="planned" :label="t('settings.connections.guide.handoff')" />
            </div>
            <ol class="connection-guide-steps">
              <li>{{ t("settings.connections.guide.step1") }}</li>
              <li>{{ t("settings.connections.guide.step2") }}</li>
              <li>{{ t("settings.connections.guide.step3") }}</li>
            </ol>
            <div class="connection-guide-actions">
              <button type="button" class="link-button" @click="startGuidedConnection('smtp')">
                {{ t("settings.connections.guide.smtp") }}
              </button>
              <button type="button" class="link-button" @click="startGuidedConnection('webhook')">
                {{ t("settings.connections.guide.webhook") }}
              </button>
              <button type="button" class="link-button" @click="startGuidedConnection('feishu')">
                {{ t("settings.connections.guide.feishu") }}
              </button>
              <button type="button" class="link-button" @click="startGuidedConnection('wecom')">
                {{ t("settings.connections.guide.wecom") }}
              </button>
              <button type="button" class="link-button" @click="copyConnectionGuide">
                {{ t("settings.connections.guide.copy") }}
              </button>
            </div>
            <p v-if="guideNotice" class="connection-notice">{{ guideNotice }}</p>
          </div>

          <div class="setting-row token-row">
            <div>
              <span>{{ t("settings.connections.token") }}</span>
              <p class="setting-hint">{{ t("settings.connections.tokenHint") }}</p>
            </div>
            <StatusPill :tone="writeToken ? 'ready' : 'planned'" :label="writeToken ? t('settings.connections.tokenSet') : t('settings.connections.tokenUnset')" />
          </div>
          <div class="token-form">
            <input v-model="writeToken" type="password" autocomplete="off" :placeholder="t('settings.connections.tokenPlaceholder')" />
            <button type="button" class="link-button" @click="saveWriteToken">{{ t("settings.connections.save") }}</button>
            <button type="button" class="link-button" @click="clearWriteToken">{{ t("settings.connections.clear") }}</button>
          </div>
          <div v-if="tokenSaved" class="empty-note">{{ t("settings.connections.saved") }}</div>

          <div class="connection-toolbar">
            <button type="button" class="link-button" @click="openAddConnection">{{ connectionCopy.add }}</button>
            <button type="button" class="link-button" @click="reloadConnections()">{{ t("common.refresh") }}</button>
          </div>

          <form v-if="connectionFormOpen" id="connection-form" class="connection-form" @submit.prevent="saveConnection">
            <div class="connection-form-head">
              <strong>{{ editingConnectionId ? connectionCopy.formTitleEdit : connectionCopy.formTitleAdd }}</strong>
              <span v-if="editingConnectionId" class="mono">#{{ editingConnectionId }}</span>
            </div>
            <p v-if="editingConnectionId" class="setting-hint">{{ connectionCopy.editHint }}</p>
            <div class="connection-form-grid">
              <label>
                <span>{{ connectionCopy.kind }}</span>
                <select v-model="connectionForm.kind">
                  <option v-for="kind in Object.keys(CONNECTION_KIND_LABELS) as ConnectionKind[]" :key="kind" :value="kind">
                    {{ connectionKindLabel(kind) }}
                  </option>
                </select>
              </label>
              <label>
                <span>{{ connectionCopy.name }}</span>
                <input v-model="connectionForm.label" maxlength="80" autocomplete="off" placeholder="default" />
              </label>
              <label class="connection-form-wide">
                <span>{{ connectionCopy.endpoint }}</span>
                <input v-model="connectionForm.endpoint" required autocomplete="off" :placeholder="connectionCopy.endpointHint" />
                <small>{{ connectionCopy.endpointHint }}</small>
              </label>
              <label>
                <span>{{ connectionCopy.token }}</span>
                <input v-model="connectionForm.token" type="password" autocomplete="new-password" />
                <small>{{ connectionCopy.tokenHint }}</small>
              </label>
              <label v-if="isMailKind">
                <span>{{ connectionCopy.username }}</span>
                <input v-model="connectionForm.username" autocomplete="off" />
              </label>
              <label v-if="isMailKind">
                <span>{{ connectionCopy.from }}</span>
                <input v-model="connectionForm.from" autocomplete="off" placeholder="digilegacy@example.com" />
              </label>
              <label v-if="isMailKind">
                <span>{{ connectionCopy.security }}</span>
                <select v-model="connectionForm.security">
                  <option value="tls">TLS</option>
                  <option value="starttls">STARTTLS</option>
                  <option value="none">None</option>
                </select>
              </label>
              <label class="connection-check">
                <input v-model="connectionForm.enabled" type="checkbox" />
                <span>{{ connectionCopy.enabled }}</span>
              </label>
            </div>
            <div class="connection-form-actions">
              <button type="button" class="link-button" @click="closeConnectionForm">{{ connectionCopy.cancel }}</button>
              <button type="submit" class="solid-button" :disabled="connectionSaving">
                {{ connectionSaving ? connectionCopy.saving : connectionCopy.save }}
              </button>
            </div>
            <p v-if="connectionFormError" class="connection-error">{{ connectionFormError }}</p>
          </form>

          <div v-if="connectionsLoading" class="empty-note">{{ t("common.loading") }}</div>
          <div v-else-if="connectionsLoadError" class="empty-note">{{ connectionsLoadError }}</div>
          <div v-else-if="!connections?.items.length" class="empty-note">{{ t("settings.connections.empty") }}</div>
          <div v-else class="connection-list">
            <div v-for="item in connections.items" :key="item.id" class="connection-row">
              <div class="connection-main">
                <strong>{{ connectionKindLabel(item.kind) }}</strong>
                <span class="mono">{{ connectionSourceLabel(item) }}</span>
              </div>
              <StatusPill
                :tone="item.enabled ? connectionTone(item.lastStatus) : 'planned'"
                :label="connectionRowStatus(item)"
              />
              <span class="mono connection-checked">
                {{ item.lastCheckAt ? formatDateTime(item.lastCheckAt, locale) : "—" }}
              </span>
              <div class="connection-actions">
                <button
                  type="button"
                  class="link-button"
                  :disabled="!item.enabled || testingId === item.id"
                  @click="testConnection(item)"
                >
                  {{ testingId === item.id ? connectionCopy.testing : connectionCopy.test }}
                </button>
                <button v-if="item.source === 'stored'" type="button" class="link-button" @click="openEditConnection(item)">
                  {{ connectionCopy.edit }}
                </button>
                <button v-if="item.source === 'stored'" type="button" class="link-button danger-link" @click="removeConnection(item)">
                  {{ connectionCopy.remove }}
                </button>
              </div>
              <p v-if="item.lastError" class="connection-error">{{ item.lastError }}</p>
            </div>
          </div>
          <div v-if="connectionNotice" class="connection-notice">{{ connectionNotice }}</div>
          <div v-if="connectionError" class="connection-error">{{ connectionError }}</div>
        </SectionCard>

        <SectionCard v-else-if="active === 'audit'" :title="t('settings.tab.audit')" :subtitle="t('settings.tab.audit.note')">
          <div class="audit-toolbar">
            <label class="select-field">
              <span>{{ t("settings.audit.kind") }}</span>
              <select v-model="auditKind" @change="audit.reload()">
                <option value="">{{ t("common.all") }}</option>
                <option value="asset.create">asset.create</option>
                <option value="asset.update">asset.update</option>
                <option value="policy.provenance.set">policy.provenance.set</option>
                <option value="deliver.export">deliver.export</option>
                <option value="trigger.checkin">trigger.checkin</option>
              </select>
            </label>
            <span class="mono audit-total">{{ t("settings.audit.count", { count: audit.data.value?.total ?? 0 }) }}</span>
          </div>
          <div v-if="audit.loading.value" class="empty-note">{{ t("common.loading") }}</div>
          <div v-else-if="audit.error.value" class="empty-note">{{ audit.error.value }}</div>
          <div v-else-if="!audit.data.value?.items.length" class="empty-note">{{ t("settings.tab.audit.empty") }}</div>
          <div v-else class="audit-table-wrap">
            <table class="audit-table">
              <thead>
                <tr>
                  <th>{{ t("settings.audit.time") }}</th>
                  <th>{{ t("settings.audit.kind") }}</th>
                  <th>{{ t("settings.audit.subject") }}</th>
                  <th>{{ t("settings.audit.outcome") }}</th>
                  <th>hash</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="entry in audit.data.value.items" :key="entry.id">
                  <td class="mono">{{ entry.at }}</td>
                  <td class="mono">{{ entry.kind }}</td>
                  <td>{{ entry.subject }}</td>
                  <td><StatusPill :tone="entry.outcome === 'ok' ? 'ready' : 'planned'" :label="entry.outcome" /></td>
                  <td class="mono hash-cell" :title="entry.hash">{{ entry.hash.slice(0, 12) }}…</td>
                </tr>
              </tbody>
            </table>
          </div>
        </SectionCard>

        <SectionCard v-else :title="t('settings.tab.about')" :subtitle="t('settings.about.note')">
          <div class="about-grid">
            <div class="about-item">
              <span class="mono">{{ t("settings.about.version") }}</span>
              <strong>0.0.16</strong>
            </div>
            <div class="about-item">
              <span class="mono">{{ t("settings.about.contracts") }}</span>
              <strong>{{ COMMAND_NAMES.length }}</strong>
            </div>
            <div class="about-item">
              <span class="mono">{{ t("settings.about.modules") }}</span>
              <strong>54</strong>
            </div>
          </div>
          <div class="about-note">
            <AppIcon name="info" :size="15" />
            <span>{{ t("settings.about.note") }}</span>
          </div>
        </SectionCard>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  max-width: 1100px;
  margin: 0 auto;
}

.settings-shell {
  display: grid;
  grid-template-columns: 220px minmax(0, 1fr);
  gap: 24px;
  align-items: start;
}

.settings-tabs {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
}

.settings-tab {
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  color: var(--muted);
  font-size: 12px;
  text-align: left;
  transition:
    color var(--fast) var(--ease),
    background var(--fast) var(--ease);
}

.settings-tab:hover {
  color: var(--ink);
}

.settings-tab.active {
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-weight: 600;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 18px;
  padding: 15px 0;
  border-top: 1px solid var(--line);
  color: var(--ink-soft);
  font-size: 12px;
}

.setting-row:first-child {
  border-top: 0;
}

.setting-row > :first-child {
  color: var(--muted);
}

.audit-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 0 0 10px; }
.audit-toolbar .select-field { display: flex; align-items: center; gap: 9px; min-height: 34px; padding: 0 12px; border: 1px solid var(--line); border-radius: 999px; background: var(--surface); color: var(--muted); font-size: 12px; }
.audit-toolbar select { border: 0; outline: 0; background: transparent; color: var(--ink); font: inherit; font-size: 12px; }
.audit-total { color: var(--muted); font-size: 11px; }
.audit-table-wrap { overflow-x: auto; }
.audit-table { width: 100%; border-collapse: collapse; font-size: 11px; }
.audit-table th { padding: 8px 10px; border-bottom: 1px solid var(--line); color: var(--muted); font-weight: 500; text-align: left; white-space: nowrap; }
.audit-table td { padding: 9px 10px; border-bottom: 1px solid var(--line); color: var(--ink-soft); vertical-align: middle; }
.audit-table .hash-cell { color: var(--muted); }
.connection-guide { display: grid; gap: 12px; margin: 0 0 16px; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: color-mix(in srgb, var(--accent) 5%, var(--surface)); }
.connection-guide-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
.connection-guide-head strong { color: var(--ink); font-size: 13px; }
.connection-guide-head .setting-hint { margin: 6px 0 0; max-width: 68ch; }
.connection-guide-steps { display: grid; gap: 6px; margin: 0; padding-left: 18px; color: var(--ink-soft); font-size: 11px; line-height: 1.55; }
.connection-guide-actions { display: flex; flex-wrap: wrap; gap: 8px 14px; align-items: center; }
.connection-guide-actions .link-button { min-height: 30px; padding: 5px 10px; border: 1px solid var(--line); border-radius: 999px; background: var(--surface); }
.connection-guide-actions .link-button:hover { border-color: var(--accent); color: var(--accent-strong); }
.token-row { align-items: flex-start; }
.setting-hint { max-width: 420px; margin: 6px 0 0; color: var(--muted); font-size: 11px; line-height: 1.5; }
.token-form { display: flex; align-items: center; gap: 8px; padding: 0 0 12px; }
.token-form input { flex: 1; min-width: 0; padding: 8px 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); color: var(--ink); font: inherit; font-size: 12px; }
.segmented {
  display: flex;
  gap: 4px;
  padding: 3px;
  border: 1px solid var(--line);
  border-radius: 999px;
  background: var(--surface-soft);
}

.segmented button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border-radius: 999px;
  color: var(--muted);
  font-size: 11px;
}

.segmented button.selected {
  background: var(--surface);
  color: var(--ink);
  box-shadow: var(--shadow-sm);
}

.empty-note {
  margin: 8px 0 2px;
  padding: 18px;
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-sm);
  color: var(--muted);
  font-size: 12px;
}

.about-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

.about-item {
  display: flex;
  flex-direction: column;
  gap: 7px;
  padding: 14px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
}

.about-item span {
  color: var(--muted);
  font-size: 9px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.about-item strong {
  color: var(--ink);
  font-family: var(--font-display);
  font-size: 18px;
  font-weight: 500;
}

.about-note {
  display: flex;
  gap: 8px;
  margin-top: 14px;
  color: var(--muted);
  font-size: 11px;
}

.connection-toolbar { display: flex; align-items: center; justify-content: flex-end; gap: 12px; padding: 12px 0 8px; border-top: 1px solid var(--line); }
.connection-toolbar .setting-hint { margin: 0; }
.connection-list { display: grid; gap: 0; border-top: 1px solid var(--line); }
.connection-row { display: grid; grid-template-columns: minmax(150px, 1.5fr) auto auto auto; gap: 12px; align-items: center; padding: 12px 0; border-bottom: 1px solid var(--line); min-width: 0; }
.connection-main { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.connection-main strong { color: var(--ink); font-size: 12px; font-weight: 600; }
.connection-main .mono { color: var(--muted); font-size: 10px; overflow-wrap: anywhere; }
.connection-checked { color: var(--muted); font-size: 10px; white-space: nowrap; }
.connection-row .link-button:disabled { cursor: not-allowed; opacity: 0.55; }
.connection-notice { margin-top: 10px; padding: 9px 12px; border: 1px solid color-mix(in srgb, var(--accent) 24%, transparent); border-radius: var(--radius-sm); background: var(--accent-soft); color: var(--accent-strong); font-size: 11px; overflow-wrap: anywhere; }
.connection-error { grid-column: 1 / -1; margin: 0; color: var(--danger); font-size: 11px; line-height: 1.5; overflow-wrap: anywhere; }
.connection-row .connection-error { padding-bottom: 2px; }
.connection-actions { display: flex; align-items: center; gap: 10px; justify-self: end; }
.danger-link { color: var(--danger); }
.connection-form { display: grid; gap: 12px; margin: 4px 0 14px; padding: 16px; border: 1px solid var(--line-strong); border-radius: var(--radius-sm); background: var(--surface-soft); }
.connection-form-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; color: var(--ink); font-size: 12px; }
.connection-form-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.connection-form-grid label { display: flex; flex-direction: column; gap: 6px; min-width: 0; color: var(--muted); font-size: 10px; text-transform: uppercase; letter-spacing: 0.04em; }
.connection-form-grid input, .connection-form-grid select { min-width: 0; padding: 8px 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface); color: var(--ink); font: inherit; font-size: 12px; text-transform: none; letter-spacing: 0; }
.connection-form-grid small { color: var(--muted); font-size: 10px; line-height: 1.4; text-transform: none; letter-spacing: 0; }
.connection-form-wide { grid-column: 1 / -1; }
.connection-check { flex-direction: row !important; align-items: center; gap: 7px !important; text-transform: none !important; letter-spacing: 0 !important; }
.connection-check input { width: auto; }
.connection-form-actions { display: flex; justify-content: flex-end; gap: 10px; }
.connection-form-actions .solid-button { padding: 8px 14px; border: 1px solid var(--accent-strong); border-radius: var(--radius-sm); background: var(--accent-strong); color: var(--surface); font-size: 11px; }
.connection-form-actions .solid-button:disabled { opacity: 0.55; }
@media (max-width: 640px) {
  .connection-form-grid { grid-template-columns: 1fr; }
  .connection-form-wide { grid-column: auto; }
  .connection-row { grid-template-columns: minmax(0, 1fr) auto; gap: 8px 10px; }
  .connection-checked { grid-column: 1; }
  .connection-row .link-button { justify-self: end; }
  .connection-actions { grid-column: 1 / -1; justify-self: stretch; justify-content: flex-end; }
  .token-form { flex-wrap: wrap; }
  .connection-toolbar { align-items: flex-start; }
  .connection-guide-head { flex-direction: column; }
  .connection-guide-actions .link-button { flex: 1 1 44%; }
}
@media (max-width: 900px) {
  .settings-shell {
    grid-template-columns: 1fr;
  }

  .settings-tabs {
    flex-direction: row;
    flex-wrap: wrap;
  }

  .about-grid {
    grid-template-columns: 1fr;
  }
}
</style>
