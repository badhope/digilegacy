<script setup lang="ts">
import { paths, type IconName } from "./paths";

withDefaults(
  defineProps<{
    name: IconName;
    size?: number;
    stroke?: number;
  }>(),
  { size: 18, stroke: 1.6 },
);
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    :stroke-width="stroke"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path v-for="(path, index) in paths[name]" :key="index" :d="path" />
  </svg>
</template>
