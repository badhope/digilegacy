<script setup lang="ts">
withDefaults(
  defineProps<{
    title: string;
    subtitle?: string;
    flush?: boolean;
  }>(),
  { subtitle: "", flush: false },
);
</script>

<template>
  <section class="section-card">
    <header class="section-head">
      <div>
        <h2>{{ title }}</h2>
        <p v-if="subtitle">{{ subtitle }}</p>
      </div>
      <div v-if="$slots.action" class="section-action">
        <slot name="action" />
      </div>
    </header>
    <div class="section-body" :class="{ flush }">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.section-card {
  min-width: 0;
  border: 1px solid var(--line);
  border-radius: var(--radius-md);
  background: var(--surface);
  box-shadow: var(--shadow-sm);
}

.section-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 18px;
  padding: 20px 22px 15px;
}

.section-head h2 {
  font-family: var(--font-display);
  font-size: 20px;
}

.section-head p {
  margin: 5px 0 0;
  color: var(--muted);
  font-size: 12px;
}

.section-action {
  flex-shrink: 0;
}

.section-body {
  padding: 0 22px 22px;
}

.section-body.flush {
  padding: 0;
}
</style>
