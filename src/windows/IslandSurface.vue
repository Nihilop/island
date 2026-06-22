<script setup lang="ts">
// Surface visuelle (chrome) de l'île = le SEUL endroit qui porte le look du thème :
// forme des coins + congés inversés (inverted radius). Le contenu (idle/launcher/…)
// reste dans Island et s'anime à l'identique. Ajouter/changer un thème = ici + le
// composable islandTheme. Hérite de --r (rayon courant) et --background du parent.
import { computed } from "vue";
import { theme } from "../composables/islandTheme";

// `open` = l'île est déployée (vs au repos) → le surface connaît l'état (hooks .is-open/.is-idle
// pour ajuster le chrome par état si besoin ; la TAILLE vient déjà de --r/islandDims).
const props = defineProps<{ open?: boolean }>();
const topbar = computed(() => theme.value.anchor === "topbar");
</script>

<template>
  <div
    class="absolute inset-0 bg-background shadow-[0_8px_24px_rgba(0,0,0,0.4),inset_0_0_0_0.5px_rgba(255,255,255,0.07)]"
    :class="[topbar ? 'topbar rounded-b-(--r) shadow-none!' : 'rounded-(--r)', props.open ? 'is-open' : 'is-idle']"
  ></div>
</template>

<style scoped>
/* Congés CONCAVES (inverted radius) : relient les flancs au bord haut en thème topbar.
   Pseudo-éléments masqués → non exprimable en classe. */
.topbar::before,
.topbar::after {
  content: ""; position: absolute; top: 0; width: var(--r); height: var(--r);
  background: var(--background);
}
.topbar::before {
  left: calc(-1 * var(--r));
  -webkit-mask: radial-gradient(circle at bottom left, transparent var(--r), #000 var(--r));
  mask: radial-gradient(circle at bottom left, transparent var(--r), #000 var(--r));
}
.topbar::after {
  right: calc(-1 * var(--r));
  -webkit-mask: radial-gradient(circle at bottom right, transparent var(--r), #000 var(--r));
  mask: radial-gradient(circle at bottom right, transparent var(--r), #000 var(--r));
}
</style>
