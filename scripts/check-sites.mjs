#!/usr/bin/env node
/**
 * check-sites.mjs — Node CLI, который реально подключается к указанным
 * сайтам по TLS, парсит peer-сертификат и цепочку, делает HTTP GET и
 * формирует JSON-отчёт.
 *
 * Примеры:
 *   node scripts/check-sites.mjs                       # стандартный набор
 *   node scripts/check-sites.mjs https://gosuslugi.ru  # один URL
 *   node scripts/check-sites.mjs --no-html             # без загрузки HTML
 *   node scripts/check-sites.mjs --out reports/        # выгрузить JSON по сайту
 */

import tls from 'node:tls';
import https from 'node:https';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const DEFAULT_SITES = [
  'https://gosuslugi.ru',
  'https://esia.gosuslugi.ru',
  'https://nalog.gov.ru',
  'https://zakupki.gov.ru',
  'https://gost.ru',
];

const MAX_HTML_BYTES = 1024 * 1024;
const TIMEOUT_MS = 15_000;

const args = process.argv.slice(2);
const flags = new Set(args.filter((a) => a.startsWith('-')));
const positional = args.filter((a) => !a.startsWith('-') && !a.startsWith('out='));
const outArg = args.find((a) => a.startsWith('--out='));
const outDir = outArg ? outArg.slice('--out='.length) : null;
const loadHtml = !flags.has('--no-html');
const quiet = flags.has('--quiet');
const sites = positional.length ? positional : DEFAULT_SITES;

const C = {
  reset: '\x1b[0m', bold: '\x1b[1m',
  green: '\x1b[32m', red: '\x1b[31m', yellow: '\x1b[33m',
  cyan: '\x1b[36m', dim: '\x1b[2m', blue: '\x1b[34m',
};
const tty = process.stdout.isTTY;
const c = (color, s) => (tty ? `${C[color]}${s}${C.reset}` : s);
const log = (...m) => { if (!quiet) console.log(...m); };

const MINCIFRY_MARKERS = [
  'russian trusted',
  'ministry of digital development',
  'минцифры',
  'минцифра',
];

function nameToString(n) {
  if (!n) return '';
  if (typeof n === 'string') return n;
  return Object.entries(n)
    .filter(([k]) => !['subjectaltname'].includes(k.toLowerCase()))
    .map(([k, v]) => `${k}=${Array.isArray(v) ? v.join('+') : v}`)
    .join(',');
}

function asn1HexSerial(serial) {
  if (!serial) return '';
  if (Buffer.isBuffer(serial)) return serial.toString('hex').toUpperCase();
  return String(serial).toUpperCase();
}

function isoDate(s) {
  if (!s) return '';
  const d = new Date(s);
  return Number.isNaN(d.getTime()) ? '' : d.toISOString().replace(/\.\d{3}Z$/, 'Z');
}

function fingerprint(cert) {
  return (cert.fingerprint256 || '').replace(/:/g, '').toUpperCase();
}

function sanList(cert) {
  if (!cert.subjectaltname) return [];
  return cert.subjectaltname
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
}

function isMincifryName(name) {
  const s = (name || '').toLowerCase();
  return MINCIFRY_MARKERS.some((m) => s.includes(m));
}

function buildChain(rootCert) {
  const out = [];
  let cur = rootCert;
  const seen = new Set();
  while (cur && !seen.has(fingerprint(cur))) {
    seen.add(fingerprint(cur));
    out.push(cur);
    cur = cur.issuerCertificate;
    if (cur && fingerprint(cur) === fingerprint(out[out.length - 1])) break;
  }
  return out;
}

function makeCertDto(cert) {
  return {
    subject: nameToString(cert.subject),
    issuer: nameToString(cert.issuer),
    serialNumber: asn1HexSerial(cert.serialNumber),
    validFrom: isoDate(cert.valid_from),
    validTo: isoDate(cert.valid_to),
    san: sanList(cert),
    cn: (cert.subject && cert.subject.CN) || '',
    fingerprintSha256: fingerprint(cert),
    signatureAlgorithm: cert.asn1Curve || cert.sigalg || cert.signatureAlgorithm
      || cert.pubkey?.asymmetricKeyType
      || 'unknown',
  };
}

function makeChainDto(cert) {
  return {
    subject: nameToString(cert.subject),
    issuer: nameToString(cert.issuer),
    serialNumber: asn1HexSerial(cert.serialNumber),
    validFrom: isoDate(cert.valid_from),
    validTo: isoDate(cert.valid_to),
    fingerprintSha256: fingerprint(cert),
  };
}

function checkHostname(cert, host) {
  try {
    return tls.checkServerIdentity(host, cert) === undefined;
  } catch {
    return false;
  }
}

function notExpired(cert) {
  const from = new Date(cert.valid_from).getTime();
  const to = new Date(cert.valid_to).getTime();
  const now = Date.now();
  return Number.isFinite(from) && Number.isFinite(to) && from <= now && now <= to;
}

function fetchTlsAndHtml(urlStr) {
  return new Promise((resolve) => {
    let parsed;
    try {
      parsed = new URL(urlStr);
    } catch (e) {
      return resolve({ ok: false, error: { code: 'URL_PARSE', message: e.message } });
    }
    if (parsed.protocol !== 'https:') {
      return resolve({ ok: false, error: { code: 'URL_PARSE', message: 'only https supported' } });
    }
    const host = parsed.hostname;
    const port = parsed.port ? Number(parsed.port) : 443;
    const path_ = parsed.pathname + (parsed.search || '');

    const opts = {
      host,
      port,
      servername: host,
      method: 'GET',
      path: path_ || '/',
      headers: {
        Host: host,
        'User-Agent': 'GosCertInspector-CLI/1.0 (+local-only)',
        Accept: 'text/html,application/xhtml+xml;q=0.9,*/*;q=0.5',
        'Accept-Encoding': 'identity',
        Connection: 'close',
      },
      minVersion: 'TLSv1.2',
      maxVersion: 'TLSv1.3',
      rejectUnauthorized: false,
      timeout: TIMEOUT_MS,
    };

    const req = https.request(opts, (res) => {
      const socket = res.socket;
      const peer = socket.getPeerCertificate(true);
      const cipher = socket.getCipher();
      const protocol = socket.getProtocol();
      const authorized = socket.authorized;
      const authorizationError = socket.authorizationError;

      let body = '';
      let totalBytes = 0;
      let truncated = false;

      if (!loadHtml) {
        res.destroy();
        resolve({
          ok: true,
          host, port, protocol, cipher, authorized, authorizationError,
          peer, statusCode: res.statusCode, body: '', truncated: false,
        });
        return;
      }

      res.setEncoding('utf8');
      res.on('data', (chunk) => {
        totalBytes += Buffer.byteLength(chunk);
        if (totalBytes <= MAX_HTML_BYTES) {
          body += chunk;
        } else if (!truncated) {
          body += chunk.slice(0, MAX_HTML_BYTES - (totalBytes - Buffer.byteLength(chunk)));
          truncated = true;
          res.destroy();
        }
      });
      res.on('end', () => {
        resolve({
          ok: true,
          host, port, protocol, cipher, authorized, authorizationError,
          peer, statusCode: res.statusCode, body, truncated,
        });
      });
      res.on('error', (e) => resolve({
        ok: false, error: { code: 'HTTP_GET', message: e.message },
      }));
    });

    req.on('timeout', () => {
      req.destroy(new Error(`timeout after ${TIMEOUT_MS}ms`));
    });
    req.on('error', (e) => resolve({
      ok: false, error: { code: 'TLS_HANDSHAKE', message: e.message },
    }));
    req.end();
  });
}

function buildReport(urlStr, raw) {
  const requestId = crypto.randomBytes(16).toString('hex');
  if (!raw.ok) {
    return {
      requestId,
      inputUrl: urlStr,
      resolvedHost: tryHost(urlStr),
      tlsVersion: '',
      certificate: null,
      chain: [],
      validation: {
        hostname_ok: false, chain_ok: false, expired_ok: false, mincifry_ca_ok: false,
      },
      is_mintsifry_ca: false,
      html: '',
      errors: [raw.error],
    };
  }

  const chainCerts = buildChain(raw.peer);
  const endEntity = chainCerts[0];
  const cert = endEntity ? makeCertDto(endEntity) : null;
  const chain = chainCerts.map(makeChainDto);

  const hostname_ok = endEntity ? checkHostname(endEntity, raw.host) : false;
  const expired_ok = endEntity ? notExpired(endEntity) : false;
  const chain_ok = !!raw.authorized;

  let mincifry = false;
  if (endEntity) {
    if (isMincifryName(nameToString(endEntity.issuer))) mincifry = true;
    for (let i = 1; i < chainCerts.length; i++) {
      if (isMincifryName(nameToString(chainCerts[i].subject))) {
        mincifry = true;
        break;
      }
    }
  }

  const errors = [];
  if (raw.authorizationError && !chain_ok) {
    errors.push({ code: 'CHAIN_INVALID', message: String(raw.authorizationError) });
  }
  if (!hostname_ok) errors.push({ code: 'HOSTNAME_MISMATCH', message: `host ${raw.host} mismatch` });
  if (!expired_ok) errors.push({ code: 'EXPIRED', message: 'certificate is outside validity window' });
  if (raw.statusCode && raw.statusCode >= 400) errors.push({
    code: 'HTTP_STATUS', message: `HTTP ${raw.statusCode}`,
  });
  if (raw.truncated) errors.push({
    code: 'HTML_TRUNCATED', message: `HTML truncated at ${MAX_HTML_BYTES} bytes`,
  });

  return {
    requestId,
    inputUrl: urlStr,
    resolvedHost: raw.host,
    tlsVersion: raw.protocol ? raw.protocol.replace('TLSv', 'TLS ') : '',
    tlsCipher: raw.cipher?.name || '',
    certificate: cert,
    chain,
    validation: {
      hostname_ok,
      chain_ok,
      expired_ok,
      mincifry_ca_ok: mincifry,
    },
    is_mintsifry_ca: mincifry,
    html: raw.body || '',
    errors,
  };
}

function tryHost(urlStr) {
  try { return new URL(urlStr).host; } catch { return ''; }
}

function printReport(r) {
  const v = r.validation;
  const flag = (b) => (b ? c('green', '✔') : c('red', '✘'));
  const tag = (b, text) => (b ? c('green', text) : c('red', text));
  log(`\n${c('bold', '═══ ' + r.inputUrl + ' ═══')}`);
  if (r.errors.length && !r.certificate) {
    log(`  ${c('red', 'FAILED:')} ${r.errors.map((e) => `${e.code}: ${e.message}`).join('; ')}`);
    return;
  }
  log(`  host:        ${c('cyan', r.resolvedHost)}`);
  log(`  TLS:         ${r.tlsVersion} ${c('dim', r.tlsCipher ? '(' + r.tlsCipher + ')' : '')}`);
  if (r.certificate) {
    log(`  CN:          ${r.certificate.cn}`);
    log(`  Issuer:      ${c('dim', r.certificate.issuer)}`);
    log(`  Valid:       ${r.certificate.validFrom} → ${r.certificate.validTo}`);
    log(`  SHA-256:     ${c('dim', r.certificate.fingerprintSha256)}`);
    log(`  Chain depth: ${r.chain.length}`);
  }
  log(`  ${flag(v.hostname_ok)} hostname_ok    ${flag(v.chain_ok)} chain_ok    ${flag(v.expired_ok)} expired_ok`);
  log(`  ${tag(v.mincifry_ca_ok, (v.mincifry_ca_ok ? '★ ' : '○ ') + 'is_mintsifry_ca = ' + v.mincifry_ca_ok)}`);
  if (r.errors.length) {
    log(`  ${c('yellow', 'warnings:')}`);
    r.errors.forEach((e) => log(`    - ${e.code}: ${e.message}`));
  }
}

function printSummary(reports) {
  log(`\n${c('bold', '─── Сводка ───')}`);
  const w1 = Math.max(...reports.map((r) => r.inputUrl.length), 20);
  const head = ` URL`.padEnd(w1 + 2)
    + 'host'.padEnd(8) + 'chain'.padEnd(8)
    + 'expired'.padEnd(10) + 'Минцифры';
  log(c('dim', head));
  for (const r of reports) {
    const v = r.validation;
    const line = ' ' + r.inputUrl.padEnd(w1 + 1)
      + (v.hostname_ok ? c('green', '✔') : c('red', '✘')).padEnd(8)
      + (v.chain_ok    ? c('green', '✔') : c('red', '✘')).padEnd(8)
      + (v.expired_ok  ? c('green', '✔') : c('red', '✘')).padEnd(10)
      + (v.mincifry_ca_ok ? c('green', '★ да') : c('red', '○ нет'));
    log(line);
  }
}

(async () => {
  if (flags.has('-h') || flags.has('--help')) {
    console.log(`Usage: node scripts/check-sites.mjs [URL...] [--no-html] [--quiet] [--out=DIR]

Без аргументов проверяет стандартный набор:
${DEFAULT_SITES.map((s) => '  - ' + s).join('\n')}

Опции:
  --no-html      не загружать HTML страницы
  --quiet        печатать только финальную сводку
  --out=DIR      сохранить отчёт по каждому сайту в DIR/<host>.json
                 + DIR/_summary.json со сводным отчётом
  -h, --help     показать справку
`);
    process.exit(0);
  }

  if (outDir) fs.mkdirSync(outDir, { recursive: true });

  log(c('bold', `GosCertInspector CLI — ${sites.length} site(s)`));
  if (!loadHtml) log(c('dim', '(HTML loading disabled)'));

  const reports = [];
  for (const url of sites) {
    process.stdout.write(c('dim', `  → ${url} ... `));
    const t0 = Date.now();
    const raw = await fetchTlsAndHtml(url);
    const report = buildReport(url, raw);
    const dt = Date.now() - t0;
    process.stdout.write(c('dim', `${dt}ms\n`));
    reports.push(report);

    if (outDir) {
      const host = report.resolvedHost || tryHost(url) || 'unknown';
      const file = path.join(outDir, `${host}.json`);
      fs.writeFileSync(file, JSON.stringify(report, null, 2));
    }
  }

  for (const r of reports) printReport(r);
  printSummary(reports);

  if (outDir) {
    const summary = {
      generatedAt: new Date().toISOString(),
      sites: reports.map((r) => ({
        url: r.inputUrl,
        host: r.resolvedHost,
        tlsVersion: r.tlsVersion,
        validation: r.validation,
        is_mintsifry_ca: r.is_mintsifry_ca,
        errors: r.errors,
      })),
    };
    const file = path.join(outDir, '_summary.json');
    fs.writeFileSync(file, JSON.stringify(summary, null, 2));
    log(`\n${c('cyan', '✓')} JSON-отчёты сохранены в ${path.resolve(outDir)}/`);
  }
})().catch((e) => {
  console.error(c('red', `[FATAL] ${e.stack || e.message || e}`));
  process.exit(2);
});
