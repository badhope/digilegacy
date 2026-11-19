import { defineStore } from 'pinia';
import { ref } from 'vue';
import { invoke } from '../api/invoke';

import type { Platform, PlatformExtended, ApiResponse } from '../types';

export const usePlatformsStore = defineStore('platforms', () => {
  const platforms = ref<Platform[]>([]);
  const platformsExtended = ref<PlatformExtended[]>([]);
  const loading = ref(false);
  const selectedIds = ref<Set<number>>(new Set());
  const loaded = ref(false);

  /** @param force 跳过「已加载」缓存；导入备份/政策补丁后必须强制重取 */
  async function loadPlatforms(force = false) {
    if (loaded.value && !force) return;
    loading.value = true;
    try {
      const res = await invoke<ApiResponse<Platform[]>>('get_all_platforms');
      if (res.success && res.data) {
        platforms.value = res.data;
        loaded.value = true;
      }
    } catch (e) {
      console.error('加载平台数据失败:', e);
    } finally {
      loading.value = false;
    }
  }

  async function loadPlatformsExtended(force = false) {
    // 扩展列表同样带缓存：导入政策补丁后旧列表会让「继承人向导」继续显示
    // 补丁前的政策，界面与现实不一致。凡是数据变更入口都必须 force。
    if (platformsExtended.value.length > 0 && !force) return;
    try {
      const res = await invoke<ApiResponse<PlatformExtended[]>>('get_all_platforms_extended');
      if (res.success && res.data) {
        platformsExtended.value = res.data;
      }
    } catch (e) {
      console.error('加载扩展平台数据失败:', e);
    }
  }

  /** 政策库/备份变更后统一失效：一次调用清掉两份缓存并重取。 */
  async function refreshPlatforms() {
    loaded.value = false;
    platformsExtended.value = [];
    await loadPlatforms(true);
  }

  function getExtendedById(id: number): PlatformExtended | undefined {
    return platformsExtended.value.find(p => p.id === id);
  }

  function toggle(id: number) {
    if (selectedIds.value.has(id)) {
      selectedIds.value.delete(id);
    } else {
      selectedIds.value.add(id);
    }
  }

  function isSelected(id: number): boolean {
    return selectedIds.value.has(id);
  }

  function getById(id: number): Platform | undefined {
    return platforms.value.find(p => p.id === id);
  }

  function clearSelection() {
    selectedIds.value.clear();
  }

  return { platforms, platformsExtended, loading, selectedIds, loaded, loadPlatforms, loadPlatformsExtended, refreshPlatforms, toggle, isSelected, getById, getExtendedById, clearSelection };
});
