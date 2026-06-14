import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { mockInvoke } from '@/mocks/mockBackend';

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined'
    && typeof (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== 'undefined';
}

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauriRuntime()) {
    return tauriInvoke<T>(cmd, args);
  }
  if (typeof console !== 'undefined') {
    console.warn(`[DEMO MODE] invoke('${cmd}') → mockBackend`, args);
  }
  return mockInvoke<T>(cmd, args);
}

export const isDemoMode = !isTauriRuntime();
