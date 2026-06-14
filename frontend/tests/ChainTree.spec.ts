import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import ChainTree from '@/components/ChainTree.vue';
import { mockResult } from './fixtures';

describe('ChainTree', () => {
  it('renders end-entity, intermediates and root labels', () => {
    const wrapper = mount(ChainTree, { props: { chain: mockResult.chain } });
    const items = wrapper.findAll('li');
    expect(items).toHaveLength(3);
    expect(items[0].text()).toContain('End-entity');
    expect(items[1].text()).toContain('Intermediate');
    expect(items[2].text()).toContain('Root');
  });

  it('shows empty state when chain is empty', () => {
    const wrapper = mount(ChainTree, { props: { chain: [] } });
    expect(wrapper.text()).toContain('Цепочка пуста');
  });
});
