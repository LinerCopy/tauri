import { describe, it, expect, vi, beforeEach } from 'vitest';

// Эти тесты обходят глобальный mock `@/lib/invokeBackend` и берут реальный
// mockBackend, чтобы проверить, что демо-режим корректно отвечает на
// trust_store_info и save_report.
vi.unmock('@/lib/invokeBackend');

describe('mockBackend', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it('returns embedded trust store manifest for trust_store_info', async () => {
    const { mockInvoke } = await import('@/mocks/mockBackend');
    const m = await mockInvoke<{
      version: string;
      roots: { fingerprintSha256: string }[];
      intermediates: { fingerprintSha256: string }[];
    }>('trust_store_info');
    expect(m.version).toMatch(/^\d{4}\.\d{2}\.\d{2}$/);
    expect(m.roots.length).toBeGreaterThan(0);
    expect(m.intermediates.length).toBeGreaterThan(0);
    expect(m.roots[0].fingerprintSha256).toMatch(/^[0-9A-F:]+$/);
  });

  it('returns a demo path for save_report', async () => {
    const { mockInvoke } = await import('@/mocks/mockBackend');
    const path = await mockInvoke<string>('save_report', { filename: 'r.json', content: '{}' });
    expect(path).toBe('/demo/Downloads/r.json');
  });

  it('returns a stable core_version string', async () => {
    const { mockInvoke } = await import('@/mocks/mockBackend');
    const v = await mockInvoke<string>('core_version');
    expect(v).toMatch(/mock/);
  });

  it('throws for unknown command', async () => {
    const { mockInvoke } = await import('@/mocks/mockBackend');
    await expect(mockInvoke('not-a-command')).rejects.toThrow(/unknown command/);
  });
});
