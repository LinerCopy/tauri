<script setup lang="ts">
import type { Certificate, Validation } from "@/types/site";

defineProps<{
  cert: Certificate;
  validation: Validation;
  tlsVersion: string;
  tlsCipher?: string;
}>();
</script>

<template>
  <article class="cert-card" aria-label="Сертификат сервера">
    <header>
      <h3>Сертификат сервера</h3>
      <p class="tls">
        <span>{{ tlsVersion || "—" }}</span>
        <span v-if="tlsCipher" class="cipher">{{ tlsCipher }}</span>
      </p>
    </header>

    <dl class="grid">
      <dt>CN</dt>
      <dd>{{ cert.cn || "—" }}</dd>
      <dt>Subject</dt>
      <dd class="mono">{{ cert.subject }}</dd>
      <dt>Issuer</dt>
      <dd class="mono">{{ cert.issuer }}</dd>
      <dt>Serial</dt>
      <dd class="mono">{{ cert.serialNumber }}</dd>
      <dt>Действителен с</dt>
      <dd>{{ cert.validFrom }}</dd>
      <dt>Действителен по</dt>
      <dd>{{ cert.validTo }}</dd>
      <dt>SHA-256</dt>
      <dd class="mono wrap">{{ cert.fingerprintSha256 }}</dd>
      <dt>Подпись</dt>
      <dd>{{ cert.signatureAlgorithm }}</dd>
      <dt>SAN</dt>
      <dd>
        <ul class="san">
          <li v-for="s in cert.san" :key="s">{{ s }}</li>
        </ul>
      </dd>
    </dl>

    <ul class="checks">
      <li :class="validation.hostname_ok ? 'ok' : 'bad'">
        Hostname: {{ validation.hostname_ok ? "ок" : "ошибка" }}
      </li>
      <li :class="validation.chain_ok ? 'ok' : 'bad'">
        Цепочка: {{ validation.chain_ok ? "доверенная" : "не доверена" }}
      </li>
      <li :class="validation.expired_ok ? 'ok' : 'bad'">
        Срок: {{ validation.expired_ok ? "действителен" : "истёк" }}
      </li>
      <li :class="validation.mincifry_ca_ok ? 'ok' : 'bad'" class="mincifry">
        УЦ Минцифры: {{ validation.mincifry_ca_ok ? "да" : "нет" }}
      </li>
    </ul>
  </article>
</template>

<style scoped>
.cert-card {
  border: 1px solid #ddd;
  border-radius: 10px;
  padding: 12px;
  background: #fff;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
}
h3 {
  margin: 0;
  font-size: 16px;
}
.tls {
  margin: 0;
  font-size: 12px;
  color: #555;
}
.cipher {
  margin-left: 6px;
  padding: 2px 6px;
  background: #eef;
  border-radius: 4px;
}
.grid {
  display: grid;
  grid-template-columns: 120px 1fr;
  gap: 4px 8px;
  font-size: 13px;
  margin: 0;
}
dt {
  color: #777;
}
dd {
  margin: 0;
  word-break: break-word;
}
.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
.wrap {
  word-break: break-all;
}
.san {
  list-style: disc;
  padding-left: 18px;
  margin: 0;
}
.checks {
  list-style: none;
  padding: 0;
  margin: 0;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 4px;
  font-size: 13px;
}
.checks li {
  padding: 6px 8px;
  border-radius: 6px;
}
.ok {
  background: #e7f7ec;
  color: #14532d;
}
.bad {
  background: #fde8e8;
  color: #7f1d1d;
}
.mincifry {
  grid-column: 1 / -1;
  font-weight: 600;
}
</style>
