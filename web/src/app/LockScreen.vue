<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import { unlockVault } from "@/platform/securityUnlock";
import { ui } from "@/platform/ui";
import { useI18n } from "@/platform/i18n";
import { useDialogFocus } from "./dialogFocus";

const { t } = useI18n();
const panel = ref<HTMLElement | null>(null);
const input = ref<HTMLInputElement | null>(null);
const password = ref("");
const busy = ref(false);
const error = ref("");
const restoreFocus = ref(true);
useDialogFocus(computed(() => ui.locked), panel, "first", restoreFocus);

async function unlock() {
  if (!password.value) {
    error.value = t("lock.passwordRequired");
    return;
  }
  busy.value = true;
  error.value = "";
  const envelope = await unlockVault(password.value);
  busy.value = false;
  if (!envelope.success) {
    error.value = envelope.code === "INVALID_ARGS" ? t("lock.invalid") : envelope.message || envelope.code;
    password.value = "";
    await nextTick();
    input.value?.focus();
    return;
  }
  password.value = "";
  restoreFocus.value = false;
  ui.locked = false;
  await nextTick();
  document.getElementById("main")?.focus({ preventScroll: true });
}
</script>

<template>
  <Teleport to="body">
    <div v-if="ui.locked" ref="panel" class="lock-layer" role="dialog" aria-modal="true" :aria-label="t('lock.title')" tabindex="-1">
      <div class="lock-panel">
        <div class="lock-mark"><AppIcon name="lock" :size="26" /></div>
        <span class="eyebrow">DIGILEGACY / LOCKED</span>
        <h1>{{ t("lock.title") }}</h1>
        <p>{{ t("lock.note") }}</p>
        <form class="lock-form" @submit.prevent="unlock">
          <label class="lock-field">
            <span>{{ t("lock.password") }}</span>
            <input
              ref="input"
              v-model="password"
              type="password"
              autocomplete="current-password"
              :disabled="busy"
              :placeholder="t('lock.passwordPlaceholder')"
            />
          </label>
          <AppButton variant="primary" type="submit" :disabled="busy">
            {{ busy ? t("lock.unlocking") : t("lock.action") }}
            <AppIcon name="arrow-right" :size="14" />
          </AppButton>
        </form>
        <p v-if="error" class="lock-error" role="status" aria-live="polite">{{ error }}</p>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.lock-layer {
  position: fixed;
  inset: 0;
  z-index: 80;
  display: grid;
  padding: 20px;
  background: color-mix(in srgb, var(--bg) 82%, rgba(18, 22, 17, 0.55));
  backdrop-filter: blur(12px);
  place-items: center;
}

.lock-panel {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  width: min(430px, 100%);
  padding: 34px;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-lg);
  background: var(--surface);
  box-shadow: var(--shadow-float);
}

.lock-mark {
  display: grid;
  width: 56px;
  height: 56px;
  margin-bottom: 22px;
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--accent-strong);
  place-items: center;
  background: var(--accent-soft);
}

.lock-panel h1 {
  margin-top: 14px;
  font-size: 34px;
  line-height: 1;
}

.lock-panel p {
  max-width: 330px;
  margin: 12px 0 20px;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.7;
}

.lock-form {
  display: flex;
  flex-direction: column;
  gap: 14px;
  width: 100%;
}

.lock-field {
  display: flex;
  flex-direction: column;
  gap: 7px;
  font-size: 12px;
  color: var(--muted-strong);
}

.lock-field input {
  width: 100%;
  min-height: 42px;
  padding: 0 13px;
  border: 1px solid var(--line-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-soft);
  color: var(--ink);
  font: inherit;
}

.lock-field input:focus-visible {
  border-color: var(--accent);
  outline: 2px solid var(--accent-soft);
}

.lock-error {
  margin: 14px 0 0 !important;
  color: var(--danger, #b3261e) !important;
}

.lock-form :deep(.app-button) {
  align-self: flex-start;
}
</style>
