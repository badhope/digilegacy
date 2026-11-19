<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import AppIcon from "@/shared/icons/AppIcon.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SectionCard from "@/shared/ui/SectionCard.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { COMMAND_NAMES } from "@/platform/generated/ipc";
import { useI18n } from "@/platform/i18n";
import { useTheme } from "@/platform/theme";

const route = useRoute();
const router = useRouter();
const { t, locale, setLocale } = useI18n();
const { theme, setTheme } = useTheme();

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
        <StatusPill tone="planned" :label="t('app.mode')" />
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
          <div class="empty-note">{{ t("settings.tab.security.empty") }}</div>
        </SectionCard>

        <SectionCard v-else-if="active === 'data'" :title="t('settings.tab.data')" :subtitle="t('settings.tab.data.note')">
          <div class="empty-note">{{ t("settings.tab.data.empty") }}</div>
        </SectionCard>

        <SectionCard v-else-if="active === 'connections'" :title="t('settings.tab.connections')" :subtitle="t('settings.tab.connections.note')">
          <div class="empty-note">{{ t("settings.tab.connections.empty") }}</div>
        </SectionCard>

        <SectionCard v-else-if="active === 'audit'" :title="t('settings.tab.audit')" :subtitle="t('settings.tab.audit.note')">
          <div class="empty-note">{{ t("settings.tab.audit.empty") }}</div>
        </SectionCard>

        <SectionCard v-else :title="t('settings.tab.about')" :subtitle="t('settings.about.note')">
          <div class="about-grid">
            <div class="about-item">
              <span class="mono">{{ t("settings.about.version") }}</span>
              <strong>0.7.0-m0</strong>
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
