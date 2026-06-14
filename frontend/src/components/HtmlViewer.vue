<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  html: string;
}>();

const dataUrl = computed(() => {
  if (!props.html) return "";
  
  const enc = new TextEncoder().encode(props.html);
  let bin = "";
  enc.forEach((b) => (bin += String.fromCharCode(b)));
  return `data:text/html;charset=utf-8;base64,${btoa(bin)}`;
});
</script>

<template>
  <article class="html-viewer" aria-label="HTML страницы">
    <header>
      <h3>HTML</h3>
      <span class="size">{{ html.length.toLocaleString("ru-RU") }} байт</span>
    </header>
    <iframe
      v-if="dataUrl"
      :src="dataUrl"
      sandbox=""
      referrerpolicy="no-referrer"
      title="HTML страницы"
      loading="lazy"
    />
    <p v-else class="empty">HTML не запрошен или пуст.</p>
  </article>
</template>

<style scoped>
.html-viewer {
  border: 1px solid #ddd;
  border-radius: 10px;
  background: #fff;
  padding: 12px;
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
.size {
  font-size: 12px;
  color: #777;
}
iframe {
  width: 100%;
  height: 360px;
  border: 1px solid #eee;
  border-radius: 6px;
  background: #fafafa;
}
.empty {
  color: #777;
  font-style: italic;
}
</style>
