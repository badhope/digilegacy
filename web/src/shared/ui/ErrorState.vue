<script setup lang="ts">
import AppIcon from "@/shared/icons/AppIcon.vue";
import AppButton from "./AppButton.vue";

withDefaults(
  defineProps<{
    title: string;
    message?: string;
    retryLabel?: string;
  }>(),
  { message: "", retryLabel: "Retry" },
);

defineEmits<{ retry: [] }>();
</script>

<template>
  <div class="error-state" role="alert">
    <span class="error-icon"><AppIcon name="alert" :size="22" /></span>
    <div>
      <h3>{{ title }}</h3>
      <p v-if="message">{{ message }}</p>
    </div>
    <AppButton variant="secondary" size="sm" @click="$emit('retry')">{{ retryLabel }}</AppButton>
  </div>
</template>

<style scoped>
.error-state {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 18px 20px;
  border: 1px solid color-mix(in srgb, var(--danger) 22%, transparent);
  border-radius: var(--radius-md);
  background: var(--danger-soft);
}

.error-icon {
  display: grid;
  flex-shrink: 0;
  width: 36px;
  height: 36px;
  border-radius: 50%;
  color: var(--danger);
  place-items: center;
  background: color-mix(in srgb, var(--surface) 70%, transparent);
}

.error-state h3 {
  font-family: var(--font-body);
  font-size: 13px;
  font-weight: 700;
}

.error-state p {
  margin: 2px 0 0;
  color: var(--danger);
  font-size: 12px;
}

.error-state > :last-child {
  margin-left: auto;
}
</style>
