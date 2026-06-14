<script setup lang="ts">
import type { ChainEntry } from "@/types/site";

defineProps<{
  chain: ChainEntry[];
}>();

function short(s: string, n = 64): string {
  return s.length > n ? s.slice(0, n) + "…" : s;
}
</script>

<template>
  <article class="chain" aria-label="Цепочка сертификатов">
    <h3>Цепочка ({{ chain.length }})</h3>
    <ol v-if="chain.length">
      <li v-for="(c, idx) in chain" :key="c.fingerprintSha256 || idx">
        <div class="role">
          <template v-if="idx === 0">End-entity</template>
          <template v-else-if="idx === chain.length - 1">Root</template>
          <template v-else>Intermediate</template>
        </div>
        <div class="subject mono">{{ short(c.subject) }}</div>
        <div class="meta">
          <span
            >Issuer: <span class="mono">{{ short(c.issuer, 48) }}</span></span
          >
          <span
            >SHA-256:
            <span class="mono">{{ short(c.fingerprintSha256, 20) }}</span></span
          >
        </div>
        <div class="valid">{{ c.validFrom }} → {{ c.validTo }}</div>
      </li>
    </ol>
    <p v-else class="empty">Цепочка пуста</p>
  </article>
</template>

<style scoped>
.chain {
  border: 1px solid #ddd;
  border-radius: 10px;
  padding: 12px;
  background: #fff;
}
h3 {
  margin: 0 0 8px;
  font-size: 16px;
}
ol {
  padding-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin: 0;
}
li {
  padding: 8px;
  border-left: 3px solid #0b3d91;
  background: #f5f7fb;
  border-radius: 0 6px 6px 0;
}
.role {
  font-size: 12px;
  color: #0b3d91;
  font-weight: 600;
  margin-bottom: 4px;
}
.subject {
  font-size: 13px;
}
.meta {
  font-size: 11px;
  color: #555;
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-top: 4px;
}
.valid {
  font-size: 11px;
  color: #777;
  margin-top: 4px;
}
.mono {
  font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
}
.empty {
  color: #777;
  font-style: italic;
}
</style>
