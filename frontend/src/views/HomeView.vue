<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import SiteSelector from "@/components/SiteSelector.vue";
import { useCheckSite } from "@/composables/useCheckSite";
import { isDemoMode } from "@/lib/invokeBackend";

const url = ref("https://gosuslugi.ru");
const loadHtml = ref(true);
const router = useRouter();
const { loading, error, checkSite } = useCheckSite();

async function onSubmit() {
  try {
    const res = await checkSite(url.value, { loadHtml: loadHtml.value });

    localStorage.setItem("lastResult", JSON.stringify(res));

    await router.push({
      name: "result",
      state: { payload: JSON.stringify(res) },
    });
  } catch {
    /* ошибка уже в `error` */
  }
}

function openSettings() {
  router.push({ name: "settings" });
}
</script>

<template>
  <main class="home">
    <header class="home-header">
      <div class="title-block">
        <h1>GosCertInspector</h1>
        <p class="subtitle">Проверка TLS-сертификатов гос-сайтов РФ</p>
      </div>
      <button
        class="settings-btn"
        type="button"
        :disabled="loading"
        aria-label="Настройки"
        title="Настройки"
        @click="openSettings"
      >
        ⚙
      </button>
    </header>

    <p v-if="isDemoMode" class="demo-badge" role="status">
      DEMO MODE — без Tauri, данные mock. В мобильной сборке вызовы идут в
      C++/OpenSSL.
    </p>

    <SiteSelector
      v-model="url"
      v-model:loadHtml="loadHtml"
      :disabled="loading"
      @submit="onSubmit"
    />

    <p v-if="loading" class="loading">Подключаемся и проверяем сертификат…</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
  </main>
</template>

<style scoped>
.home {
  padding: 20px 16px calc(env(safe-area-inset-bottom, 16px) + 20px);
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-width: 720px;
  margin: 0 auto;
  min-height: 100vh;
  min-height: 100dvh;
}
.home-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
}
.title-block {
  flex: 1;
  min-width: 0;
}
.home-header h1 {
  margin: 0;
  font-size: 22px;
  color: #0b3d91;
}
.subtitle {
  margin: 4px 0 0;
  color: #555;
  font-size: 13px;
}
.settings-btn {
  flex-shrink: 0;
  width: 40px;
  height: 40px;
  border: 1px solid #d0d7de;
  background: #fff;
  border-radius: 10px;
  font-size: 20px;
  line-height: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: #1f2937;
  -webkit-tap-highlight-color: transparent;
  transition:
    background 0.15s ease,
    transform 0.05s ease;
}
.settings-btn:hover:not(:disabled) {
  background: #f3f4f6;
}
.settings-btn:active:not(:disabled) {
  transform: scale(0.92);
  background: #e5e7eb;
}
.settings-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.demo-badge {
  margin: 0;
  padding: 6px 10px;
  font-size: 12px;
  background: #fff7ed;
  color: #7c2d12;
  border: 1px solid #fdba74;
  border-radius: 6px;
}
.loading {
  color: #555;
  font-style: italic;
}
.error {
  color: #7f1d1d;
  background: #fde8e8;
  padding: 8px;
  border-radius: 6px;
}
</style>
