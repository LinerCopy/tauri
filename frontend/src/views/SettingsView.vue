<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@/lib/invokeBackend";

interface CertEntry {
  file: string;
  subject: string;
  fingerprintSha256: string;
  notAfter: string;
}
interface TrustStoreManifest {
  version: string;
  issuer: string;
  description: string;
  source?: string;
  updatedAt?: string;
  roots: CertEntry[];
  intermediates: CertEntry[];
}

interface UpdateCheckEntry {
  name: string;
  url: string;
  bundledFingerprint: string;
  remoteFingerprint: string | null;
  matchesBundled: boolean;
  error: string | null;
}
interface UpdateCheckResult {
  checkedAt: string;
  bundledVersion: string;
  entries: UpdateCheckEntry[];
  upToDate: boolean;
}

const LS_LAST_CHECK = "gci.lastUpdateCheck";

const router = useRouter();
const manifest = ref<TrustStoreManifest | null>(null);
const coreVersion = ref<string>("");
const error = ref<string | null>(null);
const toastMsg = ref("");
const checkingUpdate = ref(false);
const lastCheck = ref<UpdateCheckResult | null>(null);

onMounted(() => {
  loadManifest();
  restoreLastCheck();
});

function restoreLastCheck() {
  try {
    const raw = localStorage.getItem(LS_LAST_CHECK);
    if (!raw) return;
    const parsed = JSON.parse(raw) as UpdateCheckResult;
    if (parsed && typeof parsed.checkedAt === "string") {
      lastCheck.value = parsed;
    }
  } catch {
    /* ignore corrupt storage */
  }
}

async function loadManifest() {
  error.value = null;
  try {
    const [m, v] = await Promise.all([
      invoke<TrustStoreManifest>("trust_store_info"),
      invoke<string>("core_version"),
    ]);
    manifest.value = m;
    coreVersion.value = v;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}

function goBack() {
  router.push({ name: "home" });
}

function formatDate(iso: string | undefined): string {
  if (!iso) return "—";
  try {
    return new Date(iso).toLocaleDateString("ru-RU", {
      day: "numeric",
      month: "long",
      year: "numeric",
    });
  } catch {
    return iso;
  }
}

function formatDateTime(iso: string | undefined): string {
  if (!iso) return "—";
  try {
    return new Date(iso).toLocaleString("ru-RU", {
      day: "numeric",
      month: "short",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

function shortFp(fp: string): string {
  const parts = fp.split(":");
  if (parts.length <= 8) return fp;
  return parts.slice(0, 4).join(":") + " … " + parts.slice(-4).join(":");
}

async function copyFingerprint(fp: string) {
  await copyToClipboard(fp);
  showToast("Отпечаток скопирован");
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
  }
}

function openSource() {
  if (manifest.value?.source) {
    window.open(manifest.value.source, "_blank", "noopener,noreferrer");
  }
}

const allFingerprints = computed<string>(() => {
  if (!manifest.value) return "";
  const lines: string[] = [];
  lines.push(`trust-store ${manifest.value.version}`);
  lines.push(`source: ${manifest.value.source ?? "—"}`);
  lines.push("");
  for (const c of manifest.value.roots) {
    lines.push(`[ROOT] ${c.subject}`);
    lines.push(`SHA-256: ${c.fingerprintSha256}`);
    lines.push(`notAfter: ${c.notAfter}`);
    lines.push("");
  }
  for (const c of manifest.value.intermediates) {
    lines.push(`[SUB]  ${c.subject}`);
    lines.push(`SHA-256: ${c.fingerprintSha256}`);
    lines.push(`notAfter: ${c.notAfter}`);
    lines.push("");
  }
  return lines.join("\n").trimEnd();
});

async function copyAllFingerprints() {
  if (!allFingerprints.value) return;
  await copyToClipboard(allFingerprints.value);
  showToast("Все отпечатки скопированы");
}

/**
 * Реальная проверка обновлений: бэкенд скачивает официальные PEM
 * с сайта УЦ Минцифры и сравнивает SHA-256 с встроенным манифестом.
 *
 * По соображениям безопасности trust-store НЕ перезаписывается на лету
 * (это нарушило бы модель «trust-store подписан вместе с приложением»,
 * см. ТЗ §9). Если на сервере есть новая версия — пользователю
 * предлагается обновить приложение через App Store / Google Play.
 */
async function checkForUpdate() {
  if (checkingUpdate.value) return;
  checkingUpdate.value = true;
  try {
    const res = await invoke<UpdateCheckResult>("check_trust_store_updates");
    lastCheck.value = res;
    try {
      localStorage.setItem(LS_LAST_CHECK, JSON.stringify(res));
    } catch {
      /* private mode / quota — игнорируем */
    }
    await loadManifest();
    const hasError = res.entries.some((e: UpdateCheckEntry) => e.error);
    if (hasError) {
      showToast("Не удалось проверить часть источников");
    } else if (res.upToDate) {
      showToast(
        "Сертификаты актуальны (версия " +
          (manifest.value?.version ?? res.bundledVersion ?? "—") +
          ")",
      );
    } else {
      showToast("Доступно обновление — обновите приложение");
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    showToast("Ошибка проверки обновления");
  } finally {
    checkingUpdate.value = false;
  }
}

const updateBadge = computed(() => {
  if (!lastCheck.value) return null;
  if (lastCheck.value.entries.some((e: UpdateCheckEntry) => e.error)) {
    return { kind: "warn", text: "Не удалось проверить часть источников" };
  }
  if (lastCheck.value.upToDate) {
    return { kind: "ok", text: "Сертификаты актуальны" };
  }
  return { kind: "update", text: "Доступно обновление trust-store" };
});

function showToast(msg: string) {
  toastMsg.value = msg;
  setTimeout(() => {
    toastMsg.value = "";
  }, 2200);
}
</script>

<template>
  <div class="settings-view">
    <header class="settings-header">
      <button class="back-btn" type="button" aria-label="Назад" @click="goBack">
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
      <div class="header-title">Настройки</div>
    </header>

    <main class="settings-content">
      <section class="card">
        <h3 class="card-title">О приложении</h3>
        <div class="info-row">
          <span class="info-label">Ядро</span>
          <span class="info-value mono">{{ coreVersion || "…" }}</span>
        </div>
        <div class="info-row">
          <span class="info-label">Версия trust-store</span>
          <span class="info-value">{{ manifest?.version || "…" }}</span>
        </div>
        <div class="info-row" v-if="manifest?.updatedAt">
          <span class="info-label">Обновлён</span>
          <span class="info-value">{{ formatDate(manifest.updatedAt) }}</span>
        </div>
      </section>

      <section class="card update-card">
        <h3 class="card-title">Обновление сертификатов Минцифры</h3>
        <p class="info-desc">
          Корневые сертификаты УЦ Минцифры встроены в приложение на этапе сборки
          и подписаны вместе с бинарём — это защищает их от подмены в канале
          связи. Новые версии trust-store доставляются через App Store / Google
          Play вместе с обновлением приложения.
        </p>

        <div class="update-actions">
          <button
            class="btn btn-primary"
            type="button"
            :disabled="checkingUpdate || !manifest"
            @click="checkForUpdate"
          >
            <span v-if="checkingUpdate">Проверяем…</span>
            <span v-else>🔄 Проверить обновление</span>
          </button>

          <button
            class="btn btn-secondary"
            type="button"
            :disabled="!manifest"
            @click="copyAllFingerprints"
          >
            📋 Скопировать SHA-256 для сверки
          </button>
        </div>

        <div v-if="updateBadge" class="check-badge" :class="`badge-${updateBadge.kind}`">
          <span class="badge-dot" />
          <span>{{ updateBadge.text }}</span>
        </div>

        <p v-if="lastCheck" class="check-status">
          Последняя проверка: {{ formatDateTime(lastCheck.checkedAt) }}
        </p>

        <ul v-if="lastCheck" class="check-list">
          <li
            v-for="entry in lastCheck.entries"
            :key="entry.name"
            :class="{
              ok: entry.matchesBundled && !entry.error,
              warn: !entry.matchesBundled && !entry.error,
              err: !!entry.error,
            }"
          >
            <span class="check-icon">
              {{ entry.error ? "!" : entry.matchesBundled ? "✓" : "↑" }}
            </span>
            <span class="check-name">{{ entry.name }}</span>
            <span v-if="entry.error" class="check-msg">{{ entry.error }}</span>
            <span v-else-if="entry.matchesBundled" class="check-msg">актуален</span>
            <span v-else class="check-msg">доступна новая версия</span>
          </li>
        </ul>

        <p class="info-hint">
          Чтобы убедиться, что приложение работает с настоящими сертификатами
          Минцифры — скопируйте SHA-256 и сверьте их со значениями,
          опубликованными на <code>gosuslugi.ru/crt</code>.
        </p>
      </section>

      <section class="card" v-if="manifest?.source">
        <h3 class="card-title">Источник сертификатов</h3>
        <p class="info-desc">{{ manifest.description }}</p>
        <button class="link-btn" @click="openSource">
          🔗 {{ manifest.source }}
        </button>
      </section>

      <section class="card" v-if="manifest?.roots?.length">
        <h3 class="card-title">
          Корневые сертификаты ({{ manifest.roots.length }})
        </h3>
        <div class="cert-card" v-for="(c, i) in manifest.roots" :key="`r${i}`">
          <div class="cert-badge cert-badge-root">ROOT</div>
          <div class="cert-info">
            <div class="cert-subject">{{ c.subject }}</div>
            <div class="cert-meta">
              <span>До {{ formatDate(c.notAfter) }}</span>
            </div>
            <button
              class="fp-btn"
              @click="copyFingerprint(c.fingerprintSha256)"
            >
              <span class="fp-label">SHA-256</span>
              <span class="fp-value mono">{{
                shortFp(c.fingerprintSha256)
              }}</span>
              <span class="fp-copy">📋</span>
            </button>
          </div>
        </div>
      </section>

      <section class="card" v-if="manifest?.intermediates?.length">
        <h3 class="card-title">
          Промежуточные ({{ manifest.intermediates.length }})
        </h3>
        <div
          class="cert-card"
          v-for="(c, i) in manifest.intermediates"
          :key="`i${i}`"
        >
          <div class="cert-badge cert-badge-sub">SUB</div>
          <div class="cert-info">
            <div class="cert-subject">{{ c.subject }}</div>
            <div class="cert-meta">
              <span>До {{ formatDate(c.notAfter) }}</span>
            </div>
            <button
              class="fp-btn"
              @click="copyFingerprint(c.fingerprintSha256)"
            >
              <span class="fp-label">SHA-256</span>
              <span class="fp-value mono">{{
                shortFp(c.fingerprintSha256)
              }}</span>
              <span class="fp-copy">📋</span>
            </button>
          </div>
        </div>
      </section>

      <section v-if="error" class="card error-card">
        <h3 class="card-title">Ошибка</h3>
        <p>{{ error }}</p>
      </section>

      <div class="toast" v-if="toastMsg">{{ toastMsg }}</div>
    </main>
  </div>
</template>

<style scoped>
.settings-view {
  min-height: 100vh;
  background: #f5f7fa;
}

.settings-header {
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
  font-size: 17px;
  font-weight: 600;
  color: #1a1a1a;
}

.settings-content {
  padding: calc(env(safe-area-inset-top, 12px) + 72px) 16px
    env(safe-area-inset-bottom, 32px);
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-width: 720px;
  margin: 0 auto;
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

.info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 0;
  gap: 12px;
}
.info-label {
  font-size: 14px;
  color: #4b5563;
}
.info-value {
  font-size: 14px;
  color: #1f2937;
  font-weight: 500;
  text-align: right;
  word-break: break-word;
}
.info-value.mono {
  font-family: monospace;
  font-size: 12px;
}
.info-desc {
  font-size: 13px;
  color: #4b5563;
  margin: 0 0 10px;
  line-height: 1.4;
}
.info-hint {
  font-size: 11px;
  color: #9ca3af;
  margin: 10px 0 0;
  line-height: 1.4;
  font-style: italic;
}
.info-hint code {
  font-family: monospace;
  background: #f3f4f6;
  padding: 1px 4px;
  border-radius: 3px;
  font-style: normal;
}

.update-card {
  border-left: 3px solid #0b3d91;
}
.update-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 12px 0 4px;
}
.btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  padding: 12px 14px;
  border-radius: 10px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  transition:
    background 0.15s ease,
    transform 0.05s ease,
    opacity 0.15s ease;
}
.btn:active:not(:disabled) {
  transform: scale(0.98);
}
.btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
.btn-primary {
  background: #0b3d91;
  color: #fff;
  border: 1px solid #0b3d91;
}
.btn-primary:hover:not(:disabled) {
  background: #082f70;
}
.btn-secondary {
  background: #f3f4f6;
  color: #1f2937;
  border: 1px solid #d1d5db;
}
.btn-secondary:hover:not(:disabled) {
  background: #e5e7eb;
}
.check-status {
  font-size: 12px;
  color: #047857;
  margin: 8px 0 0;
}

.check-badge {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
  padding: 6px 12px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
}
.badge-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}
.badge-ok {
  background: #ecfdf5;
  color: #065f46;
  border: 1px solid #6ee7b7;
}
.badge-ok .badge-dot {
  background: #10b981;
}
.badge-update {
  background: #eff6ff;
  color: #1e40af;
  border: 1px solid #93c5fd;
}
.badge-update .badge-dot {
  background: #3b82f6;
}
.badge-warn {
  background: #fef3c7;
  color: #92400e;
  border: 1px solid #fcd34d;
}
.badge-warn .badge-dot {
  background: #f59e0b;
}

.check-list {
  list-style: none;
  margin: 8px 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.check-list li {
  display: grid;
  grid-template-columns: 18px 70px 1fr;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  padding: 6px 8px;
  border-radius: 8px;
  background: #f9fafb;
  border: 1px solid #e5e7eb;
}
.check-list .check-icon {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 700;
}
.check-list .check-name {
  font-family: monospace;
  font-weight: 600;
  color: #374151;
}
.check-list .check-msg {
  color: #6b7280;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.check-list li.ok .check-icon {
  background: #dcfce7;
  color: #16a34a;
}
.check-list li.warn .check-icon {
  background: #dbeafe;
  color: #1d4ed8;
}
.check-list li.warn .check-msg {
  color: #1d4ed8;
}
.check-list li.err .check-icon {
  background: #fee2e2;
  color: #b91c1c;
}
.check-list li.err .check-msg {
  color: #b91c1c;
}

.link-btn {
  display: block;
  width: 100%;
  padding: 12px;
  border: 1px solid #2563eb;
  border-radius: 10px;
  background: #eff6ff;
  color: #1d4ed8;
  font-size: 13px;
  font-family: monospace;
  text-align: left;
  cursor: pointer;
  word-break: break-all;
  -webkit-tap-highlight-color: transparent;
}
.link-btn:active {
  background: #dbeafe;
}

.cert-card {
  display: flex;
  gap: 12px;
  padding: 12px;
  margin-top: 8px;
  background: #f9fafb;
  border-radius: 10px;
  border: 1px solid #e5e7eb;
}
.cert-badge {
  flex-shrink: 0;
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 0.5px;
  display: flex;
  align-items: center;
  justify-content: center;
  height: fit-content;
}
.cert-badge-root {
  background: #fef3c7;
  color: #92400e;
}
.cert-badge-sub {
  background: #dbeafe;
  color: #1e40af;
}
.cert-info {
  flex: 1;
  min-width: 0;
}
.cert-subject {
  font-size: 13px;
  font-weight: 500;
  color: #1f2937;
  word-break: break-word;
  line-height: 1.3;
}
.cert-meta {
  font-size: 11px;
  color: #6b7280;
  margin: 4px 0 8px;
}

.fp-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 10px;
  border: none;
  border-radius: 8px;
  background: #fff;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  text-align: left;
}
.fp-btn:active {
  background: #f3f4f6;
}
.fp-label {
  font-size: 10px;
  font-weight: 700;
  color: #9ca3af;
  letter-spacing: 0.3px;
}
.fp-value {
  flex: 1;
  font-size: 11px;
  color: #4b5563;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.fp-value.mono {
  font-family: monospace;
}
.fp-copy {
  font-size: 12px;
  opacity: 0.5;
}

.error-card {
  background: #fef2f2;
  border: 1px solid #fca5a5;
  color: #991b1b;
  font-size: 13px;
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
  max-width: 90vw;
  text-align: center;
}
</style>
