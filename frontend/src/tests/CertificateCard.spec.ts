import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import CertificateCard from '@/components/CertificateCard.vue';
import { mockResult } from './fixtures';

describe('CertificateCard', () => {
  it('renders main fields and Минцифры badge', () => {
    const wrapper = mount(CertificateCard, {
      props: {
        cert: mockResult.certificate!,
        validation: mockResult.validation,
        tlsVersion: mockResult.tlsVersion,
        tlsCipher: mockResult.tlsCipher,
      },
    });

    const text = wrapper.text();
    expect(text).toContain('gosuslugi.ru');
    expect(text).toContain('TLS 1.3');
    expect(text).toContain('AA11BB22CC33DD44');
    expect(text).toContain('УЦ Минцифры: да');

    const okItems = wrapper.findAll('.checks li.ok');
    expect(okItems).toHaveLength(4);
  });

  it('marks failed checks as .bad', () => {
    const wrapper = mount(CertificateCard, {
      props: {
        cert: mockResult.certificate!,
        validation: {
          hostname_ok: false,
          chain_ok: false,
          expired_ok: false,
          mincifry_ca_ok: false,
        },
        tlsVersion: 'TLS 1.2',
      },
    });
    expect(wrapper.findAll('.checks li.bad')).toHaveLength(4);
    expect(wrapper.text()).toContain('УЦ Минцифры: нет');
  });
});
