import { onMounted, ref, shallowRef } from "vue";
import type { Envelope } from "./types";

export function useResource<T>(loader: () => Promise<Envelope<T>>) {
  const data = shallowRef<T | null>(null);
  const loading = ref(true);
  const error = ref<string | null>(null);
  let sequence = 0;

  async function reload() {
    const current = ++sequence;
    loading.value = true;
    error.value = null;
    try {
      const envelope = await loader();
      if (current !== sequence) return;
      if (!envelope.success || envelope.data === null) {
        error.value = envelope.message || envelope.code;
        return;
      }
      data.value = envelope.data;
    } catch (cause) {
      if (current !== sequence) return;
      error.value = cause instanceof Error ? cause.message : "request failed";
    } finally {
      if (current === sequence) {
        loading.value = false;
      }
    }
  }

  onMounted(reload);

  return { data, loading, error, reload };
}
