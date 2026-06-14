<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@/lib/invokeBackend";
import type { InspectResult } from "@/types/site";

const router = useRouter();
const data = ref<InspectResult | null>(null);
const toastMsg = ref("");
const showHtml = ref(false);
const showJson = ref(false);

onMounted(() => {
  const stored = localStorage.getItem("lastResult");
  if (stored) {
    try {
      data.value = JSON.parse(stored) as InspectResult;
    } catch {
      router.replace({ name: "home" });
    }
  } else {
    const raw = (history.state as { payload?: string } | null)?.payload;
    if (raw) {
      try {
        data.value = JSON.parse(raw) as InspectResult;
      } catch {
        router.replace({ name: "home" });
      }
    } else {
      router.replace({ name: "home" });
    }
  }
});

function goBack() {
  router.push({ name: "home" });
}

function formatCipher(cipher: string | undefined): string {
  if (!cipher) return "";

  return cipher
    .replace("TLS13_", "")
    .replace("TLS_ECDHE_RSA_WITH_", "")
    .replace("TLS_ECDHE_ECDSA_WITH_", "")
    .replace("TLS_RSA_WITH_", "")
    .replace("_", " ");
}

function formatDate(iso: string | undefined): string {
  if (!iso) return "—";
  try {
    const d = new Date(iso);
    return d.toLocaleDateString("ru-RU", {
      day: "numeric",
      month: "short",
      year: "numeric",
    });
  } catch {
    return iso;
  }
}

function getJsonString(): string {
  return JSON.stringify(data.value, null, 2);
}

const jsonString = computed(() => getJsonString());

const htmlDataUrl = computed(() => {
  const html = data.value?.html ?? "";
  if (!html) return "";
  const bytes = new TextEncoder().encode(html);
  let bin = "";
  bytes.forEach((b) => (bin += String.fromCharCode(b)));
  return `data:text/html;charset=utf-8;base64,${btoa(bin)}`;
});

async function copyJson() {
  try {
    await navigator.clipboard.writeText(getJsonString());
    showToast("Скопировано в буфер!");
  } catch {
    const ta = document.createElement("textarea");
    ta.value = getJsonString();
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
    showToast("Скопировано в буфер!");
  }
}

async function downloadJson() {
  const json = getJsonString();
  const host = data.value?.resolvedHost || "report";
  const filename = `${host}-${Date.now()}.json`;

  try {
    const path = await invoke<string>("save_report", {
      filename,
      content: json,
    });
    showToast(`Сохранено: ${path}`);
  } catch {
    try {
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = filename;
      a.style.display = "none";
      document.body.appendChild(a);
      a.click();
      setTimeout(() => {
        URL.revokeObjectURL(url);
        document.body.removeChild(a);
      }, 100);
      showToast("Сохранено!");
    } catch {
      showToast("Ошибка сохранения");
    }
  }
}

function showToast(msg: string) {
  toastMsg.value = msg;
  setTimeout(() => {
    toastMsg.value = "";
  }, 3000);
}
</script>

<template>
  <div class="result-view" v-if="data">
    <header class="result-header">
      <button
        class="back-btn"
        type="button"
        aria-label="Назад"
        @click="goBack"
      >
        <svg
          class="back-icon"
          viewBox="0 0 24 24"
          xmlns="http://www.w3.org/2000/svg"
          aria-hidden="true"
        >
          <path
            d="M19 12H5M12 19l-7-7 7-7"
            fill="none"
            stroke="currentColor"
            stroke-width="2.4"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </button>
      <div class="header-title">
        <span class="header-host">{{
          data.resolvedHost || data.inputUrl
        }}</span>
      </div>
    </header>

    <main class="result-content">
      <section class="card tls-card">
        <div class="tls-badge">
          <span class="tls-version">{{ data.tlsVersion }}</span>
          <span class="tls-cipher">{{ formatCipher(data.tlsCipher) }}</span>
        </div>
      </section>

      <section
        class="card status-card"
        :class="{
          'status-yes': data.is_mintsifry_ca,
          'status-no': !data.is_mintsifry_ca,
        }"
      >
        <div class="status-icon">{{ data.is_mintsifry_ca ? "🛡️" : "⚠️" }}</div>
        <div class="status-text">
          <strong>{{
            data.is_mintsifry_ca
              ? "Сертификат УЦ Минцифры"
              : "Не сертификат Минцифры"
          }}</strong>
          <p class="status-sub">
            Issuer: {{ data.certificate?.issuer || "—" }}
          </p>
        </div>
      </section>

      <section class="card">
        <h3 class="card-title">Проверки</h3>
        <ul class="checks-list">
          <li :class="{ ok: data.validation.hostname_ok }">
            <span class="check-icon">{{
              data.validation.hostname_ok ? "✓" : "✗"
            }}</span>
            Имя хоста
          </li>
          <li :class="{ ok: data.validation.chain_ok }">
            <span class="check-icon">{{
              data.validation.chain_ok ? "✓" : "✗"
            }}</span>
            Цепочка доверия
          </li>
          <li :class="{ ok: data.validation.expired_ok }">
            <span class="check-icon">{{
              data.validation.expired_ok ? "✓" : "✗"
            }}</span>
            Срок действия
          </li>
        </ul>
      </section>

      <section v-if="data.errors.length" class="card errors-card">
        <h3 class="card-title">Ошибки</h3>
        <div v-for="e in data.errors" :key="e.code" class="error-item">
          <strong>{{ e.code }}:</strong> {{ e.message }}
        </div>
      </section>

      <section class="card" v-if="data.certificate">
        <h3 class="card-title">Сертификат сервера</h3>
        <div class="cert-details">
          <div class="cert-row">
            <span class="cert-label">CN</span
            ><span class="cert-value">{{ data.certificate.cn }}</span>
          </div>
          <div class="cert-row">
            <span class="cert-label">Issuer</span
            ><span class="cert-value">{{ data.certificate.issuer }}</span>
          </div>
          <div class="cert-row">
            <span class="cert-label">Годен до</span
            ><span class="cert-value">{{
              formatDate(data.certificate.validTo)
            }}</span>
          </div>
          <div class="cert-row">
            <span class="cert-label">SAN</span
            ><span class="cert-value san-value">{{
              data.certificate.san?.join(", ")
            }}</span>
          </div>
          <div class="cert-row">
            <span class="cert-label">SHA-256</span
            ><span class="cert-value mono">{{
              data.certificate.fingerprintSha256
            }}</span>
          </div>
        </div>
      </section>

      <section class="card" v-if="data.chain?.length">
        <h3 class="card-title">Цепочка ({{ data.chain.length }})</h3>
        <div class="chain-list">
          <div class="chain-item" v-for="(cert, i) in data.chain" :key="i">
            <div class="chain-index">{{ i }}</div>
            <div class="chain-info">
              <div class="chain-subject">{{ cert.subject }}</div>
              <div class="chain-issuer">← {{ cert.issuer }}</div>
            </div>
          </div>
        </div>
      </section>

      <section class="card collapsible-card" v-if="data.html">
        <button
          class="collapsible-header"
          type="button"
          :aria-expanded="showHtml"
          @click="showHtml = !showHtml"
        >
          <span class="collapsible-title">
            HTML страницы
            <span class="collapsible-meta"
              >{{ data.html.length.toLocaleString("ru-RU") }} байт</span
            >
          </span>
          <span class="collapsible-chevron" :class="{ open: showHtml }">▾</span>
        </button>
        <div v-if="showHtml" class="collapsible-body">
          <iframe
            class="html-frame"
            :src="htmlDataUrl"
            sandbox=""
            referrerpolicy="no-referrer"
            loading="lazy"
            title="HTML страницы"
          />
        </div>
      </section>

      <section class="card collapsible-card">
        <button
          class="collapsible-header"
          type="button"
          :aria-expanded="showJson"
          @click="showJson = !showJson"
        >
          <span class="collapsible-title">
            JSON-отчёт
            <span class="collapsible-meta"
              >{{ jsonString.length.toLocaleString("ru-RU") }} символов</span
            >
          </span>
          <span class="collapsible-chevron" :class="{ open: showJson }">▾</span>
        </button>
        <pre v-if="showJson" class="json-pre">{{ jsonString }}</pre>
      </section>

      <section class="actions">
        <button class="action-btn action-copy" @click="copyJson">
          📋 Скопировать
        </button>
        <button class="action-btn action-download" @click="downloadJson">
          💾 Сохранить
        </button>
      </section>

      <div class="toast" v-if="toastMsg">{{ toastMsg }}</div>
    </main>
  </div>
</template>

<style scoped>
.result-view {
  min-height: 100vh;
  background: #f5f7fa;
}

.result-header {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: calc(env(safe-area-inset-top, 12px) + 8px) 16px 12px;
  background: #fff;
  border-bottom: 1px solid #e8ecf0;
  min-height: 56px;
  box-sizing: border-box;
}
.back-btn {
  flex-shrink: 0;
  width: 44px;
  height: 44px;
  border: none;
  background: #f0f2f5;
  border-radius: 12px;
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  -webkit-tap-highlight-color: transparent;
  cursor: pointer;
  color: #1f2937;
  line-height: 1;
}
.back-btn:active {
  transform: scale(0.92);
  background: #e0e3e8;
}
.back-icon {
  width: 24px;
  height: 24px;
  display: block;
  pointer-events: none;
}
.header-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}
.header-host {
  font-size: 16px;
  font-weight: 600;
  color: #1a1a1a;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  display: block;
}

.result-content {
  padding: calc(env(safe-area-inset-top, 12px) + 72px) 16px 32px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.card {
  background: #fff;
  border-radius: 14px;
  padding: 16px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.06);
}
.card-title {
  font-size: 13px;
  font-weight: 600;
  color: #6b7280;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  margin: 0 0 12px 0;
}

.tls-card {
  padding: 12px 16px;
}
.tls-badge {
  display: flex;
  align-items: center;
  gap: 8px;
}
.tls-version {
  background: #10b981;
  color: #fff;
  font-size: 12px;
  font-weight: 700;
  padding: 4px 10px;
  border-radius: 6px;
  white-space: nowrap;
  flex-shrink: 0;
}
.tls-cipher {
  font-size: 11px;
  color: #6b7280;
  font-family: monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.status-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px;
}
.status-yes {
  background: #ecfdf5;
  border: 1px solid #6ee7b7;
}
.status-no {
  background: #fef3c7;
  border: 1px solid #fcd34d;
}
.status-icon {
  font-size: 28px;
  flex-shrink: 0;
}
.status-text strong {
  font-size: 15px;
  color: #1a1a1a;
  display: block;
}
.status-sub {
  font-size: 12px;
  color: #6b7280;
  margin: 4px 0 0;
  word-break: break-all;
}

.checks-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.checks-list li {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  color: #dc2626;
}
.checks-list li.ok {
  color: #16a34a;
}
.check-icon {
  width: 20px;
  height: 20px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 700;
  flex-shrink: 0;
}
li.ok .check-icon {
  background: #dcfce7;
  color: #16a34a;
}
li:not(.ok) .check-icon {
  background: #fee2e2;
  color: #dc2626;
}

.errors-card {
  background: #fef2f2;
  border: 1px solid #fca5a5;
}
.error-item {
  font-size: 13px;
  color: #991b1b;
  margin-bottom: 4px;
  word-break: break-all;
}

.cert-details {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.cert-row {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.cert-label {
  font-size: 11px;
  font-weight: 600;
  color: #9ca3af;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}
.cert-value {
  font-size: 13px;
  color: #374151;
  word-break: break-all;
}
.cert-value.mono {
  font-family: monospace;
  font-size: 11px;
  color: #6b7280;
}
.san-value {
  font-size: 12px;
}

.chain-list {
  display: flex;
  flex-direction: column;
}
.chain-item {
  display: flex;
  gap: 10px;
  padding: 10px 0;
  border-bottom: 1px solid #f3f4f6;
}
.chain-item:last-child {
  border-bottom: none;
}
.chain-index {
  width: 22px;
  height: 22px;
  background: #e5e7eb;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 700;
  color: #6b7280;
  flex-shrink: 0;
}
.chain-info {
  min-width: 0;
  flex: 1;
}
.chain-subject {
  font-size: 13px;
  font-weight: 500;
  color: #1f2937;
  word-break: break-all;
}
.chain-issuer {
  font-size: 11px;
  color: #9ca3af;
  word-break: break-all;
  margin-top: 2px;
}

.actions {
  display: flex;
  gap: 10px;
  margin-top: 4px;
  padding-bottom: env(safe-area-inset-bottom, 16px);
}
.action-btn {
  flex: 1;
  padding: 14px 12px;
  border: none;
  border-radius: 12px;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  transition:
    transform 0.1s,
    box-shadow 0.15s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}
.action-btn:active {
  transform: scale(0.95);
}
.action-copy {
  background: #f0f2f5;
  color: #374151;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
}
.action-copy:active {
  background: #e5e7eb;
}
.action-download {
  background: linear-gradient(135deg, #2563eb, #1d4ed8);
  color: #fff;
  box-shadow: 0 2px 8px rgba(37, 99, 235, 0.3);
}
.action-download:active {
  background: linear-gradient(135deg, #1d4ed8, #1e40af);
}

.toast {
  position: fixed;
  bottom: calc(env(safe-area-inset-bottom, 16px) + 24px);
  left: 50%;
  transform: translateX(-50%);
  background: #1f2937;
  color: #fff;
  padding: 10px 20px;
  border-radius: 10px;
  font-size: 14px;
  z-index: 1000;
  animation: fadeUp 0.2s ease;
}
@keyframes fadeUp {
  from {
    opacity: 0;
    transform: translateX(-50%) translateY(8px);
  }
  to {
    opacity: 1;
    transform: translateX(-50%) translateY(0);
  }
}

/* Сворачиваемые секции HTML и JSON (ТЗ §9: "сертификат / цепочка / HTML / JSON"). */
.collapsible-card {
  padding: 0;
  overflow: hidden;
}
.collapsible-header {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  background: transparent;
  border: none;
  font-family: inherit;
  font-size: 13px;
  font-weight: 600;
  color: #6b7280;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
}
.collapsible-header:active {
  background: #f9fafb;
}
.collapsible-title {
  display: flex;
  align-items: baseline;
  gap: 8px;
}
.collapsible-meta {
  font-size: 11px;
  font-weight: 500;
  color: #9ca3af;
  text-transform: none;
  letter-spacing: 0;
}
.collapsible-chevron {
  font-size: 16px;
  color: #9ca3af;
  transition: transform 0.18s ease;
}
.collapsible-chevron.open {
  transform: rotate(180deg);
  color: #374151;
}
.collapsible-body {
  padding: 0 16px 16px;
}

/* HTML iframe — sandbox-режим, изолированное окружение (ТЗ §7). */
.html-frame {
  width: 100%;
  height: 360px;
  border: 1px solid #e5e7eb;
  border-radius: 8px;
  background: #fafafa;
  display: block;
}

/* JSON-pre — компактный шрифт, прокрутка по обоим осям. */
.json-pre {
  margin: 0 16px 16px;
  padding: 12px;
  background: #0f172a;
  color: #e2e8f0;
  border-radius: 8px;
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 11px;
  line-height: 1.45;
  white-space: pre;
  overflow: auto;
  max-height: 360px;
  -webkit-overflow-scrolling: touch;
}
</style>
