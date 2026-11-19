<script setup lang="ts">
defineProps<{
  label: string;
  value: string | number;
  unit?: string;
  note?: string;
}>();
</script>

<template>
  <article class="metric-card">
    <div class="metric-top">
      <span class="metric-label">{{ label }}</span>
      <slot name="icon" />
    </div>
    <div class="metric-value">
      <span>{{ value }}</span>
      <small v-if="unit">{{ unit }}</small>
    </div>
    <p v-if="note" class="metric-note">{{ note }}</p>
    <slot />
  </article>
</template>

<style scoped>
.metric-card {
  position: relative;
  min-height: 150px;
  padding: 20px 20px 18px;
  overflow: hidden;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.metric-card::after {
  position: absolute;
  right: -28px;
  bottom: -38px;
  width: 100px;
  height: 100px;
  border-radius: 50%;
  background: var(--accent-soft);
  content: "";
  opacity: 0.55;
}

.metric-top {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  color: var(--muted-strong);
}

.metric-label {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.metric-value {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: baseline;
  gap: 5px;
  margin-top: 22px;
}

.metric-value span {
  color: var(--ink);
  font-family: var(--font-display);
  font-size: clamp(32px, 3vw, 44px);
  line-height: 0.96;
}

.metric-value small {
  color: var(--muted);
  font-family: var(--font-mono);
  font-size: 11px;
}

.metric-note {
  position: relative;
  z-index: 1;
  margin: 12px 0 0;
  color: var(--muted);
  font-size: 11px;
}
</style>
