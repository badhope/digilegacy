<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import { unlockVault } from "@/platform/securityUnlock";
import { recoverVault } from "@/platform/securityRecover";
import { ui } from "@/platform/ui";
import { useI18n } from "@/platform/i18n";
import { useDialogFocus } from "./dialogFocus";

const { t } = useI18n();
const panel = ref<HTMLElement | null>(null);
const input = ref<HTMLInputElement | null>(null);
const mode = ref<"password" | "recovery">("password");
const password = ref("");
const recovery = ref("");
const busy = ref(false);
const error = ref("");
const restoreFocus = ref(true);
useDialogFocus(computed(() => ui.locked), panel, "first", restoreFocus);

function switchMode() {
  mode.value = mode.value === "password" ? "recovery" : "password";
  error.value = "";
  void nextTick(() => input.value?.focus());
}

async function submit() {
  const value = mode.value === "password" ? password.value : recovery.value;
  if (!value.trim()) {
    error.value = t(mode.value === "password" ? "lock.passwordRequired" : "lock.recoveryRequired");
    return;
  }
  busy.value = true;
  error.value = "";
  const envelope =
    mode.value === "password" ? await unlockVault(value) : await recoverVault(value);
  busy.value = false;
  if (!envelope.success) {
    if (envelope.code === "RATE_LIMITED") {
      error.value = t("lock.rateLimited");
    } else if (envelope.code === "INVALID_ARGS") {
      error.value = t(mode.value === "password" ? "lock.invalid" : "lock.invalidRecovery");
    } else {
      error.value = envelope.message || envelope.code;
    }
    if (mode.value === "password") password.value = "";
    else recovery.value = "";
    await nextTick();
    input.value?.focus();
    return;
  }
  password.value = "";
  recovery.value = "";
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
        <p>{{ t(mode === "password" ? "lock.note" : "lock.recoveryNote") }}</p>
        <form class="lock-form" @submit.prevent="submit">
          <label class="lock-field">
            <span>{{ t(mode === "password" ? "lock.password" : "lock.recovery") }}</span>
            <input
              v-if="mode === 'password'"
              ref="input"
              v-model="password"
              type="password"
              autocomplete="current-password"
              :disabled="busy"
              :placeholder="t('lock.passwordPlaceholder')"
            />
            <input
              v-else
              ref="input"
              v-model="recovery"
              type="text"
              autocomplete="one-time-code"
              :disabled="busy"
              :placeholder="t('lock.recoveryPlaceholder')"
            />
          </label>
          <AppButton variant="primary" type="submit" :disabled="busy">
            {{ busy ? t(mode === "password" ? "lock.unlocking" : "lock.recovering") : t("lock.action") }}
            <AppIcon name="arrow-right" :size="14" />
          </AppButton>
        </form>
        <button class="lock-switch" type="button" :disabled="busy" @click="switchMode">
          {{ t(mode === "password" ? "lock.useRecovery" : "lock.usePassword") }}
        </button>
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

.lock-switch {
  margin-top: 14px;
  padding: 0;
  border: 0;
  background: none;
  color: var(--accent-strong);
  font: inherit;
  font-size: 12px;
  text-decoration: underline;
  cursor: pointer;
}

.lock-switch:disabled {
  opacity: 0.55;
  cursor: default;
}
</style>
