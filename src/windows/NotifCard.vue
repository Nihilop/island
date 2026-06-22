<script setup lang="ts">
// Carte de notification — SOURCE UNIQUE du design des notifs (pile + centre).
// L'île l'utilise dans les deux TransitionGroup (nstack / nlist). Tout le rendu et
// le style d'une notif se modifient ICI. La carte n'a qu'un seul élément racine
// → les classes de transition du TransitionGroup parent s'y appliquent.
import type { Notif } from "../composables/notifications";

defineProps<{ notif: Notif }>();
// `close` = croix dédiée (fermeture explicite, distincte du clic-action sur la carte).
const emit = defineEmits<{ close: [] }>();
</script>

<template>
  <div class="group relative flex h-14 flex-none items-center gap-2.5 rounded-2xl px-2.5 py-1 transition hover:bg-foreground/[0.06]">
    <!-- icône injectée en v-html → on dimensionne le <svg> enfant via un variant arbitraire -->
    <div
      class="grid h-9 w-9 flex-none place-items-center rounded-[10px] bg-foreground/10 text-foreground [&_svg]:h-[18px] [&_svg]:w-[18px]"
      :style="notif.color ? { background: notif.color + '26', color: notif.color } : {}"
      v-html="notif.icon"
    ></div>
    <div class="min-w-0 flex-1">
      <div class="truncate text-[12.5px] font-semibold leading-tight">{{ notif.title }}</div>
      <div v-if="notif.body" class="mt-px truncate text-[11px] leading-tight text-muted-foreground">{{ notif.body }}</div>
    </div>
    <span v-if="notif.source" class="flex-none self-end pr-0.5 text-[10px] text-muted-foreground">{{ notif.source }}</span>
    <!-- Croix VISIBLE (cercle) en haut à droite : fermeture individuelle, ≠ clic-action. -->
    <button
      class="absolute top-1.5 right-2 grid size-[18px] place-items-center rounded-full bg-foreground/10 text-muted-foreground opacity-70 transition hover:bg-foreground/20 hover:text-foreground hover:opacity-100"
      aria-label="Fermer" @click.stop="emit('close')"
    >
      <svg viewBox="0 0 24 24" class="h-3 w-3"><path fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" d="M6 6l12 12M18 6 6 18" /></svg>
    </button>
  </div>
</template>
