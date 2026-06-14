import { describe, it, expect } from 'vitest';
import type { InspectResult } from '@/types/site';
import { mockResult } from './fixtures';

/**
 * Тесты DTO-маппинга: проверяют, что контракт от ядра успешно парсится
 * в типы фронтенда без потерь и что snake_case флаги валидации сохраняются.
 */
describe('DTO mapping', () => {
  it('round-trips through JSON without losing fields', () => {
    const json = JSON.stringify(mockResult);
    const back = JSON.parse(json) as InspectResult;
    expect(back).toEqual(mockResult);
  });

  it('keeps snake_case for validation flags', () => {
    const json = JSON.stringify(mockResult);
    expect(json).toContain('"hostname_ok":true');
    expect(json).toContain('"chain_ok":true');
    expect(json).toContain('"expired_ok":true');
    expect(json).toContain('"mincifry_ca_ok":true');
    expect(json).toContain('"is_mintsifry_ca":true');
  });

  it('camelCase for the rest', () => {
    const json = JSON.stringify(mockResult);
    expect(json).toContain('"requestId":');
    expect(json).toContain('"resolvedHost":');
    expect(json).toContain('"tlsVersion":');
    expect(json).toContain('"fingerprintSha256":');
    expect(json).toContain('"signatureAlgorithm":');
  });

  it('handles null certificate gracefully', () => {
    const partial: InspectResult = {
      ...mockResult,
      certificate: null,
      chain: [],
      validation: {
        hostname_ok: false,
        chain_ok: false,
        expired_ok: false,
        mincifry_ca_ok: false,
      },
      is_mintsifry_ca: false,
      html: '',
      errors: [{ code: 'TLS_HANDSHAKE', message: 'EOF' }],
    };
    const back = JSON.parse(JSON.stringify(partial)) as InspectResult;
    expect(back.certificate).toBeNull();
    expect(back.errors[0].code).toBe('TLS_HANDSHAKE');
  });
});
