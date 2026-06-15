import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@/lib/invokeBackend';
import { useCheckSite } from '@/composables/useCheckSite';
import { mockResult } from './fixtures';

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

describe('useCheckSite', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('passes url and loadHtml to Tauri invoke', async () => {
    invokeMock.mockResolvedValueOnce(mockResult);
    const { checkSite, result, loading, error } = useCheckSite();

    const promise = checkSite('https://gosuslugi.ru', { loadHtml: true });
    expect(loading.value).toBe(true);
    const res = await promise;

    expect(invokeMock).toHaveBeenCalledWith('check_site', {
      url: 'https://gosuslugi.ru',
      loadHtml: true,
    });
    expect(res.is_mintsifry_ca).toBe(true);
    expect(result.value?.resolvedHost).toBe('gosuslugi.ru');
    expect(loading.value).toBe(false);
    expect(error.value).toBeNull();
  });

  it('defaults loadHtml to true when not provided', async () => {
    invokeMock.mockResolvedValueOnce(mockResult);
    const { checkSite } = useCheckSite();
    await checkSite('https://nalog.gov.ru');
    expect(invokeMock).toHaveBeenCalledWith('check_site', {
      url: 'https://nalog.gov.ru',
      loadHtml: true,
    });
  });

  it('captures errors into reactive state and rethrows', async () => {
    invokeMock.mockRejectedValueOnce(new Error('boom'));
    const { checkSite, error, loading } = useCheckSite();

    await expect(checkSite('https://x.ru')).rejects.toThrow('boom');
    expect(error.value).toBe('boom');
    expect(loading.value).toBe(false);
  });

  it('reset() clears state', async () => {
    invokeMock.mockResolvedValueOnce(mockResult);
    const { checkSite, result, reset } = useCheckSite();
    await checkSite('https://gosuslugi.ru');
    expect(result.value).not.toBeNull();
    reset();
    expect(result.value).toBeNull();
  });
});
