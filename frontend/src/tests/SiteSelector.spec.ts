import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import SiteSelector from '@/components/SiteSelector.vue';
import { KNOWN_SITES } from '@/types/site';

describe('SiteSelector', () => {
  it('renders all known sites', () => {
    const wrapper = mount(SiteSelector, {
      props: { modelValue: '', loadHtml: true },
    });
    const chips = wrapper.findAll('.chip');
    expect(chips).toHaveLength(KNOWN_SITES.length);
  });

  it('emits update:modelValue and submit on chip + button', async () => {
    const wrapper = mount(SiteSelector, {
      props: { modelValue: '', loadHtml: true },
    });
    await wrapper.findAll('.chip')[0].trigger('click');
    expect(wrapper.emitted('update:modelValue')?.[0]?.[0]).toBe(KNOWN_SITES[0].url);

    await wrapper.setProps({ modelValue: KNOWN_SITES[0].url });
    await wrapper.find('button.primary').trigger('click');
    expect(wrapper.emitted('submit')).toBeTruthy();
  });

  it('disables submit when URL invalid', async () => {
    const wrapper = mount(SiteSelector, {
      props: { modelValue: 'ftp://bad', loadHtml: true },
    });
    expect(wrapper.find('button.primary').attributes('disabled')).toBeDefined();
  });

  it('toggles loadHtml', async () => {
    const wrapper = mount(SiteSelector, {
      props: { modelValue: 'https://x.ru', loadHtml: true },
    });
    await wrapper.find('input[type="checkbox"]').setValue(false);
    expect(wrapper.emitted('update:loadHtml')?.[0]?.[0]).toBe(false);
  });
});
