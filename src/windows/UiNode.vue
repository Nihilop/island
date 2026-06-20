<script setup lang="ts">
// Rendu d'un noeud du kit UI déclaratif (ship dans le SDK).
// Le plugin DÉCRIT des composants ; l'hôte les REND nativement. Le plugin ne
// touche jamais au DOM → cohérence + sécurité. Types supportés (v1) :
// text · toggle · input · select/segmented · slider · button · row · progress
interface UiNodeSpec {
  type: string;
  id?: string;
  label?: string;
  value?: unknown;
  options?: string[];
  min?: number;
  max?: number;
  placeholder?: string;
  variant?: "primary" | "ghost" | "danger";
  children?: UiNodeSpec[];
}

const props = defineProps<{ node: UiNodeSpec; values: Record<string, unknown> }>();
const emit = defineEmits<{ change: [{ id: string; value: unknown }]; action: [{ id: string }] }>();

function cur<T>(fallback: T): T {
  const id = props.node.id;
  return (id && id in props.values ? (props.values[id] as T) : (props.node.value as T)) ?? fallback;
}
function set(value: unknown) {
  if (props.node.id) emit("change", { id: props.node.id, value });
}
</script>

<template>
  <div v-if="node.type === 'text'" class="ui-text">{{ node.label }}</div>

  <div v-else-if="node.type === 'toggle'" class="ui-field">
    <span class="ui-label">{{ node.label }}</span>
    <div class="ui-sw" :class="{ on: cur(false) }" @click="set(!cur(false))"></div>
  </div>

  <div v-else-if="node.type === 'segmented' || node.type === 'select'" class="ui-field">
    <span class="ui-label">{{ node.label }}</span>
    <div class="ui-seg">
      <button v-for="o in node.options" :key="o" :class="{ on: cur('') === o }" @click="set(o)">{{ o }}</button>
    </div>
  </div>

  <div v-else-if="node.type === 'slider'" class="ui-field">
    <span class="ui-label">{{ node.label }}</span>
    <input class="ui-range" type="range" :min="node.min ?? 0" :max="node.max ?? 100"
           :value="cur(0)" @input="set(+($event.target as HTMLInputElement).value)" />
  </div>

  <div v-else-if="node.type === 'input'" class="ui-field col">
    <span class="ui-label">{{ node.label }}</span>
    <input class="ui-input" :value="cur('')" :placeholder="node.placeholder"
           @input="set(($event.target as HTMLInputElement).value)" />
  </div>

  <button v-else-if="node.type === 'button'" class="ui-btn" :class="node.variant || 'ghost'"
          @click="emit('action', { id: node.id || '' })">{{ node.label }}</button>

  <div v-else-if="node.type === 'progress'" class="ui-prog">
    <div class="ui-prog-fill" :style="{ width: cur(0) + '%' }"></div>
  </div>

  <div v-else-if="node.type === 'row'" class="ui-row">
    <UiNode v-for="(c, i) in node.children" :key="i" :node="c" :values="values"
            @change="emit('change', $event)" @action="emit('action', $event)" />
  </div>
</template>

<style scoped>
.ui-text { font-size: 13px; color: rgba(255, 255, 255, 0.6); padding: 4px 0; }
.ui-field { display: flex; align-items: center; justify-content: space-between; gap: 14px; padding: 11px 0; border-bottom: 0.5px solid rgba(255, 255, 255, 0.07); }
.ui-field.col { flex-direction: column; align-items: stretch; gap: 8px; }
.ui-field:last-child { border-bottom: none; }
.ui-label { font-size: 13px; color: rgba(255, 255, 255, 0.85); }
.ui-sw { width: 42px; height: 24px; border-radius: 13px; background: rgba(255, 255, 255, 0.18); position: relative; cursor: pointer; transition: background 0.2s; flex: none; }
.ui-sw.on { background: #1db954; }
.ui-sw::after { content: ""; position: absolute; top: 3px; left: 3px; width: 18px; height: 18px; border-radius: 50%; background: #fff; transition: left 0.2s; }
.ui-sw.on::after { left: 21px; }
.ui-seg { display: flex; gap: 4px; background: rgba(255, 255, 255, 0.08); border-radius: 10px; padding: 3px; }
.ui-seg button { border: none; background: transparent; color: rgba(255, 255, 255, 0.7); font-size: 12px; padding: 5px 12px; border-radius: 8px; cursor: pointer; font-family: inherit; }
.ui-seg button.on { background: #fff; color: #111; }
.ui-range { width: 130px; accent-color: #1db954; }
.ui-input { background: rgba(255, 255, 255, 0.08); border: 0.5px solid rgba(255, 255, 255, 0.12); border-radius: 10px; color: #fff; font-size: 13px; padding: 9px 12px; outline: none; font-family: inherit; }
.ui-btn { flex: 1; height: 40px; border-radius: 12px; border: none; font-size: 14px; font-weight: 500; cursor: pointer; font-family: inherit; }
.ui-btn.primary { background: #1db954; color: #fff; }
.ui-btn.ghost { background: rgba(255, 255, 255, 0.1); color: #fff; }
.ui-btn.danger { background: #ff3b30; color: #fff; }
.ui-btn:active { transform: scale(0.98); }
.ui-prog { height: 6px; border-radius: 3px; background: rgba(255, 255, 255, 0.15); overflow: hidden; }
.ui-prog-fill { height: 100%; border-radius: 3px; background: #1db954; transition: width 0.3s ease; }
.ui-row { display: flex; gap: 10px; padding-top: 6px; }
</style>
