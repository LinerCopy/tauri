import { describe, it, expect, beforeEach, vi } from 'vitest';
import { ref } from 'vue';
import { mount } from '@vue/test-utils';
import HomeView from '@/views/HomeView.vue';

const routerPush = vi.fn();
vi.mock('vue-router', () => ({
  useRouter: () => ({ push: routerPush }),
}));

vi.mock('@/composables/useCheckSite', () => ({
  useCheckSite: () => ({
    loading: ref(false),
    error: ref(''),
    checkSite: vi.fn(),
  }),
}));

describe('HomeView', () => {
  beforeEach(() => {
    routerPush.mockReset();
  });

  it('renders title and settings button', () => {
    const wrapper = mount(HomeView);
    expect(wrapper.text()).toContain('GosCertInspector');
    const btn = wrapper.find('.settings-btn');
    expect(btn.exists()).toBe(true);
    expect(btn.attributes('aria-label')).toBe('Настройки');
    expect(btn.attributes('disabled')).toBeUndefined();
  });

  it('navigates to /settings on button click', async () => {
    const wrapper = mount(HomeView);
    await wrapper.find('.settings-btn').trigger('click');
    expect(routerPush).toHaveBeenCalledWith({ name: 'settings' });
  });
});
