<template>
  <div class="not-found fade-in">
    <div class="card text-center" style="padding: 56px 32px;">
      <EmptyState kind="search" :size="180" />
      <h2 style="font-size: 22px; margin: 20px 0 8px;">{{ t('notFound.title') }}</h2>
      <p class="text-muted mb-24" style="font-size: 14px;">
        {{ t('notFound.desc') }}
      </p>
      <div class="flex gap-12" style="justify-content: center; flex-wrap: wrap;">
        <router-link to="/dashboard" class="btn btn-primary">{{ t('notFound.backDashboard') }}</router-link>
        <button class="btn btn-secondary" @click="goBack">{{ t('notFound.backPrev') }}</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useRouter } from 'vue-router';
import { useI18n } from '../i18n';
import EmptyState from '../components/EmptyState.vue';

const { t } = useI18n();
const router = useRouter();

function goBack() {
  if (window.history.length > 1) router.back();
  else router.push('/dashboard');
}
</script>
