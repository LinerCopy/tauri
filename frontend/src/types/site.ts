export interface Certificate {
  subject: string;
  issuer: string;
  serialNumber: string;
  validFrom: string;
  validTo: string;
  san: string[];
  cn: string;
  fingerprintSha256: string;
  signatureAlgorithm: string;
}

export interface ChainEntry {
  subject: string;
  issuer: string;
  serialNumber: string;
  validFrom: string;
  validTo: string;
  fingerprintSha256: string;
}

export interface Validation {
  hostname_ok: boolean;
  chain_ok: boolean;
  expired_ok: boolean;
  mincifry_ca_ok: boolean;
}

export interface InspectError {
  code: string;
  message: string;
}

export interface InspectResult {
  requestId: string;
  inputUrl: string;
  resolvedHost: string;
  tlsVersion: string;
  tlsCipher?: string;
  certificate: Certificate | null;
  chain: ChainEntry[];
  validation: Validation;
  is_mintsifry_ca: boolean;
  html: string;
  errors: InspectError[];
}

export interface KnownSite {
  id: string;
  title: string;
  url: string;
}

export const KNOWN_SITES: KnownSite[] = [
  { id: 'gosuslugi',     title: 'Госуслуги',                  url: 'https://gosuslugi.ru' },
  { id: 'esia',          title: 'ЕСИА',                       url: 'https://esia.gosuslugi.ru' },
  { id: 'nalog',         title: 'ФНС России',                 url: 'https://nalog.gov.ru' },
  { id: 'zakupki',       title: 'Единый портал закупок',      url: 'https://zakupki.gov.ru' },
  { id: 'gost',          title: 'Росстандарт',                url: 'https://gost.ru' },
];
