<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import EmptyState from "@/shared/ui/EmptyState.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";
import { TOOL_CATALOG } from "@/platform/generated/tools";

const router = useRouter();
const { t } = useI18n();
const query = ref("");
const permission = ref("all");

const permissions = computed(() => ["all", ...Array.from(new Set(TOOL_CATALOG.map((tool) => tool.permission)))]);
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  return TOOL_CATALOG.filter((tool) => {
    const matchesPermission = permission.value === "all" || tool.permission === permission.value;
    const matchesQuery = !q || tool.name.toLowerCase().includes(q) || tool.title.toLowerCase().includes(q);
    return matchesPermission && matchesQuery;
  });
});

function tone(value: string): "ready" | "planned" | "degraded" | "danger" | "neutral" {
  if (value === "read") return "ready";
  if (value === "write") return "danger";
  if (value === "draft" || value === "network") return "degraded";
  return "neutral";
}
</script>

<template>
  <div class="tools-page">
    <PageHeader eyebrow="AUTOMATION / TOOLS" :title="t('automation.tools.title')" :description="t('automation.tools.subtitle')">
      <template #actions>
        <AppButton variant="secondary" size="sm" @click="router.push('/automation')">
          <AppIcon name="arrow-right" :size="14" class="back-arrow" />
          {{ t("tools.back") }}
        </AppButton>
        <StatusPill tone="ready" :label="t('tools.count', { count: TOOL_CATALOG.length })" />
      </template>
    </PageHeader>

    <div class="tool-toolbar">
      <input v-model="query" type="search" :placeholder="t('tools.search')" />
      <select v-model="permission">
        <option v-for="item in permissions" :key="item" :value="item">
          {{ item === "all" ? t("common.all") : item }}
        </option>
      </select>
      <span class="mono count">{{ t("tools.count", { count: filtered.length }) }}</span>
    </div>

    <div v-if="filtered.length" class="tool-grid">
      <article v-for="tool in filtered" :key="tool.name" class="tool-card">
        <header>
          <div>
            <span class="mono tool-name">{{ tool.name }}</span>
            <h2>{{ tool.title }}</h2>
          </div>
          <StatusPill :tone="tone(tool.permission)" :label="tool.permission" />
        </header>
        <p class="tool-desc">{{ tool.description }}</p>
        <dl class="tool-meta mono">
          <div><dt>approval</dt><dd>{{ tool.approval }}</dd></div>
          <div><dt>mcp</dt><dd>{{ tool.mcp ? "yes" : "no" }}</dd></div>
          <div><dt>audit</dt><dd>{{ tool.audit ? "yes" : "no" }}</dd></div>
          <div><dt>rate</dt><dd>{{ tool.rateLimit }}</dd></div>
        </dl>
        <div class="tool-schemas mono">
          <span>in {{ tool.inputSchema }}</span>
          <span>out {{ tool.outputSchema }}</span>
        </div>
        <div class="tool-access">
          <span v-for="scope in tool.availableTo" :key="scope" class="scope mono">{{ scope }}</span>
        </div>
      </article>
    </div>
    <EmptyState v-else :title="t('tools.empty')" :note="t('tools.emptyNote')" icon="command" />
  </div>
</template>

<style scoped>
.back-arrow { transform: rotate(180deg); }
.tools-page { max-width: var(--content-max); margin: 0 auto; }
.tool-toolbar { display: flex; align-items: center; gap: 10px; margin-bottom: 14px; }
.tool-toolbar input, .tool-toolbar select { min-height: 36px; padding: 0 12px; border: 1px solid var(--line); border-radius: 999px; background: var(--surface); color: var(--ink); font: inherit; font-size: 12px; }
.tool-toolbar input { flex: 1; }
.count { color: var(--muted); font-size: 11px; white-space: nowrap; }
.tool-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.tool-card { padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
.tool-card header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
.tool-name { color: var(--muted); font-size: 10px; }
.tool-card h2 { margin: 4px 0 0; font-size: 15px; }
.tool-desc { min-height: 34px; margin: 10px 0; color: var(--ink-soft); font-size: 12px; line-height: 1.5; }
.tool-meta { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; margin: 0; padding: 10px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); }
.tool-meta div { min-width: 0; }
.tool-meta dt { color: var(--muted); font-size: 9px; text-transform: uppercase; }
.tool-meta dd { margin: 3px 0 0; font-size: 10px; overflow-wrap: anywhere; }
.tool-schemas { display: flex; flex-wrap: wrap; gap: 12px; margin-top: 10px; color: var(--muted); font-size: 10px; }
.tool-access { display: flex; gap: 6px; margin-top: 10px; }
.scope { padding: 2px 7px; border: 1px solid var(--line); border-radius: 999px; color: var(--muted); font-size: 9px; }
@media (max-width: 720px) { .tool-grid { grid-template-columns: 1fr; } .tool-meta { grid-template-columns: repeat(2, 1fr); } }
</style>
