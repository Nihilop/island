<script setup lang="ts">
// La modal = un "grand island" centré, désormais COMPOSANT de l'overlay (plus
// une fenêtre). Pilotée par le store `modalSpec`. Ouverture/fermeture par ressort.
import { ref, watch, nextTick, onMounted, onUnmounted, useTemplateRef } from "vue";
import UiNode from "./UiNode.vue";
import { modalSpec, closeModal, setHitRegion, type ModalSpec } from "../composables/overlay";

const BOX_W = 360;
const localSpec = ref<ModalSpec | null>(null);
const open = ref(false);
const values = ref<Record<string, unknown>>({});
const boxH = ref(220);
const innerEl = useTemplateRef('innerEl');
const headerEl = useTemplateRef('headerEl');

function measure(): number {
  return innerEl.value ? Math.ceil(innerEl.value.offsetHeight) : 0;
}

function headerMeasure(): number {
  return headerEl.value ? Math.ceil(headerEl.value.offsetHeight) / 1 : 0;
}

watch(modalSpec, async (s) => {
  if (s) {
    open.value = true;
    localSpec.value = s;
    values.value = {};
    await nextTick();
    boxH.value = measure() + headerMeasure();
    // Quand la modal est ouverte, tout l'écran capte les clics (backdrop). On se base
    // sur la taille ÉCRAN (pas window.innerWidth) car la fenêtre overlay vient de passer
    // en plein écran de façon ASYNC → innerWidth peut encore valoir la taille de la boîte.
    setHitRegion("modal", { x: 0, y: 0, w: window.screen.width, h: window.screen.height });
  } else {
    open.value = false;
    setHitRegion("modal", null);
    window.setTimeout(() => { localSpec.value = null; }, 360);
  }
});

function onChange(e: { id: string; value: unknown }) { values.value[e.id] = e.value; }
function onAction() { closeModal(); }
function onKey(e: KeyboardEvent) { if (e.key === "Escape" && modalSpec.value) closeModal(); }

onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div v-if="localSpec"
    class="absolute inset-0 z-[200] flex items-center justify-center transition-all duration-300 opacity-0"
    :class="{ 'bg-black/99 opacity-100': open }"
    @click="closeModal"
  >
  <Transition name="fade-in" mode="in-out">
    <div 
      v-if="open"
      class="box relative bg-background rounded-3xl shadow-2xl overflow-hidden border min-w-150 min-h-fit" 
      :style="{ width: BOX_W + 'px', height: boxH + 'px' }" 
      @click.stop
    >
      <div ref="innerEl" class="absolute inset-0 flex flex-col">
        <div v-if="localSpec.title" ref="headerEl" class="head">
          <div class="ttl">{{ localSpec.title }}</div>
          <div v-if="localSpec.subtitle" class="sub">{{ localSpec.subtitle }}</div>
        </div>
        <div class="flex-1 w-full overflow-x-hidden overflow-y-auto p-8">
          <component v-if="localSpec.component" :is="localSpec.component" />
          <UiNode v-else v-for="(n, i) in (localSpec.ui || [])" :key="i" :node="n" :values="values"
                  @change="onChange" @action="onAction" />
        </div>
      </div>
    </div>
    </Transition>
  </div>
</template>

<style scoped>
@reference "@/style.css";

.fade-in-enter-active,
.fade-in-leave-active {
  transition: all 500ms 800ms ease;
}

.fade-in-enter-from,
.fade-in-leave-to {
  @apply scale-50 opacity-0;
}

.box {
  transition: width 0.5s cubic-bezier(0.34, 1.42, 0.4, 1), height 0.5s cubic-bezier(0.34, 1.42, 0.4, 1),
    opacity 0.3s ease, transform 0.5s cubic-bezier(0.34, 1.45, 0.4, 1);
}

.head { padding: 16px 18px 4px; }
.ttl { font-size: 15px; font-weight: 500; }
.sub { font-size: 12px; color: rgba(255, 255, 255, 0.5); }
</style>
