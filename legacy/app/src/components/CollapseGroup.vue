<template>
  <div class="collapse-group" :class="{ open: isOpen }">
    <button class="collapse-head" type="button" :aria-expanded="isOpen" @click="toggle">
      <span class="collapse-chevron" aria-hidden="true">▸</span>
      <span class="collapse-title">{{ title }}</span>
      <span v-if="count != null" class="collapse-count">{{ count }}</span>
    </button>
    <div v-if="isOpen" class="collapse-body">
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';

const props = defineProps({
  title: { type: String, required: true },
  count: { type: Number, default: undefined },
  /**
   * 受控值（父组件 v-model）。必须显式 default: undefined，
   * 否则 Vue 的布尔转换会把「未传入」强制为 false，导致 defaultOpen 永远不生效。
   */
  modelValue: { type: Boolean, default: undefined },
  defaultOpen: { type: Boolean, default: false },
});

const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>();

const isOpen = ref(props.modelValue ?? props.defaultOpen);

watch(
  () => props.modelValue,
  (v) => {
    if (v !== undefined) isOpen.value = v;
  }
);

function toggle() {
  isOpen.value = !isOpen.value;
  emit('update:modelValue', isOpen.value);
}
</script>
