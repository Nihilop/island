<script setup lang="ts">
// Fond de l'outil Météo, derrière tout le notch ouvert (en-tête et dock compris) : le ciel
// de la journée (cf. sky.ts) rendu en WebGL (skyRenderer.ts). Fondu vers la couleur du
// notch en haut (raccord avec les « oreilles ») et en bas (lisibilité du dock) ; voile
// derrière la température. Pause quand la fenêtre est masquée ; animations réduites
// respectées (image fixe). Sans WebGL : le dégradé seul.
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { flashRequest, sky } from "./sky";
import { SkyRenderer, cssGradient, targetFor } from "./skyRenderer";

const canvas = ref<HTMLCanvasElement>();
const ready = ref(false);
let renderer: SkyRenderer | null = null;

// Thème effectif (classe `light` posée sur <html> par notch/theme.ts).
const light = ref(document.documentElement.classList.contains("light"));
const themeObserver = new MutationObserver(() => (light.value = document.documentElement.classList.contains("light")));
themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });

const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
const params = computed(() => (sky.value ? targetFor(sky.value, light.value) : null));
const fallback = computed(() => (params.value ? cssGradient(params.value) : undefined));

watch(params, (p) => p && renderer?.set(p));
watch(flashRequest, () => renderer?.strike());

function onVisibility() {
  if (document.hidden) renderer?.stop();
  else renderer?.start();
}

function boot() {
  renderer?.dispose();
  renderer = null;
  if (!canvas.value) return;
  try {
    renderer = new SkyRenderer(canvas.value, { still: reduced.matches });
    if (params.value) renderer.set(params.value, true);
    renderer.start();
    // Une image dessinée avant d'apparaître : pas de flash noir.
    requestAnimationFrame(() => (ready.value = true));
  } catch (e) {
    console.warn("[weather] ciel WebGL indisponible", e);
  }
}

onMounted(() => {
  boot();
  document.addEventListener("visibilitychange", onVisibility);
  reduced.addEventListener("change", boot);
});

onBeforeUnmount(() => {
  document.removeEventListener("visibilitychange", onVisibility);
  reduced.removeEventListener("change", boot);
  themeObserver.disconnect();
  renderer?.dispose();
});
</script>

<template>
  <div
    v-show="params"
    aria-hidden="true"
    class="pointer-events-none overflow-hidden transition-[background] duration-1000 [mask-image:linear-gradient(to_bottom,transparent_0,#000_95px,#000_calc(100%-74px),rgb(0_0_0/0.22)_calc(100%-14px),rgb(0_0_0/0.22))]"
    :style="{ background: fallback }"
  >
    <canvas ref="canvas" class="absolute inset-0 size-full transition-opacity duration-700 ease-out" :class="ready ? 'opacity-100' : 'opacity-0'" />
    <!-- Voile de lecture, couleur du notch (sombre ou clair selon le thème) : une teinte
         générale qui calme les ciels lumineux (soleil, couvert clair, neige), renforcée
         derrière le bloc « maintenant » (haut gauche), les détails (haut droite) et la bande
         des jours. Le ciel reste lisible en arrière-plan, le texte passe devant. -->
    <div class="absolute inset-0 bg-[color-mix(in_srgb,var(--notch)_20%,transparent)]" />
    <div
      class="absolute inset-0 [background:radial-gradient(ellipse_48%_58%_at_13%_42%,color-mix(in_srgb,var(--notch)_58%,transparent),transparent_76%)]"
    />
    <div
      class="absolute inset-0 [background:radial-gradient(ellipse_40%_46%_at_88%_38%,color-mix(in_srgb,var(--notch)_42%,transparent),transparent_78%)]"
    />
    <div class="absolute inset-0 [background:linear-gradient(to_top,color-mix(in_srgb,var(--notch)_42%,transparent)_8%,transparent_58%)]" />
  </div>
</template>
