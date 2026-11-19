<script setup lang="ts">
import { computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import AppIcon from "@/shared/icons/AppIcon.vue";
import AppButton from "@/shared/ui/AppButton.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SectionCard from "@/shared/ui/SectionCard.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { COMMAND_PERMISSION, type CommandName } from "@/platform/generated/ipc";
import { useI18n } from "@/platform/i18n";

const route = useRoute();
const router = useRouter();
const { t } = useI18n();

const titleKey = computed(() => String(route.meta.titleKey ?? "empty.title"));
const eyebrowKey = computed(() => String(route.meta.eyebrowKey ?? "app.mode"));
const descriptionKey = computed(() => String(route.meta.descriptionKey ?? "feature.scaffold.note"));
const readinessKey = computed(() => String(route.meta.readinessKey ?? "feature.scaffold.note"));
const modules = computed(() => (route.meta.modules as string[] | undefined) ?? []);
const commands = computed(() => (route.meta.commands as string[] | undefined) ?? []);

function permission(command: string): string {
  return COMMAND_PERMISSION[command as CommandName] ?? "read";
}
</script>

<template>
  <div class="scaffold-page">
    <PageHeader :eyebrow="t(eyebrowKey)" :title="t(titleKey)" :description="t(descriptionKey)">
      <template #actions>
        <StatusPill tone="planned" :label="t('feature.scaffold.badge')" />
      </template>
    </PageHeader>

    <div class="scaffold-banner">
      <span class="scaffold-icon"><AppIcon name="shield" :size="20" /></span>
      <div>
        <strong>{{ t(readinessKey) }}</strong>
        <p>{{ t("feature.scaffold.note") }}</p>
      </div>
    </div>

    <div class="scaffold-grid">
      <SectionCard :title="t('feature.scaffold.modules')" :subtitle="t('feature.scaffold.note')">
        <div class="module-list">
          <div v-for="module in modules" :key="module" class="module-row">
            <span class="mono module-id">{{ module }}</span>
            <span>{{ t("common.planned") }}</span>
            <StatusPill tone="planned" :label="t('common.planned')" />
          </div>
        </div>
      </SectionCard>

      <SectionCard :title="t('feature.scaffold.contracts')" :subtitle="t('feature.scaffold.note')">
        <div class="contract-list">
          <div v-for="command in commands" :key="command" class="contract-row">
            <span class="mono">{{ command }}</span>
            <span class="mono contract-permission">{{ permission(command) }}</span>
          </div>
        </div>
        <div class="scaffold-foot">
          <AppButton variant="ghost" size="sm" @click="router.push('/overview')">
            {{ t("nav.overview") }}
            <AppIcon name="arrow-right" :size="14" />
          </AppButton>
        </div>
      </SectionCard>
    </div>
  </div>
</template>

<style scoped>
.scaffold-page {
  max-width: 1100px;
  margin: 0 auto;
}

.scaffold-banner {
  display: flex;
  align-items: center;
  gap: 14px;
  margin-bottom: 16px;
  padding: 18px 20px;
  border: 1px solid color-mix(in srgb, var(--accent) 24%, var(--line));
  border-radius: var(--radius-md);
  background: var(--accent-soft);
}

.scaffold-icon {
  display: grid;
  flex-shrink: 0;
  width: 42px;
  height: 42px;
  border-radius: 50%;
  color: var(--accent-strong);
  place-items: center;
  background: color-mix(in srgb, var(--surface) 70%, transparent);
}

.scaffold-banner strong {
  display: block;
  color: var(--ink);
  font-size: 13px;
}

.scaffold-banner p {
  margin: 3px 0 0;
  color: var(--muted);
  font-size: 11px;
}

.scaffold-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.module-list,
.contract-list {
  display: flex;
  flex-direction: column;
}

.module-row,
.contract-row {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 12px 0;
  border-top: 1px solid var(--line);
  color: var(--ink-soft);
  font-size: 12px;
}

.module-row:first-child,
.contract-row:first-child {
  border-top: 0;
}

.module-id {
  min-width: 82px;
  color: var(--muted);
  font-size: 9px;
}

.module-row > :nth-child(2) {
  flex: 1;
}

.contract-row {
  justify-content: space-between;
}

.contract-permission {
  color: var(--muted);
  font-size: 9px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.scaffold-foot {
  display: flex;
  justify-content: flex-end;
  margin-top: 14px;
}

@media (max-width: 800px) {
  .scaffold-grid {
    grid-template-columns: 1fr;
  }
}
</style>
