<script setup lang="ts">
// Outil « Coffre » (560 × 324) : pont Bitwarden. Écrans par priorité : chargement →
// installation du CLI → connexion → déverrouillage → formulaires → liste.
import { onMounted, watch } from "vue";
import { presentation, setPresentation } from "../../notch/presentation";
import { PhEyeSlash } from "@phosphor-icons/vue";
import Empty from "../../ui/Empty.vue";
import { Button } from "@/sdk/ui/button";
import { Skeleton } from "@/sdk/ui/skeleton";
import { PhArrowsClockwise, PhLockSimple, PhLockSimpleOpen, PhPlus, PhWarningCircle } from "@phosphor-icons/vue";
import IconButton from "../../ui/IconButton.vue";
import { busy, editing, lock, refreshStatus, screen, segment, sync, toast, vault, errorText } from "./store";
import { PhCircleNotch } from "@phosphor-icons/vue";
import { t } from "../../notch/i18n";
import InstallScreen from "./InstallScreen.vue";
import LoginScreen from "./LoginScreen.vue";
import UnlockScreen from "./UnlockScreen.vue";
import VaultList from "./VaultList.vue";
import PasswordForm from "./PasswordForm.vue";
import TotpForm from "./TotpForm.vue";

onMounted(refreshStatus);

// Mode présentation : le coffre se verrouille et reste masqué jusqu'à la fin.
watch(
  presentation,
  (on) => {
    if (on && vault.state === "unlocked") void lock();
  },
  { immediate: true },
);

function add() {
  editing.value = null;
  screen.value = segment.value === "totp" ? "totpForm" : "passwordForm";
}
</script>

<template>
  <div class="relative h-full px-4 [&>*:not([data-toast])]:h-full">
    <Teleport defer to="#notch-header-actions">
      <template v-if="vault.state === 'unlocked' && screen === 'list' && !presentation">
        <IconButton :icon="PhArrowsClockwise" :label="t('vault.sync')" :loading="busy === 'sync'" :disabled="!!busy" @click="sync" />
        <IconButton :icon="PhLockSimpleOpen" active :label="t('vault.lock')" :loading="busy === 'lock'" :disabled="!!busy" @click="lock" />
        <IconButton :icon="PhPlus" :label="t('vault.add')" :disabled="!!busy" @click="add" />
      </template>
      <IconButton v-else-if="vault.state === 'locked'" :icon="PhLockSimple" :label="t('vault.locked')" />
    </Teleport>

    <Transition
      enter-active-class="transition-[opacity,filter] duration-240 ease-out delay-50"
      enter-from-class="opacity-0 blur-[4px]"
      leave-active-class="transition-[opacity,filter] duration-120 ease-out"
      leave-to-class="opacity-0 blur-[4px]"
      mode="out-in"
    >
      <Empty v-if="presentation" key="presentation" :icon="PhEyeSlash" :title="t('vault.presentation.title')" :text="t('vault.presentation.text')">
        <Button @click="setPresentation(false)">{{ t("vault.presentation.exit") }}</Button>
      </Empty>
      <div v-else-if="screen === 'loading'" key="loading" class="flex flex-col gap-2 p-4">
        <Skeleton v-for="i in 3" :key="i" class="h-11 rounded-[10px] animate-[pulse_1.4s_ease-in-out_infinite]" />
      </div>
      <InstallScreen v-else-if="screen === 'install'" key="install" />
      <LoginScreen v-else-if="screen === 'login'" key="login" />
      <UnlockScreen v-else-if="screen === 'unlock'" key="unlock" />
      <PasswordForm v-else-if="screen === 'passwordForm'" key="pw" />
      <TotpForm v-else-if="screen === 'totpForm'" key="totp" />
      <VaultList v-else key="list" />
    </Transition>

    <Transition
      enter-active-class="transition-opacity duration-160 ease-out"
      enter-from-class="opacity-0"
      leave-active-class="transition-opacity duration-160 ease-out"
      leave-to-class="opacity-0"
    >
      <div v-if="busy" class="absolute inset-0 z-[2] flex flex-col items-center justify-center gap-2 bg-scrim/60 backdrop-blur-[2px]">
        <PhCircleNotch :size="18" weight="bold" class="animate-[spin_0.9s_linear_infinite]" />
        <span class="type-footnote text-label-2">{{ t(`vault.busy.${busy}`) }}</span>
      </div>
    </Transition>

    <Transition
      enter-active-class="transition-[opacity,translate] duration-160 ease-out"
      enter-from-class="opacity-0 translate-y-1"
      leave-active-class="transition-[opacity,translate] duration-160 ease-out"
      leave-to-class="opacity-0 translate-y-1"
    >
      <div
        v-if="toast"
        data-toast
        class="type-footnote absolute bottom-2 left-1/2 flex max-w-[calc(100%-32px)] -translate-x-1/2 items-center gap-1.5 overflow-hidden rounded-full bg-elevated px-3 py-1.5 text-ellipsis whitespace-nowrap shadow-[0_6px_18px_rgb(0_0_0/0.45)]"
      >
        <PhWarningCircle :size="14" weight="fill" class="text-sys-orange" />
        {{ errorText(t, toast) }}
      </div>
    </Transition>
  </div>
</template>
