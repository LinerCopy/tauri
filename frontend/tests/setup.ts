import { vi } from 'vitest';

// Глобальный mock унифицированного backend-инвокера.
// Конкретный тест может перенастроить возвращаемое значение через
// `(invoke as ReturnType<typeof vi.fn>).mockResolvedValue(...)`.
vi.mock('@/lib/invokeBackend', () => ({
  invoke: vi.fn(),
  isTauriRuntime: () => true,
  isDemoMode: false,
}));
