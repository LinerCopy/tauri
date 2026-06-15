import type { InspectResult } from '@/types/site';

export const mockResult: InspectResult = {
  requestId: 'rid-test',
  inputUrl: 'https://gosuslugi.ru/',
  resolvedHost: 'gosuslugi.ru',
  tlsVersion: 'TLS 1.3',
  tlsCipher: 'TLS_AES_128_GCM_SHA256',
  certificate: {
    subject: 'CN=gosuslugi.ru,O=Минцифры России,C=RU',
    issuer: 'CN=Russian Trusted Sub CA,O=Минцифры России,C=RU',
    serialNumber: '0A1B2C3D',
    validFrom: '2024-01-01T00:00:00Z',
    validTo:   '2026-01-01T00:00:00Z',
    san: ['DNS:gosuslugi.ru', 'DNS:www.gosuslugi.ru'],
    cn: 'gosuslugi.ru',
    fingerprintSha256:
      'AA11BB22CC33DD44EE55FF66001122334455667788990011223344556677AABB',
    signatureAlgorithm: 'sha256WithRSAEncryption',
  },
  chain: [
    {
      subject: 'CN=gosuslugi.ru',
      issuer: 'CN=Russian Trusted Sub CA',
      serialNumber: '0A',
      validFrom: '2024-01-01T00:00:00Z',
      validTo:   '2026-01-01T00:00:00Z',
      fingerprintSha256: 'AA11BB22',
    },
    {
      subject: 'CN=Russian Trusted Sub CA',
      issuer: 'CN=Russian Trusted Root CA',
      serialNumber: '0B',
      validFrom: '2022-01-01T00:00:00Z',
      validTo:   '2032-01-01T00:00:00Z',
      fingerprintSha256: 'CC33DD44',
    },
    {
      subject: 'CN=Russian Trusted Root CA',
      issuer: 'CN=Russian Trusted Root CA',
      serialNumber: '0C',
      validFrom: '2020-01-01T00:00:00Z',
      validTo:   '2040-01-01T00:00:00Z',
      fingerprintSha256: 'EE55FF66',
    },
  ],
  validation: {
    hostname_ok: true,
    chain_ok: true,
    expired_ok: true,
    mincifry_ca_ok: true,
  },
  is_mintsifry_ca: true,
  html: '<!doctype html><title>hi</title>',
  errors: [],
};
