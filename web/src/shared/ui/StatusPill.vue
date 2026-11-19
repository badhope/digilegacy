<script setup lang="ts">
withDefaults(
  defineProps<{
    tone?: "ready" | "planned" | "degraded" | "warn" | "danger" | "neutral";
    label: string;
    dot?: boolean;
  }>(),
  { tone: "neutral", dot: true },
);
</script>

<template>
  <span class="status-pill" :class="`tone-${tone}`">
    <span v-if="dot" class="status-dot" />
    {{ label }}
  </span>
</template>

<style scoped>
.status-pill {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  width: fit-content;
  min-height: 24px;
  padding: 0 9px;
  border: 1px solid transparent;
  border-radius: 999px;
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.04em;
  white-space: nowrap;
}

.status-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: currentColor;
}

.tone-ready {
  border-color: color-mix(in srgb, var(--accent) 24%, transparent);
  background: var(--accent-soft);
  color: var(--accent-strong);
}

.tone-planned {
  border-color: var(--line);
  background: var(--surface-soft);
  color: var(--muted-strong);
}

.tone-degraded,
.tone-warn {
  border-color: color-mix(in srgb, var(--amber) 24%, transparent);
  background: var(--amber-soft);
  color: var(--amber);
}

.tone-danger {
  border-color: color-mix(in srgb, var(--danger) 24%, transparent);
  background: var(--danger-soft);
  color: var(--danger);
}

.tone-neutral {
  border-color: var(--line);
  background: transparent;
  color: var(--muted-strong);
}
</style>
