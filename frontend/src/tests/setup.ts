import { vi } from 'vitest';

vi.mock('@/lib/invokeBackend', () => ({
  invoke: vi.fn(),
  isTauriRuntime: () => true,
  isDemoMode: false,
}));
