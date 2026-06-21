<script setup lang="ts">
// Carte de notification — SOURCE UNIQUE du design des notifs (pile + centre).
// L'île l'utilise dans les deux TransitionGroup (nstack / nlist). Tout le rendu et
// le style d'une notif se modifient ICI. La carte n'a qu'un seul élément racine
// → les classes de transition du TransitionGroup parent s'y appliquent.
import type { Notif } from "../composables/notifications";

defineProps<{ notif: Notif }>();
</script>

<template>
  <div class="flex h-14 flex-none items-center gap-2.5 rounded-2xl px-2.5 py-1 transition hover:bg-foreground/[0.06]">
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
    <div v-if="notif.source" class="flex-none self-start text-[10px] text-muted-foreground">{{ notif.source }}</div>
  </div>
</template>
