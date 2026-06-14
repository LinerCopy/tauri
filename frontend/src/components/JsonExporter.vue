<script setup lang="ts">
import { computed } from "vue";
import type { InspectResult } from "@/types/site";

const props = defineProps<{
  data: InspectResult;
}>();

const json = computed(() => JSON.stringify(props.data, null, 2));

async function copy() {
  try {
    await navigator.clipboard.writeText(json.value);
  } catch {
    const ta = document.createElement("textarea");
    ta.value = json.value;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
  }
}

function download() {
  const blob = new Blob([json.value], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `inspect-${props.data.requestId || "result"}.json`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}
</script>

<template>
  <article class="json-exporter" aria-label="Экспорт JSON">
    <header>
      <h3>JSON-отчёт</h3>
      <div class="actions">
        <button type="button" @click="copy">Копировать</button>
        <button type="button" @click="download">Скачать</button>
      </div>
    </header>
    <pre class="mono">{{ json }}</pre>
  </article>
</template>

<style scoped>
.json-exporter {
  border: 1px solid #ddd;
  border-radius: 10px;
  padding: 12px;
  background: #fff;
}
header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  margin-bottom: 8px;
}
h3 {
  margin: 0;
  font-size: 16px;
}
.actions button {
  margin-left: 6px;
  padding: 4px 10px;
  font-size: 12px;
  border: 1px solid #bbb;
  background: #f5f5f5;
  border-radius: 4px;
  cursor: pointer;
}
.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  font-size: 11px;
  background: #0f172a;
  color: #e2e8f0;
  padding: 8px;
  border-radius: 6px;
  overflow: auto;
  max-height: 320px;
  margin: 0;
}
</style>
