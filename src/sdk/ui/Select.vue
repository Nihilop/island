<script setup lang="ts">
// Select maison (design system Island) — pas de <select> natif moche.
// Trigger + menu flottant, click-outside, stylé via tokens. Neutre (s'adapte
// dark/light via currentColor + surfaces rgba).
import { ref, computed, onMounted, onUnmounted } from "vue";

interface Opt { value: string; label: string }
const props = defineProps<{ modelValue: string; options: (string | Opt)[] }>();
const emit = defineEmits<{ "update:modelValue": [string] }>();

const open = ref(false);
const root = ref<HTMLElement>();
const opts = computed<Opt[]>(() =>
  props.options.map((o) => (typeof o === "string" ? { value: o, label: o } : o)),
);
const current = computed(() => opts.value.find((o) => o.value === props.modelValue)?.label ?? props.modelValue);

function pick(v: string) { emit("update:modelValue", v); open.value = false; }
function onDoc(e: MouseEvent) { if (root.value && !root.value.contains(e.target as Node)) open.value = false; }
function onKey(e: KeyboardEvent) { if (e.key === "Escape") open.value = false; }

onMounted(() => { document.addEventListener("click", onDoc); document.addEventListener("keydown", onKey); });
onUnmounted(() => { document.removeEventListener("click", onDoc); document.removeEventListener("keydown", onKey); });
</script>

<template>
  <div ref="root" class="i-sel" :class="{ open }">
    <button type="button" class="trigger" @click.stop="open = !open">
      <span class="val">{{ current }}</span>
      <svg class="chev" viewBox="0 0 24 24" width="14" height="14"><path fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="m6 9 6 6 6-6" /></svg>
    </button>
    <div v-if="open" class="menu">
      <button v-for="o in opts" :key="o.value" type="button" class="opt" :class="{ on: o.value === modelValue }" @click.stop="pick(o.value)">
        <span>{{ o.label }}</span>
        <svg v-if="o.value === modelValue" class="tick" viewBox="0 0 24 24" width="14" height="14"><path fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" d="m5 12 5 5L20 7" /></svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.i-sel { position: relative; font-family: inherit; }
.trigger {
  display: flex; align-items: center; justify-content: space-between; gap: 8px;
  height: 34px; padding: 0 10px 0 12px; min-width: 130px;
  border-radius: 9px; border: 0.5px solid rgba(128, 128, 128, 0.3);
  background: rgba(128, 128, 128, 0.1); color: inherit; font: inherit; cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
}
.trigger:hover { background: rgba(128, 128, 128, 0.18); }
.i-sel.open .trigger { border-color: rgba(128, 128, 128, 0.5); }
.val { font-size: 13px; }
.chev { opacity: 0.55; transition: transform 0.18s ease; flex: none; }
.i-sel.open .chev { transform: rotate(180deg); }
.menu {
  position: absolute; right: 0; top: calc(100% + 6px); min-width: 100%;
  background: #232327; border: 0.5px solid rgba(255, 255, 255, 0.12); border-radius: 10px;
  padding: 4px; box-shadow: 0 12px 32px rgba(0, 0, 0, 0.4); z-index: 50;
  display: flex; flex-direction: column; gap: 1px;
}
.opt {
  display: flex; align-items: center; justify-content: space-between; gap: 12px;
  text-align: left; border: none; background: transparent; color: #e7e7ea; font: inherit;
  font-size: 13px; padding: 8px 10px; border-radius: 7px; cursor: pointer; white-space: nowrap;
}
.opt:hover { background: rgba(255, 255, 255, 0.1); }
.opt.on { color: #fff; }
.tick { color: var(--island-accent, #1db954); flex: none; }
</style>
