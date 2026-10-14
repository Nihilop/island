<script setup lang="ts">
// Barre de progression / de niveau (lecture, mixer, volume, réglages) : piste 5 px → 9 px
// et plus claire au survol ou pendant le drag ; remplissage par TRANSLATION (pas
// d'étirement). Zone de saisie 16 px. `drag` = valeur en cours (pour afficher la cible),
// `commit` = valeur au relâchement (un seul seek par geste). Molette/flèches optionnelles.
// `size="thick"` : piste épaisse de 24 px remplie en blanc (tuile volume).
import { computed, ref } from "vue";
import { cn } from "@/lib/utils";
import { hold } from "../notch/state";

const props = withDefaults(
  defineProps<{
    value: number; // 0..1
    interactive?: boolean;
    /** Suit le curseur quasi directement pendant le drag (75 ms) au lieu de glisser. */
    follow?: boolean;
    wheelStep?: number;
    keyStep?: number;
    size?: "default" | "thick";
    /** Couleur du remplissage (sinon label-2 → label). */
    color?: string;
  }>(),
  { interactive: true, size: "default" },
);
const emit = defineEmits<{ drag: [v: number]; commit: [v: number] }>();

const el = ref<HTMLElement>();
const dragging = ref(false);
const dragValue = ref(0);
let release: (() => void) | null = null;

const shown = computed(() => (dragging.value ? dragValue.value : Math.min(1, Math.max(0, props.value))));

function valueAt(e: PointerEvent): number {
  const r = el.value!.getBoundingClientRect();
  return Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
}

function down(e: PointerEvent) {
  if (!props.interactive || e.button !== 0) return;
  dragging.value = true;
  dragValue.value = valueAt(e);
  release = hold();
  el.value!.setPointerCapture(e.pointerId);
  emit("drag", dragValue.value);
}
function move(e: PointerEvent) {
  if (!dragging.value) return;
  dragValue.value = valueAt(e);
  emit("drag", dragValue.value);
}
function up() {
  if (!dragging.value) return;
  dragging.value = false;
  emit("commit", dragValue.value);
  release?.();
  release = null;
}

function wheel(e: WheelEvent) {
  if (!props.interactive || !props.wheelStep) return;
  e.preventDefault();
  const step = e.shiftKey ? props.wheelStep * 5 : props.wheelStep;
  emit("commit", Math.min(1, Math.max(0, props.value + (e.deltaY < 0 ? step : -step))));
}
function key(e: KeyboardEvent) {
  if (!props.interactive || !props.keyStep) return;
  if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
  e.preventDefault();
  emit("commit", Math.min(1, Math.max(0, props.value + (e.key === "ArrowRight" ? props.keyStep : -props.keyStep))));
}

const thick = computed(() => props.size === "thick");
</script>

<template>
  <div
    ref="el"
    role="slider"
    :aria-valuenow="Math.round(shown * 100)"
    :tabindex="interactive ? 0 : -1"
    data-no-drag
    :data-dragging="dragging || undefined"
    :class="cn('group/track relative flex items-center outline-none', thick ? 'h-6' : 'h-4', interactive && 'cursor-pointer')"
    @pointerdown="down"
    @pointermove="move"
    @pointerup="up"
    @pointercancel="up"
    @wheel="wheel"
    @keydown="key"
  >
    <div
      :class="
        cn(
          'relative w-full overflow-hidden bg-fill-2 transition-[height] duration-150',
          thick ? 'h-6 rounded-sm' : 'h-[5px] rounded-full',
          !thick && interactive && 'group-hover/track:h-[9px] group-data-dragging/track:h-[9px]',
        )
      "
    >
      <div
        :class="
          cn(
            'absolute inset-0 rounded-[inherit] ease-out',
            thick ? 'bg-label' : 'bg-label-2',
            !thick && interactive && 'group-hover/track:bg-label group-data-dragging/track:bg-label',
            dragging ? (follow ? 'transition-transform duration-75 ease-linear' : 'transition-none') : 'transition-[transform,background-color] duration-300',
          )
        "
        :style="{ transform: `translateX(${(shown - 1) * 100}%)`, background: color }"
      />
    </div>
  </div>
</template>
