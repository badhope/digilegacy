<script setup lang="ts">
import { useRouter } from "vue-router";
import AppButton from "@/shared/ui/AppButton.vue";
import AppIcon from "@/shared/icons/AppIcon.vue";
import EmptyState from "@/shared/ui/EmptyState.vue";
import PageHeader from "@/shared/ui/PageHeader.vue";
import StatusPill from "@/shared/ui/StatusPill.vue";
import { useI18n } from "@/platform/i18n";

const router = useRouter();
const { t } = useI18n();
</script>

<template>
  <div class="not-found">
    <PageHeader eyebrow="ROUTE / 404" :title="t('notfound.title')" :description="t('notfound.note')">
      <template #actions>
        <StatusPill tone="danger" :label="t('app.mode')" />
      </template>
    </PageHeader>
    <EmptyState :title="t('notfound.title')" :note="t('notfound.note')" icon="alert">
      <template #action>
        <AppButton variant="primary" @click="router.push('/overview')">
          {{ t("notfound.action") }}
          <AppIcon name="arrow-right" :size="14" />
        </AppButton>
      </template>
    </EmptyState>
  </div>
</template>

<style scoped>
.not-found {
  max-width: 900px;
  margin: 0 auto;
}
</style>
