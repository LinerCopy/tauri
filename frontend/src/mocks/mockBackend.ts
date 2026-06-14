import type { InspectResult } from '@/types/site';

export async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (cmd === 'core_version') {
    return 'mock-browser-1.0.0' as unknown as T;
  }
  if (cmd === 'trust_store_info') {
    return mockTrustStoreManifest() as unknown as T;
  }
  if (cmd === 'check_trust_store_updates') {
    return mockUpdateCheck() as unknown as T;
  }
  if (cmd === 'save_report') {
    const filename = String((args ?? {}).filename ?? 'report.json');
    return (`/demo/Downloads/${filename}`) as unknown as T;
  }
  if (cmd !== 'check_site') {
    throw new Error(`mock: unknown command ${cmd}`);
  }
  const url = String((args ?? {}).url ?? '');
  const loadHtml = Boolean((args ?? {}).loadHtml ?? true);

  // Небольшая задержка, чтобы был виден loading-state
  await new Promise((r) => setTimeout(r, 450));

  const host = safeHost(url);
  const profile = profileFor(host);

  const result: InspectResult = {
    requestId: cryptoRid(),
    inputUrl: url,
    resolvedHost: host,
    tlsVersion: profile.tlsVersion,
    tlsCipher: profile.tlsCipher,
    isGostCipher: profile.tlsCipher.includes('GOST') || profile.tlsCipher.includes('KUZNYECHIK'),
    gostSupported: false,
    certificate: {
      subject: `CN=${host},O=${profile.org},C=RU`,
      issuer: profile.issuer,
      serialNumber: profile.serial,
      validFrom: profile.validFrom,
      validTo: profile.validTo,
      san: profile.san.length ? profile.san : [`DNS:${host}`, `DNS:www.${host}`],
      cn: host,
      fingerprintSha256: pseudoSha256(host),
      signatureAlgorithm: profile.sigAlg,
    },
    chain: [
      {
        subject: `CN=${host}`,
        issuer: profile.issuer,
        serialNumber: profile.serial,
        validFrom: profile.validFrom,
        validTo: profile.validTo,
        fingerprintSha256: pseudoSha256(host).slice(0, 16),
      },
      {
        subject: profile.issuer,
        issuer: profile.rootSubject,
        serialNumber: '0B',
        validFrom: '2022-01-01T00:00:00Z',
        validTo: '2032-01-01T00:00:00Z',
        fingerprintSha256: pseudoSha256(profile.issuer).slice(0, 16),
      },
      {
        subject: profile.rootSubject,
        issuer: profile.rootSubject,
        serialNumber: '0C',
        validFrom: '2020-01-01T00:00:00Z',
        validTo: '2040-01-01T00:00:00Z',
        fingerprintSha256: pseudoSha256(profile.rootSubject).slice(0, 16),
      },
    ],
    validation: {
      hostname_ok: true,
      chain_ok: profile.isMincifry,
      expired_ok: true,
      mincifry_ca_ok: profile.isMincifry,
    },
    is_mintsifry_ca: profile.isMincifry,
    html: loadHtml ? mockHtml(host, profile.title) : '',
    errors: profile.isMincifry
      ? []
      : [{ code: 'CHAIN_INVALID', message: 'Issuer is not in local Минцифры trust store' }],
  };
  return result as unknown as T;
}

function safeHost(url: string): string {
  try {
    return new URL(url).host || 'unknown';
  } catch {
    return 'unknown';
  }
}

interface Profile {
  org: string;
  issuer: string;
  rootSubject: string;
  serial: string;
  validFrom: string;
  validTo: string;
  san: string[];
  tlsVersion: string;
  tlsCipher: string;
  sigAlg: string;
  title: string;
  isMincifry: boolean;
}

const MINCIFRY_ISSUER = 'CN=Russian Trusted Sub CA,O=The Ministry of Digital Development and Communications,C=RU';
const MINCIFRY_ROOT   = 'CN=Russian Trusted Root CA,O=The Ministry of Digital Development and Communications,C=RU';
const FOREIGN_ISSUER  = 'CN=R3,O=Let\'s Encrypt,C=US';
const FOREIGN_ROOT    = 'CN=ISRG Root X1,O=Internet Security Research Group,C=US';

function profileFor(host: string): Profile {
  const base = {
    serial: '0A1B2C3D4E5F',
    validFrom: '2025-01-15T00:00:00Z',
    validTo:   '2026-04-15T23:59:59Z',
    san: [] as string[],
    tlsVersion: 'TLS 1.3',
    tlsCipher: 'TLS_AES_128_GCM_SHA256',
    sigAlg: 'sha256WithRSAEncryption',
  };
  if (host.endsWith('gosuslugi.ru')) {
    return { ...base, org: 'Минцифры России', issuer: MINCIFRY_ISSUER,
      rootSubject: MINCIFRY_ROOT, san: ['DNS:gosuslugi.ru', 'DNS:www.gosuslugi.ru', 'DNS:esia.gosuslugi.ru'],
      title: 'Госуслуги', isMincifry: true };
  }
  if (host.endsWith('nalog.gov.ru')) {
    return { ...base, org: 'ФНС России', issuer: MINCIFRY_ISSUER,
      rootSubject: MINCIFRY_ROOT, title: 'ФНС России', isMincifry: true };
  }
  if (host.endsWith('zakupki.gov.ru')) {
    return { ...base, org: 'Казначейство России', issuer: MINCIFRY_ISSUER,
      rootSubject: MINCIFRY_ROOT, title: 'Закупки', isMincifry: true };
  }
  if (host.endsWith('gost.ru')) {
    return { ...base, org: 'Росстандарт', issuer: MINCIFRY_ISSUER,
      rootSubject: MINCIFRY_ROOT, title: 'Росстандарт', isMincifry: true };
  }

  return { ...base, org: 'Example Org', issuer: FOREIGN_ISSUER,
    rootSubject: FOREIGN_ROOT, title: host, isMincifry: false };
}

function mockHtml(host: string, title: string): string {
  return `<!doctype html>
<html lang="ru"><head><meta charset="utf-8"><title>${escapeHtml(title)}</title></head>
<body style="font-family:sans-serif;padding:24px">
<h1>${escapeHtml(title)}</h1>
<p>Mock HTML для <code>${escapeHtml(host)}</code> — DEMO MODE.</p>
<p>В реальной мобильной сборке здесь будет настоящий ответ сервера.</p>
</body></html>`;
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) =>
    ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]!));
}

function cryptoRid(): string {
  const a = Math.random().toString(16).slice(2).padEnd(12, '0');
  const b = Math.random().toString(16).slice(2).padEnd(12, '0');
  return (a + b).slice(0, 32);
}

function pseudoSha256(seed: string): string {
  let h = 2166136261;
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i);
    h = (h * 16777619) >>> 0;
  }
  let out = '';
  for (let i = 0; i < 8; i++) {
    out += ('00000000' + ((h ^ (i * 2654435761)) >>> 0).toString(16)).slice(-8);
  }
  return out.toUpperCase();
}

function mockTrustStoreManifest() {  return {
    version: '2026.06.12',
    issuer: 'Минцифры России',
    description:
      'Локальный trust store с корневыми и промежуточными сертификатами УЦ Минцифры России для проверки государственных сайтов РФ.',
    source: 'https://www.gosuslugi.ru/crt',
    updatedAt: '2026-06-12T00:00:00Z',
    roots: [
      {
        file: 'roots/russian-trusted-root-ca.pem',
        subject:
          'C=RU, O=The Ministry of Digital Development and Communications, CN=Russian Trusted Root CA',
        fingerprintSha256:
          'D2:6D:2D:02:31:B7:C3:9F:92:CC:73:85:12:BA:54:10:35:19:E4:40:5D:68:B5:BD:70:3E:97:88:CA:8E:CF:31',
        notAfter: '2032-02-27T21:04:15Z',
      },
    ],
    intermediates: [
      {
        file: 'intermediates/russian-trusted-sub-ca.pem',
        subject:
          'C=RU, O=The Ministry of Digital Development and Communications, CN=Russian Trusted Sub CA',
        fingerprintSha256:
          'BB:BD:E2:10:3E:79:0B:99:9E:C6:2B:D0:3C:F6:25:A5:A2:E7:C3:16:E1:0A:FE:6A:49:0E:ED:EA:D8:B3:FD:9B',
        notAfter: '2027-03-06T11:25:19Z',
      },
    ],
    signature: null,
  };
}

function mockUpdateCheck() {
  const bundledRoot =
    'D2:6D:2D:02:31:B7:C3:9F:92:CC:73:85:12:BA:54:10:35:19:E4:40:5D:68:B5:BD:70:3E:97:88:CA:8E:CF:31';
  const bundledSub =
    'BB:BD:E2:10:3E:79:0B:99:9E:C6:2B:D0:3C:F6:25:A5:A2:E7:C3:16:E1:0A:FE:6A:49:0E:ED:EA:D8:B3:FD:9B';
  return {
    checkedAt: new Date().toISOString(),
    bundledVersion: '2026.06.12',
    entries: [
      {
        name: 'root',
        url: 'https://gu-st.ru/content/lending/russian_trusted_root_ca_pem.crt',
        bundledFingerprint: bundledRoot,
        remoteFingerprint: bundledRoot,
        matchesBundled: true,
        updated: false,
        error: null,
      },
      {
        name: 'sub',
        url: 'https://gu-st.ru/content/lending/russian_trusted_sub_ca_pem.crt',
        bundledFingerprint: bundledSub,
        remoteFingerprint: bundledSub,
        matchesBundled: true,
        updated: false,
        error: null,
      },
    ],
    upToDate: true,
    certsUpdated: 0,
  };
}
