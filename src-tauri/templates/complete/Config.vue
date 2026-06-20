<script setup lang="ts">
// Surface "config" de __EXT_NAME__ : ouverte en modal depuis la view.
import { Switch } from "@island/sdk";
import { state, setNotify, reset } from "./store";
</script>

<template>
  <div class="flex flex-col gap-5 text-foreground">
    <label class="flex cursor-pointer items-center justify-between gap-4">
      <div>
        <div class="text-[13.5px]">Notifier à chaque incrément</div>
        <div class="mt-0.5 text-[12px] text-muted-foreground">Envoie une bannière sur l'île</div>
      </div>
      <Switch :model-value="state.notifyOnBump" @update:model-value="setNotify" />
    </label>

    <button class="rounded-lg border px-3 py-2 text-[13px] transition hover:bg-foreground/10" @click="reset">
      Réinitialiser le compteur ({{ state.count }})
    </button>
  </div>
</template>
