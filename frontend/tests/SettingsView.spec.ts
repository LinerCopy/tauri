import { describe, it, expect, beforeEach, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import SettingsView from '@/views/SettingsView.vue';
import { invoke } from '@/lib/invokeBackend';

const mockManifest = {
  version: '2026.06.12',
  issuer: 'Минцифры России',
  description: 'mock description',
  source: 'https://www.gosuslugi.ru/crt',
  updatedAt: '2026-06-12T00:00:00Z',
  roots: [
    {
      file: 'roots/test-root.pem',
      subject: 'CN=Russian Trusted Root CA',
      fingerprintSha256:
        'D2:6D:2D:02:31:B7:C3:9F:92:CC:73:85:12:BA:54:10:35:19:E4:40:5D:68:B5:BD:70:3E:97:88:CA:8E:CF:31',
      notAfter: '2032-02-27T21:04:15Z',
    },
  ],
  intermediates: [
    {
      file: 'intermediates/test-sub.pem',
      subject: 'CN=Russian Trusted Sub CA',
      fingerprintSha256:
        'BB:BD:E2:10:3E:79:0B:99:9E:C6:2B:D0:3C:F6:25:A5:A2:E7:C3:16:E1:0A:FE:6A:49:0E:ED:EA:D8:B3:FD:9B',
      notAfter: '2027-03-06T11:25:19Z',
    },
  ],
  signature: null,
};

const routerPush = vi.fn();
vi.mock('vue-router', () => ({
  useRouter: () => ({ push: routerPush }),
}));

function setupInvoke() {
  const fn = invoke as unknown as ReturnType<typeof vi.fn>;
  fn.mockReset();
  fn.mockImplementation(async (cmd: string) => {
    if (cmd === 'trust_store_info') return mockManifest;
    if (cmd === 'core_version') return 'gci-core 1.0.0';
    throw new Error(`unexpected cmd ${cmd}`);
  });
  return fn;
}

describe('SettingsView', () => {
  beforeEach(() => {
    routerPush.mockReset();
    // Stub clipboard so copy buttons work in jsdom.
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText: vi.fn().mockResolvedValue(undefined) },
    });
    // window.open used by openSource()
    vi.spyOn(window, 'open').mockImplementation(() => null);
  });

  it('renders manifest version, roots and intermediates', async () => {
    setupInvoke();
    const wrapper = mount(SettingsView);
    await flushPromises();

    const html = wrapper.html();
    expect(html).toContain('2026.06.12');
    expect(html).toContain('gci-core 1.0.0');
    expect(html).toContain('Russian Trusted Root CA');
    expect(html).toContain('Russian Trusted Sub CA');
    expect(html).toContain('Обновление сертификатов Минцифры');
  });

  it('copies a single fingerprint to clipboard', async () => {
    setupInvoke();
    const wrapper = mount(SettingsView);
    await flushPromises();

    const fpBtn = wrapper.findAll('.fp-btn')[0];
    expect(fpBtn).toBeTruthy();
    await fpBtn.trigger('click');
    await flushPromises();

    const writeText = (navigator.clipboard as unknown as { writeText: ReturnType<typeof vi.fn> }).writeText;
    expect(writeText).toHaveBeenCalledWith(mockManifest.roots[0].fingerprintSha256);
  });

  it('copies all fingerprints with header lines', async () => {
    setupInvoke();
    const wrapper = mount(SettingsView);
    await flushPromises();

    const copyAll = wrapper
      .findAll('button')
      .find((b) => b.text().includes('Скопировать SHA-256'));
    expect(copyAll).toBeTruthy();
    await copyAll!.trigger('click');
    await flushPromises();

    const writeText = (navigator.clipboard as unknown as { writeText: ReturnType<typeof vi.fn> }).writeText;
    const payload: string = writeText.mock.calls[0][0];
    expect(payload).toContain('trust-store 2026.06.12');
    expect(payload).toContain('[ROOT]');
    expect(payload).toContain('[SUB]');
    expect(payload).toContain(mockManifest.roots[0].fingerprintSha256);
    expect(payload).toContain(mockManifest.intermediates[0].fingerprintSha256);
  });

  it('runs "Проверить обновление" and reloads the manifest', async () => {
    const fn = setupInvoke();
    const wrapper = mount(SettingsView);
    await flushPromises();

    // initial load = 2 invocations (trust_store_info + core_version)
    expect(fn).toHaveBeenCalledTimes(2);

    const updateBtn = wrapper
      .findAll('button')
      .find((b) => b.text().includes('Проверить обновление'));
    expect(updateBtn).toBeTruthy();
    await updateBtn!.trigger('click');
    await flushPromises();
    // wait the small setTimeout(350)
    await new Promise((r) => setTimeout(r, 400));
    await flushPromises();

    // reload = +2 more invocations
    expect(fn).toHaveBeenCalledTimes(4);
    expect(wrapper.html()).toContain('Последняя проверка');
  });

  it('opens source URL via window.open', async () => {
    setupInvoke();
    const wrapper = mount(SettingsView);
    await flushPromises();

    const openBtn = wrapper
      .findAll('button')
      .find((b) => b.text().includes('Открыть источник'));
    expect(openBtn).toBeTruthy();
    await openBtn!.trigger('click');
    expect(window.open).toHaveBeenCalledWith(
      mockManifest.source,
      '_blank',
      'noopener,noreferrer',
    );
  });

  it('navigates back to home', async () => {
    setupInvoke();
    const wrapper = mount(SettingsView);
    await flushPromises();

    await wrapper.find('.back-btn').trigger('click');
    expect(routerPush).toHaveBeenCalledWith({ name: 'home' });
  });

  it('shows error card if invoke fails', async () => {
    const fn = invoke as unknown as ReturnType<typeof vi.fn>;
    fn.mockReset();
    fn.mockRejectedValue(new Error('boom'));
    const wrapper = mount(SettingsView);
    await flushPromises();

    expect(wrapper.html()).toContain('Ошибка');
    expect(wrapper.html()).toContain('boom');
  });
});
