<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import ErrorState from "@/shared/ui/ErrorState.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import SectionCard from "@/shared/ui/SectionCard.vue";
import SkeletonBlock from "@/shared/ui/SkeletonBlock.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { formatDate } from "@/shared/format";
import { useI18n } from "@/platform/i18n";
import { useResource } from "@/platform/useResource";
import { platformClient } from "@/platform/client";
import type { NotificationItem, TriggerEvent } from "@/platform/types";
import { loadPlan } from "./api";

const router = useRouter();
const { t, locale } = useI18n();
const { data, loading, error, reload, refresh } = useResource(loadPlan);

const heirCount = computed(() => data.value?.heirs.length ?? 0);
const triggerMode = computed(() => {
  const mode = data.value?.trigger.mode ?? "";
  return mode === "dms" ? t("planning.trigger.mode.dms") : mode;
});
const triggerStatus = computed(() => {
  const status = data.value?.trigger.status ?? "";
  const key = `planning.trigger.status.${status}`;
  return t(key) === key ? status : t(key);
});

const acting = ref(false);
const actionError = ref("");
const notice = ref("");
const draftDays = ref(30);
const draftEnabled = ref(false);
const triggerBusy = ref(false);
const triggerError = ref("");
const triggerNotice = ref("");
const events = ref<TriggerEvent[]>([]);
const eventsLoading = ref(false);
const eventsError = ref("");
const notifications = ref<NotificationItem[]>([]);
const notificationsLoading = ref(false);
const notificationsError = ref("");
const notificationsNotice = ref("");
const notifying = ref(false);

watch(
  data,
  (value) => {
    if (!value) return;
    draftDays.value = value.trigger.days;
    draftEnabled.value = value.trigger.status !== "off";
  },
  { immediate: true },
);

async function loadEvents() {
  eventsLoading.value = true;
  eventsError.value = "";
  try {
    const envelope = await platformClient.triggerEvents({ limit: 10 });
    if (!envelope.success || !envelope.data) {
      eventsError.value = envelope.message || envelope.code;
      return;
    }
    events.value = envelope.data.items;
  } catch (cause) {
    eventsError.value = cause instanceof Error ? cause.message : "trigger events failed";
  } finally {
    eventsLoading.value = false;
  }
}

async function loadNotifications() {
  notificationsLoading.value = true;
  notificationsError.value = "";
  try {
    const envelope = await platformClient.listNotifications({ limit: 10 });
    if (!envelope.success || !envelope.data) {
      notificationsError.value = envelope.message || envelope.code;
      return;
    }
    notifications.value = envelope.data.items;
  } catch (cause) {
    notificationsError.value = cause instanceof Error ? cause.message : "notifications failed";
  } finally {
    notificationsLoading.value = false;
  }
}

onMounted(() => {
  void loadEvents();
  void loadNotifications();
});

function notificationStatus(status: string): string {
  const key = `planning.notification.status.${status}`;
  return t(key) === key ? status : t(key);
}

function notificationTone(status: string): "ready" | "planned" | "danger" | "warn" {
  if (status === "sent") return "ready";
  if (status === "failed") return "danger";
  if (status === "blocked") return "warn";
  return "planned";
}

async function sendNotifications() {
  notifying.value = true;
  notificationsNotice.value = "";
  notificationsError.value = "";
  try {
    const envelope = await platformClient.sendNotifications({ retryFailed: true });
    if (!envelope.success || !envelope.data) {
      notificationsError.value = envelope.code === "AUTH_REQUIRED" ? t("runs.detail.tokenRequired") : envelope.message || envelope.code;
      return;
    }
    const result = envelope.data;
    notificationsNotice.value = result.configured
      ? t("planning.notifications.sent", { sent: result.sent, failed: result.failed, pending: result.pending })
      : t("planning.notifications.notConfigured");
    await loadNotifications();
  } catch (cause) {
    notificationsError.value = cause instanceof Error ? cause.message : "notification send failed";
  } finally {
    notifying.value = false;
  }
}

function eventKind(kind: string): string {
  const key = `planning.event.kind.${kind}`;
  return t(key) === key ? kind : t(key);
}

async function saveTrigger(pauseDays?: number) {
  triggerBusy.value = true;
  triggerError.value = "";
  triggerNotice.value = "";
  try {
    const args: { enabled: boolean; days: number; pauseDays?: number } = {
      enabled: draftEnabled.value,
      days: Number(draftDays.value),
    };
    if (pauseDays !== undefined) args.pauseDays = pauseDays;
    const envelope = await platformClient.configureTrigger(args);
    if (!envelope.success || !envelope.data) {
      triggerError.value = envelope.code === "AUTH_REQUIRED" ? t("runs.detail.tokenRequired") : envelope.message || envelope.code;
      return;
    }
    triggerNotice.value =
      pauseDays === 0
        ? t("planning.trigger.resumed")
        : pauseDays
          ? t("planning.trigger.pausedFor", { days: pauseDays })
          : t("planning.trigger.saved");
    await refresh();
    await loadEvents();
  } catch (cause) {
    triggerError.value = cause instanceof Error ? cause.message : "trigger update failed";
  } finally {
    triggerBusy.value = false;
  }
}

async function togglePause() {
  draftEnabled.value = true;
  await saveTrigger(data.value?.trigger.status === "paused" ? 0 : 7);
}

async function evaluateTrigger() {
  triggerBusy.value = true;
  triggerError.value = "";
  triggerNotice.value = "";
  try {
    const envelope = await platformClient.evaluateTrigger();
    if (!envelope.success || !envelope.data) {
      triggerError.value = envelope.code === "AUTH_REQUIRED" ? t("runs.detail.tokenRequired") : envelope.message || envelope.code;
      return;
    }
    triggerNotice.value = t("planning.trigger.evaluated", { status: triggerStatus.value });
    await refresh();
    await loadEvents();
    await loadNotifications();
  } catch (cause) {
    triggerError.value = cause instanceof Error ? cause.message : "trigger evaluation failed";
  } finally {
    triggerBusy.value = false;
  }
}

async function checkIn() {
  acting.value = true;
  actionError.value = "";
  notice.value = "";
  try {
    const envelope = await platformClient.checkin();
    if (!envelope.success || !envelope.data) {
      actionError.value = envelope.code === "AUTH_REQUIRED" ? t("runs.detail.tokenRequired") : envelope.message || envelope.code;
      return;
    }
    notice.value = t("planning.checkin.done", { date: formatDate(envelope.data.checkedInAt, locale.value) });
    await refresh();
  } catch (cause) {
    actionError.value = cause instanceof Error ? cause.message : "check-in failed";
  } finally {
    acting.value = false;
  }
}
</script>

<template>
  <div class="planning-page">
    <PageHeader eyebrow="PLANNING / PEOPLE & TRIGGER" :title="t('planning.title')" :description="t('planning.description')" />

    <div v-if="loading" class="planning-loading">
      <div v-for="index in 3" :key="index" class="plan-skeleton">
        <SkeletonBlock width="100px" height="11px" />
        <SkeletonBlock width="220px" height="28px" />
        <SkeletonBlock width="100%" height="60px" />
      </div>
    </div>

    <ErrorState
      v-else-if="error"
      :title="t('error.generic')"
      :message="error"
      :retry-label="t('common.retry')"
      @retry="reload"
    />

    <template v-else-if="data">
      <div class="planning-grid">
        <SectionCard :title="t('planning.holder')" :subtitle="t('planning.updated')">
          <div class="holder-block">
            <span class="holder-mark"><AppIcon name="shield" :size="21" /></span>
            <div>
              <h3>{{ data.holder.name || t("planning.holder.empty") }}</h3>
              <p class="mono">{{ formatDate(data.holder.updatedAt, locale) }}</p>
            </div>
          </div>
        </SectionCard>

        <SectionCard :title="t('planning.heirs')" :subtitle="`${heirCount} ${t('overview.unit.people')}`">
          <div v-if="!heirCount" class="heir-empty">{{ t("planning.heirs.empty") }}</div>
          <div v-else class="heir-list">
            <div v-for="heir in data.heirs" :key="heir.id" class="heir-row">
              <span class="heir-avatar">{{ heir.name.slice(0, 1) }}</span>
              <span class="heir-copy">
                <strong>{{ heir.name }}</strong>
                <small>{{ heir.role }}</small>
              </span>
            </div>
          </div>
        </SectionCard>

        <SectionCard :title="t('planning.trigger')" :subtitle="t('planning.trigger.status')">
          <div class="trigger-chain">
            <div class="trigger-node active">
              <span class="node-dot" />
              <span>{{ triggerMode }}</span>
            </div>
            <span class="trigger-line" />
            <div class="trigger-node">
              <span class="node-dot" />
              <span>{{ t("planning.trigger.days") }} · {{ data.trigger.days }}</span>
            </div>
            <span class="trigger-line" />
            <div class="trigger-node">
              <span class="node-dot" />
              <span>{{ triggerStatus }}</span>
            </div>
          </div>
          <div class="trigger-meta">
            <div>
              <span>{{ t("planning.checkin.last") }}</span>
              <strong class="mono">{{ formatDate(data.trigger.lastCheckIn, locale) }}</strong>
            </div>
            <div>
              <span>{{ t("planning.checkin.next") }}</span>
              <strong class="mono">{{ formatDate(data.trigger.nextCheckAt, locale) }}</strong>
            </div>
            <div>
              <span>{{ t("planning.trigger.stage") }}</span>
              <strong class="mono">{{ data.trigger.stage || "—" }}</strong>
            </div>
            <div v-if="data.trigger.pausedUntil">
              <span>{{ t("planning.trigger.pausedUntil") }}</span>
              <strong class="mono">{{ formatDate(data.trigger.pausedUntil, locale) }}</strong>
            </div>
          </div>
          <div class="trigger-controls">
            <label class="trigger-toggle">
              <input v-model="draftEnabled" type="checkbox" />
              <span>{{ t("planning.trigger.enable") }}</span>
            </label>
            <label class="trigger-days">
              <span>{{ t("planning.trigger.days") }}</span>
              <input v-model.number="draftDays" type="number" min="1" max="365" />
            </label>
            <AppButton variant="ghost" size="sm" :disabled="triggerBusy" @click="saveTrigger()">
              {{ t("common.save") }}
            </AppButton>
            <AppButton variant="ghost" size="sm" :disabled="triggerBusy" @click="togglePause">
              {{ data.trigger.status === "paused" ? t("planning.trigger.resume") : t("planning.trigger.pause") }}
            </AppButton>
            <AppButton variant="ghost" size="sm" :disabled="triggerBusy" @click="evaluateTrigger">
              {{ triggerBusy ? t("planning.trigger.evaluating") : t("planning.trigger.evaluate") }}
            </AppButton>
          </div>
          <div v-if="triggerNotice" class="trigger-notice">{{ triggerNotice }}</div>
          <div v-else-if="triggerError" class="trigger-error">{{ triggerError }}</div>
          <div class="trigger-foot">
            <AppIcon name="clock" :size="15" />
            <span>{{ t("planning.trigger.note") }}</span>
          </div>
          <div class="trigger-action">
            <AppButton variant="primary" size="sm" :disabled="acting" @click="checkIn">
              {{ acting ? t("planning.checkin.acting") : t("planning.checkin.action") }}
              <AppIcon name="check" :size="14" />
            </AppButton>
            <span v-if="notice" class="trigger-notice">{{ notice }}</span>
            <span v-else-if="actionError" class="trigger-error">{{ actionError }}</span>
          </div>
        </SectionCard>
      </div>

      <SectionCard class="trigger-events" :title="t('planning.events.title')" :subtitle="t('planning.events.subtitle')">
        <div v-if="eventsLoading" class="events-loading" aria-busy="true">
          <SkeletonBlock width="100%" height="42px" />
          <SkeletonBlock width="82%" height="42px" />
          <SkeletonBlock width="70%" height="42px" />
        </div>
        <div v-else-if="eventsError" class="events-error">{{ eventsError }}</div>
        <div v-else-if="!events.length" class="events-empty">{{ t("planning.events.empty") }}</div>
        <div v-else class="event-timeline">
          <div v-for="event in events" :key="event.id" class="event-row">
            <span class="event-dot" />
            <div class="event-copy">
              <div class="event-head">
                <strong>{{ eventKind(event.kind) }}</strong>
                <span class="mono">{{ formatDate(event.createdAt, locale) }} · {{ event.stage || "—" }}</span>
              </div>
              <p>{{ event.detail }}</p>
            </div>
          </div>
        </div>
      </SectionCard>

      <SectionCard
        class="notification-outbox"
        :title="t('planning.notifications.title')"
        :subtitle="t('planning.notifications.subtitle')"
      >
        <div class="notification-head">
          <span class="mono notification-count">{{ t("planning.notifications.count", { count: notifications.length }) }}</span>
          <AppButton variant="secondary" size="sm" :disabled="notifying" @click="sendNotifications">
            {{ notifying ? t("planning.notifications.sending") : t("planning.notifications.send") }}
          </AppButton>
        </div>
        <div v-if="notificationsLoading" class="events-loading" aria-busy="true">
          <SkeletonBlock width="100%" height="42px" />
          <SkeletonBlock width="80%" height="42px" />
        </div>
        <div v-else-if="notificationsError" class="events-error">{{ notificationsError }}</div>
        <div v-else-if="!notifications.length" class="events-empty">{{ t("planning.notifications.empty") }}</div>
        <div v-else class="notification-list">
          <div v-for="item in notifications" :key="item.id" class="notification-row">
            <StatusPill :tone="notificationTone(item.status)" :label="notificationStatus(item.status)" />
            <div class="notification-copy">
              <strong>{{ item.recipient || t("planning.notifications.noRecipient") }}</strong>
              <small>{{ item.recipientRole }} · {{ item.subject }}</small>
              <p v-if="item.lastError">{{ item.lastError }}</p>
            </div>
            <span class="mono notification-time">{{ formatDate(item.createdAt, locale) }}</span>
          </div>
        </div>
        <div v-if="notificationsNotice" class="trigger-notice">{{ notificationsNotice }}</div>
      </SectionCard>

      <div class="planning-note">
        <AppButton variant="ghost" size="sm" @click="router.push('/deliverables')">
          {{ t("nav.deliverables") }}
          <AppIcon name="arrow-right" :size="14" />
        </AppButton>
      </div>
    </template>
  </div>
</template>

<style scoped>
.planning-page {
  max-width: var(--content-max);
  margin: 0 auto;
}

.planning-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
  align-items: start;
}

.holder-block {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 10px 0 20px;
}

.holder-mark {
  display: grid;
  width: 48px;
  height: 48px;
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--accent-strong);
  place-items: center;
  background: var(--accent-soft);
}

.holder-block h3 {
  font-size: 22px;
}

.holder-block p {
  margin: 3px 0 0;
  color: var(--muted);
  font-size: 10px;
}

.plan-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
  color: var(--muted);
  font-size: 11px;
}

.heir-list {
  display: flex;
  flex-direction: column;
}

.heir-row {
  display: flex;
  align-items: center;
  gap: 11px;
  width: 100%;
  padding: 11px 0;
  border-top: 1px solid var(--line);
}

.heir-row:first-child {
  border-top: 0;
}

.heir-avatar {
  display: grid;
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  color: var(--accent-strong);
  font-family: var(--font-display);
  font-size: 15px;
  place-items: center;
  background: var(--surface-soft);
}

.heir-copy {
  flex: 1;
}

.heir-copy strong {
  display: block;
  color: var(--ink);
  font-size: 13px;
  font-weight: 600;
  transition: color var(--fast) var(--ease);
}

.heir-copy small {
  display: block;
  margin-top: 2px;
  color: var(--muted);
  font-size: 10px;
}

.trigger-chain {
  display: flex;
  flex-direction: column;
  gap: 0;
  padding: 8px 0 4px;
}

.trigger-node {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--muted-strong);
  font-size: 12px;
}

.trigger-node.active {
  color: var(--ink);
  font-weight: 600;
}

.node-dot {
  width: 8px;
  height: 8px;
  border: 1px solid var(--line-strong);
  border-radius: 50%;
  background: var(--surface);
}

.trigger-node.active .node-dot {
  border-color: var(--accent);
  background: var(--accent);
  box-shadow: 0 0 0 4px var(--accent-soft);
}

.trigger-line {
  width: 1px;
  height: 22px;
  margin-left: 3px;
  background: var(--line);
}

.trigger-meta { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; margin-top: 16px; padding-top: 12px; border-top: 1px solid var(--line); }
.trigger-meta > div { display: grid; gap: 3px; }
.trigger-meta span { color: var(--muted); font-size: 10px; text-transform: uppercase; letter-spacing: .04em; }
.trigger-meta strong { font-size: 12px; font-weight: 500; }
.trigger-action { display: flex; align-items: center; gap: 10px; margin-top: 12px; flex-wrap: wrap; }
.trigger-notice { color: var(--accent-strong); font-size: 11px; }
.trigger-error { color: var(--danger, #b54949); font-size: 11px; }
.trigger-foot {
  display: flex;
  gap: 8px;
  margin-top: 18px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
  color: var(--muted);
  font-size: 11px;
  line-height: 1.55;
}

.planning-note {
  display: flex;
  justify-content: flex-end;
  margin-top: 12px;
}

.planning-loading {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 16px;
}

.plan-skeleton {
  display: flex;
  flex-direction: column;
  gap: 18px;
  min-height: 220px;
  padding: 22px;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
}

.trigger-controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 12px;
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--line);
}
.trigger-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--muted);
  font-size: 11px;
}
.trigger-days {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--muted);
  font-size: 11px;
}
.trigger-days input {
  width: 58px;
  padding: 5px 6px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--text);
  font: inherit;
}
.trigger-events { margin-top: 16px; }
.events-loading { display: grid; gap: 8px; }
.events-empty, .events-error { padding: 14px 0; color: var(--muted); font-size: 12px; }
.events-error { color: var(--danger, #b54949); }
.event-timeline { display: grid; gap: 0; }
.event-row { display: flex; gap: 12px; padding: 9px 0; }
.event-dot {
  flex: 0 0 auto;
  width: 8px;
  height: 8px;
  margin-top: 5px;
  border-radius: 50%;
  background: var(--accent);
}
.event-copy { flex: 1; min-width: 0; }
.event-head { display: flex; justify-content: space-between; gap: 12px; align-items: baseline; }
.event-head strong { font-size: 12px; font-weight: 500; }
.event-head span { color: var(--muted); font-size: 10px; white-space: nowrap; }
.event-copy p { margin: 3px 0 0; color: var(--muted); font-size: 11px; line-height: 1.5; }

@media (max-width: 1000px) {
  .planning-grid,
  .planning-loading {
    grid-template-columns: 1fr;
  }
}
.heir-empty {
  padding: 18px 0;
  color: var(--muted);
  font-size: 13px;
}

.notification-outbox {
  margin-top: 16px;
}

.notification-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 12px;
}

.notification-count {
  color: var(--muted);
  font-size: 10px;
}

.notification-list {
  display: grid;
  gap: 8px;
}

.notification-row {
  display: grid;
  grid-template-columns: 92px minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid var(--line);
  border-radius: var(--radius-sm);
  background: var(--surface-soft);
}

.notification-copy {
  display: grid;
  gap: 3px;
  min-width: 0;
}

.notification-copy strong,
.notification-copy small,
.notification-copy p {
  overflow-wrap: anywhere;
}

.notification-copy small {
  color: var(--muted);
  font-size: 11px;
}

.notification-copy p {
  margin: 0;
  color: var(--danger, #b42318);
  font-size: 11px;
}

.notification-time {
  color: var(--muted);
  font-size: 10px;
  white-space: nowrap;
}

@media (max-width: 680px) {
  .notification-row {
    grid-template-columns: 1fr auto;
  }

  .notification-row .status-pill {
    grid-column: 1 / -1;
  }
}
</style>
