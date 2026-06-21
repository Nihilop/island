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
  <div v-if="node.type === 'text'" class="py-1 text-[13px] text-white/60">{{ node.label }}</div>

  <div v-else-if="node.type === 'toggle'" class="flex items-center justify-between gap-3.5 border-b-[0.5px] border-white/[0.07] py-[11px] last:border-b-0">
    <span class="text-[13px] text-white/85">{{ node.label }}</span>
    <!-- bouton du switch = pseudo-élément ::after → variants after: -->
    <div
      class="relative h-6 w-[42px] flex-none cursor-pointer rounded-[13px] transition-[background] after:absolute after:top-[3px] after:h-[18px] after:w-[18px] after:rounded-full after:bg-white after:transition-[left] after:content-['']"
      :class="cur(false) ? 'bg-[#1db954] after:left-[21px]' : 'bg-white/[0.18] after:left-[3px]'"
      @click="set(!cur(false))"
    ></div>
  </div>

  <div v-else-if="node.type === 'segmented' || node.type === 'select'" class="flex items-center justify-between gap-3.5 border-b-[0.5px] border-white/[0.07] py-[11px] last:border-b-0">
    <span class="text-[13px] text-white/85">{{ node.label }}</span>
    <div class="flex gap-1 rounded-[10px] bg-white/[0.08] p-[3px]">
      <button
        v-for="o in node.options" :key="o"
        class="cursor-pointer rounded-lg border-none px-3 py-[5px] text-[12px]"
        :class="cur('') === o ? 'bg-white text-[#111]' : 'bg-transparent text-white/70'"
        @click="set(o)"
      >{{ o }}</button>
    </div>
  </div>

  <div v-else-if="node.type === 'slider'" class="flex items-center justify-between gap-3.5 border-b-[0.5px] border-white/[0.07] py-[11px] last:border-b-0">
    <span class="text-[13px] text-white/85">{{ node.label }}</span>
    <input class="w-[130px] accent-[#1db954]" type="range" :min="node.min ?? 0" :max="node.max ?? 100"
           :value="cur(0)" @input="set(+($event.target as HTMLInputElement).value)" />
  </div>

  <div v-else-if="node.type === 'input'" class="flex flex-col items-stretch gap-2 border-b-[0.5px] border-white/[0.07] py-[11px] last:border-b-0">
    <span class="text-[13px] text-white/85">{{ node.label }}</span>
    <input class="rounded-[10px] border-[0.5px] border-white/[0.12] bg-white/[0.08] px-3 py-[9px] text-[13px] text-white outline-none"
           :value="cur('')" :placeholder="node.placeholder"
           @input="set(($event.target as HTMLInputElement).value)" />
  </div>

  <button v-else-if="node.type === 'button'"
          class="h-10 flex-1 cursor-pointer rounded-xl border-none text-[14px] font-medium active:scale-[0.98]"
          :class="node.variant === 'primary' ? 'bg-[#1db954] text-white' : node.variant === 'danger' ? 'bg-[#ff3b30] text-white' : 'bg-white/10 text-white'"
          @click="emit('action', { id: node.id || '' })">{{ node.label }}</button>

  <div v-else-if="node.type === 'progress'" class="h-1.5 overflow-hidden rounded-[3px] bg-white/15">
    <div class="h-full rounded-[3px] bg-[#1db954] transition-[width] duration-300" :style="{ width: cur(0) + '%' }"></div>
  </div>

  <div v-else-if="node.type === 'row'" class="flex gap-2.5 pt-1.5">
    <UiNode v-for="(c, i) in node.children" :key="i" :node="c" :values="values"
            @change="emit('change', $event)" @action="emit('action', $event)" />
  </div>
</template>

<style scoped>
/* Les contrôles de formulaire n'héritent pas de la police par défaut (UA) → reset. */
button, input { font-family: inherit; }
</style>
