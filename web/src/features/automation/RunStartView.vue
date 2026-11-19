<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SectionCard from "@/shared/ui/SectionCard.vue";
import { useI18n } from "@/platform/i18n";
import { platformClient } from "@/platform/client";

const router = useRouter();
const { t } = useI18n();
const task = ref("");
const stepsText = ref('[\n  { "tool": "app.overview", "args": {} }\n]');
const submitting = ref(false);
const planning = ref(false);
const planModel = ref("");
const error = ref("");

async function generatePlan() {
  error.value = "";
  planModel.value = "";
  if (!task.value.trim()) {
    error.value = t("runs.start.taskRequired");
    return;
  }
  planning.value = true;
  try {
    const envelope = await platformClient.planRun(task.value.trim());
    if (!envelope.success || !envelope.data) {
      error.value = envelope.code === "UNSUPPORTED" ? t("runs.start.plannerMissing") : envelope.message || envelope.code;
      return;
    }
    stepsText.value = JSON.stringify(envelope.data.steps, null, 2);
    planModel.value = envelope.data.model;
  } finally {
    planning.value = false;
  }
}

async function submit() {
  error.value = "";
  let steps: Array<{ tool: string; args: Record<string, unknown> }>;
  try {
    const parsed = JSON.parse(stepsText.value);
    if (!Array.isArray(parsed)) throw new Error("steps 必须是数组");
    steps = parsed;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    return;
  }
  if (!task.value.trim()) {
    error.value = t("runs.start.taskRequired");
    return;
  }
  submitting.value = true;
  try {
    const envelope = await platformClient.startRun(task.value.trim(), steps);
    if (!envelope.success || !envelope.data) {
      error.value = envelope.code === "AUTH_REQUIRED" ? t("runs.start.tokenRequired") : envelope.message || envelope.code;
      return;
    }
    if (envelope.data.status === "waiting_approval") {
      router.push("/automation/approvals");
    } else {
      router.push(`/automation/runs/${envelope.data.id}`);
    }
  } finally {
    submitting.value = false;
  }
}
</script>

<template>
  <div class="run-start-page">
    <PageHeader eyebrow="AUTOMATION / RUN" :title="t('runs.start.title')" :description="t('runs.start.subtitle')">
      <template #actions>
        <AppButton variant="secondary" size="sm" @click="router.push('/automation')">
          <AppIcon name="arrow-right" :size="14" class="back-arrow" />
          {{ t("runs.detail.back") }}
        </AppButton>
      </template>
    </PageHeader>

    <SectionCard :title="t('runs.start.formTitle')" :subtitle="t('runs.start.formSubtitle')">
      <div class="run-form">
        <label>
          <span>{{ t("runs.start.task") }}</span>
          <input v-model="task" type="text" maxlength="200" :placeholder="t('runs.start.taskPlaceholder')" />
        </label>
        <label>
          <span>{{ t("runs.start.steps") }}</span>
          <textarea v-model="stepsText" rows="9" spellcheck="false" />
        </label>
        <p class="run-hint">{{ t("runs.start.hint") }}</p>
        <p v-if="planModel" class="run-plan-model mono">{{ t("runs.start.planModel", { model: planModel }) }}</p>
        <div v-if="error" class="run-error">{{ error }}</div>
        <div class="run-actions">
          <AppButton variant="primary" :disabled="submitting || planning" @click="submit">
            {{ submitting ? t("runs.start.submitting") : t("runs.start.submit") }}
            <AppIcon name="arrow-right" :size="14" />
          </AppButton>
          <AppButton variant="secondary" :disabled="submitting || planning" @click="generatePlan">
            {{ planning ? t("runs.start.planning") : t("runs.start.plan") }}
          </AppButton>
          <AppButton variant="ghost" @click="router.push('/automation/tools')">{{ t("runs.start.tools") }}</AppButton>
        </div>
      </div>
    </SectionCard>
  </div>
</template>

<style scoped>
.back-arrow { transform: rotate(180deg); }
.run-start-page { max-width: 760px; margin: 0 auto; }
.run-form { display: grid; gap: 14px; }
.run-form label { display: grid; gap: 7px; }
.run-form label > span { color: var(--muted); font-size: 11px; letter-spacing: 0.04em; }
.run-form input, .run-form textarea { width: 100%; padding: 10px 12px; border: 1px solid var(--line); border-radius: var(--radius-sm); background: var(--surface-soft); color: var(--ink); font: inherit; font-size: 12px; }
.run-form textarea { resize: vertical; font-family: var(--font-mono); line-height: 1.55; }
.run-hint { margin: 0; color: var(--muted); font-size: 11px; line-height: 1.6; }
.run-plan-model { margin: 0; color: var(--muted); font-size: 10px; letter-spacing: 0.03em; }
.run-error { padding: 9px 11px; border: 1px solid var(--line); border-radius: var(--radius-sm); color: var(--danger, #b42318); font-size: 12px; }
.run-actions { display: flex; gap: 9px; }
</style>
