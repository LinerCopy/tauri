<script setup lang="ts">
import { ref, computed } from "vue";
import { KNOWN_SITES, type KnownSite } from "@/types/site";

const props = defineProps<{
  modelValue: string;
  loadHtml: boolean;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", v: string): void;
  (e: "update:loadHtml", v: boolean): void;
  (e: "submit"): void;
}>();

const customUrl = ref(props.modelValue);
const selectedId = ref<string>("");

const isValid = computed(() =>
  /^https:\/\/[^\s]+$/i.test(customUrl.value.trim()),
);

function pickSite(site: KnownSite) {
  selectedId.value = site.id;
  customUrl.value = site.url;
  emit("update:modelValue", site.url);
}

function onInput(e: Event) {
  const v = (e.target as HTMLInputElement).value;
  customUrl.value = v;
  selectedId.value = "";
  emit("update:modelValue", v);
}

function onSubmit() {
  if (!isValid.value || props.disabled) return;
  emit("submit");
}
</script>

<template>
  <section class="site-selector" aria-label="Выбор сайта для проверки">
    <fieldset class="known">
      <legend>Госсайты</legend>
      <button
        v-for="s in KNOWN_SITES"
        :key="s.id"
        type="button"
        class="chip"
        :class="{ active: selectedId === s.id }"
        :disabled="disabled"
        @click="pickSite(s)"
      >
        {{ s.title }}
      </button>
    </fieldset>

    <label class="field">
      <span>Или введите свой URL</span>
      <input
        type="url"
        inputmode="url"
        placeholder="https://example.gov.ru/"
        :value="customUrl"
        :disabled="disabled"
        @input="onInput"
        @keydown.enter.prevent="onSubmit"
      />
    </label>

    <label class="switch">
      <input
        type="checkbox"
        :checked="loadHtml"
        :disabled="disabled"
        @change="
          emit('update:loadHtml', ($event.target as HTMLInputElement).checked)
        "
      />
      Загрузить HTML страницы
    </label>

    <button
      class="primary"
      type="button"
      :disabled="!isValid || disabled"
      @click="onSubmit"
    >
      Проверить
    </button>
  </section>
</template>

<style scoped>
.site-selector {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.known {
  border: 1px solid #ddd;
  border-radius: 8px;
  padding: 8px;
}
.chip {
  margin: 4px;
  padding: 8px 14px;
  border-radius: 20px;
  border: 1px solid #d1d5db;
  background: #fff;
  cursor: pointer;
  font-size: 14px;
  transition: background 0.15s, transform 0.1s, border-color 0.15s;
  -webkit-tap-highlight-color: transparent;
}
.chip:active {
  transform: scale(0.93);
}
.chip.active {
  background: #0b3d91;
  color: #fff;
  border-color: #0b3d91;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 14px;
}
.field input {
  padding: 10px;
  border-radius: 6px;
  border: 1px solid #bbb;
  font-size: 15px;
}
.switch {
  font-size: 14px;
  display: flex;
  align-items: center;
  gap: 6px;
}
.primary {
  padding: 14px;
  border: 0;
  border-radius: 12px;
  background: #0b3d91;
  color: #fff;
  font-size: 16px;
  font-weight: 600;
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  transition: background 0.15s, transform 0.1s;
}
.primary:active:not(:disabled) {
  background: #092d6b;
  transform: scale(0.97);
}
.primary:disabled {
  background: #9ca3af;
  cursor: not-allowed;
}
</style>
