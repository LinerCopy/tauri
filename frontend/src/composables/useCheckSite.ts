import { ref, shallowRef } from "vue";
import { invoke } from "@/lib/invokeBackend";
import type { InspectResult } from "@/types/site";

export interface CheckSiteOptions {
  loadHtml?: boolean;
}

export function useCheckSite() {
  const loading = ref(false);
  const error = ref<string | null>(null);
  const result = shallowRef<InspectResult | null>(null);

  async function checkSite(
    url: string,
    options: CheckSiteOptions = {},
  ): Promise<InspectResult> {
    loading.value = true;
    error.value = null;
    result.value = null;
    try {
      const payload = { url, loadHtml: options.loadHtml ?? true };

      const res = await invoke<InspectResult>("check_site", payload);
      result.value = res;
      return res;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      error.value = msg;
      throw e;
    } finally {
      loading.value = false;
    }
  }

  function reset() {
    loading.value = false;
    error.value = null;
    result.value = null;
  }

  return { loading, error, result, checkSite, reset };
}
