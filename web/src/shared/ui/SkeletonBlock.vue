<script setup lang="ts">
withDefaults(
  defineProps<{
    width?: string;
    height?: string;
    radius?: string;
  }>(),
  { width: "100%", height: "16px", radius: "8px" },
);
</script>

<template>
  <span class="skeleton" :style="{ width, height, borderRadius: radius }" aria-hidden="true" />
</template>

<style scoped>
.skeleton {
  display: block;
  overflow: hidden;
  background: var(--surface-soft);
}

.skeleton::after {
  display: block;
  width: 45%;
  height: 100%;
  background: linear-gradient(90deg, transparent, color-mix(in srgb, var(--surface) 70%, transparent), transparent);
  content: "";
  animation: shimmer 1.4s infinite var(--ease);
}

@keyframes shimmer {
  from {
    transform: translateX(-120%);
  }
  to {
    transform: translateX(260%);
  }
}
</style>
