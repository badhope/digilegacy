<script setup lang="ts">
defineProps<{
  eyebrow: string;
  title: string;
  description?: string;
}>();</script>

<template>
  <header class="page-header">
    <div class="page-copy">
      <span class="eyebrow">{{ eyebrow }}</span>
      <h1>{{ title }}</h1>
      <p v-if="description">{{ description }}</p>
    </div>
    <div v-if="$slots.actions" class="page-actions">
      <slot name="actions" />
    </div>
  </header>
</template>

<style scoped>
.page-header {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: clamp(24px, 5vw, 72px);
  margin-bottom: 26px;
}

.page-copy {
  max-width: 720px;
}

.page-copy h1 {
  margin-top: 14px;
  font-size: clamp(34px, 4.4vw, 54px);
  line-height: 1.02;
}

.page-copy p {
  max-width: 620px;
  margin: 14px 0 0;
  color: var(--muted-strong);
  font-size: 14px;
  line-height: 1.65;
}

.page-actions {
  display: flex;
  flex-shrink: 0;
  flex-wrap: wrap;
  gap: 10px;
}

@media (max-width: 700px) {
  .page-header {
    align-items: flex-start;
    flex-direction: column;
    gap: 18px;
  }

  .page-copy h1 {
    font-size: clamp(28px, 8vw, 36px);
    line-height: 1.12;
  }

  .page-actions {
    width: 100%;
  }
}
</style>
