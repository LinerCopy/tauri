import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import JsonExporter from '@/components/JsonExporter.vue';
import { mockResult } from './fixtures';

describe('JsonExporter', () => {
  it('renders pretty-printed JSON', () => {
    const wrapper = mount(JsonExporter, { props: { data: mockResult } });
    const pre = wrapper.find('pre').text();
    expect(pre).toContain('"requestId": "rid-test"');
    expect(pre).toContain('"is_mintsifry_ca": true');
  });

  it('has copy and download buttons', () => {
    const wrapper = mount(JsonExporter, { props: { data: mockResult } });
    const buttons = wrapper.findAll('button');
    expect(buttons.map((b) => b.text())).toEqual(['Копировать', 'Скачать']);
  });
});
