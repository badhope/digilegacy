<script setup lang="ts">
import AppIcon from "@/shared/icons/AppIcon.vue";
import type { IconName } from "@/shared/icons/paths";

withDefaults(
  defineProps<{
    icon?: IconName;
    title: string;
    note?: string;
  }>(),
  { icon: "assets", note: "" },
);
</script>

<template>
  <div class="empty-state">
    <span class="empty-icon"><AppIcon :name="icon" :size="24" /></span>
    <h3>{{ title }}</h3>
    <p v-if="note">{{ note }}</p>
    <div v-if="$slots.action" class="empty-action">
      <slot name="action" />
    </div>
  </div>
</template>

<style scoped>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
  padding: clamp(28px, 5vw, 48px);
  border: 1px dashed var(--line-strong);
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--surface) 70%, transparent);
}

.empty-icon {
  display: grid;
  width: 44px;
  height: 44px;
  margin-bottom: 8px;
  border: 1px solid var(--line);
  border-radius: 50%;
  color: var(--accent-strong);
  place-items: center;
  background: var(--accent-soft);
}

.empty-state h3 {
  font-size: 20px;
}

.empty-state p {
  max-width: 520px;
  margin: 0;
  color: var(--muted);
  font-size: 13px;
  line-height: 1.65;
}

.empty-action {
  margin-top: 10px;
}
</style>
