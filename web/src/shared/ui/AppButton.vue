<script setup lang="ts">
withDefaults(
  defineProps<{
    variant?: "primary" | "secondary" | "ghost" | "quiet" | "danger";
    size?: "sm" | "md";
    type?: "button" | "submit";
    disabled?: boolean;
  }>(),
  { variant: "secondary", size: "md", type: "button", disabled: false },
);
</script>

<template>
  <button
    :type="type"
    :disabled="disabled"
    class="app-button"
    :class="[`variant-${variant}`, `size-${size}`]"
  >
    <slot />
  </button>
</template>

<style scoped>
.app-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  border: 1px solid transparent;
  border-radius: 999px;
  font-weight: 600;
  letter-spacing: 0.01em;
  white-space: nowrap;
  transition:
    background var(--fast) var(--ease),
    border-color var(--fast) var(--ease),
    color var(--fast) var(--ease),
    transform var(--fast) var(--ease);
}

.app-button:active:not(:disabled) {
  transform: translateY(1px);
}

.app-button:disabled {
  cursor: not-allowed;
  opacity: 0.55;
}

.size-md {
  min-height: 38px;
  padding: 0 16px;
  font-size: 13px;
}

.size-sm {
  min-height: 30px;
  padding: 0 12px;
  font-size: 12px;
}

.variant-primary {
  background: var(--accent);
  color: var(--accent-ink);
}

.variant-primary:hover:not(:disabled) {
  background: var(--accent-strong);
}

.variant-secondary {
  border-color: var(--line-strong);
  background: var(--surface);
  color: var(--ink);
}

.variant-secondary:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent-strong);
}

.variant-ghost {
  color: var(--muted-strong);
}

.variant-ghost:hover:not(:disabled) {
  background: var(--surface-soft);
  color: var(--ink);
}

.variant-danger {
  border-color: color-mix(in srgb, var(--danger, #b42318) 40%, var(--line));
  background: color-mix(in srgb, var(--danger, #b42318) 8%, var(--surface));
  color: var(--danger, #b42318);
}

.variant-danger:hover:not(:disabled) {
  background: color-mix(in srgb, var(--danger, #b42318) 14%, var(--surface));
  border-color: var(--danger, #b42318);
}

.variant-quiet {
  color: var(--muted);
  font-weight: 500;
}

.variant-quiet:hover:not(:disabled) {
  color: var(--ink);
}
</style>
