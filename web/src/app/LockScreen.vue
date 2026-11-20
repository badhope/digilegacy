<script setup lang="ts">
import { computed, ref } from "vue";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import { ui, unlockConsole } from "@/platform/ui";
import { useI18n } from "@/platform/i18n";
import { useDialogFocus } from "./dialogFocus";

const { t } = useI18n();
const panel = ref<HTMLElement | null>(null);
useDialogFocus(computed(() => ui.locked), panel, "first");
</script>

<template>
  <Teleport to="body">
    <div v-if="ui.locked" ref="panel" class="lock-layer" role="dialog" aria-modal="true" :aria-label="t('lock.title')" tabindex="-1">
      <div class="lock-panel">
        <div class="lock-mark"><AppIcon name="lock" :size="26" /></div>
        <span class="eyebrow">DIGILEGACY / LOCKED</span>
        <h1>{{ t("lock.title") }}</h1>
        <p>{{ t("lock.note") }}</p>
        <AppButton variant="primary" @click="unlockConsole()">
          {{ t("lock.action") }}
          <AppIcon name="arrow-right" :size="14" />
        </AppButton>
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
</style>
