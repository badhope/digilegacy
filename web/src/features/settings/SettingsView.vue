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
import { loadAudit, loadSecurity } from "./api";

const route = useRoute();
const router = useRouter();
const { t, locale, setLocale } = useI18n();
const { theme, setTheme } = useTheme();
const { data: security, loading: securityLoading, error: securityError, reload: reloadSecurity } = useResource(loadSecurity);
const auditKind = ref("");
const audit = useResource(() => loadAudit({ limit: 20, ...(auditKind.value ? { kind: auditKind.value } : {}) }));

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
          <div class="empty-note">{{ t("settings.connections.empty") }}</div>
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
              <strong>0.8.4-m4d</strong>
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
