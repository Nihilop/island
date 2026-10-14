<script setup lang="ts">
// Déverrouillage : Windows Hello (si activé, demandé dès l'ouverture) ou mot de passe
// maître. Entrée = valider ; erreur = secousse + message.
import { onMounted, ref } from "vue";
import { PhFingerprint, PhLockSimple } from "@phosphor-icons/vue";
import TextField from "../../ui/TextField.vue";
import { Button } from "@/sdk/ui/button";
import { busy as viewBusy, helloUnlock, logout, unlock, vault, errorText } from "./store";
import { hello, helloError } from "../../notch/hello";
import { takeKeyboard } from "../../notch/state";
import { t } from "../../notch/i18n";

const password = ref("");
const busy = ref(false);
const error = ref<string | null>(null);
const shake = ref(false);

function fail(message: string | null) {
  error.value = message;
  if (!message) return;
  shake.value = false;
  requestAnimationFrame(() => (shake.value = true));
}

async function submit() {
  if (!password.value || busy.value || hello.prompting) return;
  busy.value = true;
  error.value = null;
  try {
    await unlock(password.value);
    password.value = "";
  } catch (e) {
    password.value = "";
    fail(errorText(t, String(e)));
  } finally {
    busy.value = false;
  }
}

async function withHello() {
  if (busy.value || hello.prompting) return;
  error.value = null;
  try {
    await helloUnlock();
  } catch (e) {
    const code = String(e);
    fail(code === "helloExpired" ? t("vault.err.helloExpired") : helloError(t, e));
    takeKeyboard();
  }
}

// Ouverture rapide : l'invite Hello s'ouvre d'elle-même.
onMounted(() => {
  if (vault.hello) void withHello();
});
</script>

<template>
  <div class="flex flex-col items-center justify-center gap-1.5">
    <span class="mb-0.5 grid size-10 place-items-center rounded-full bg-fill-2"><PhLockSimple :size="18" weight="fill" /></span>
    <div class="type-body font-medium">{{ t("vault.unlock.title") }}</div>
    <div class="type-caption text-label-3">{{ vault.email }}</div>
    <div class="mt-1 w-[260px]" :class="{ 'animate-shake': shake }">
      <TextField
        v-model="password"
        secret
        :autofocus="!vault.hello"
        autocomplete="current-password"
        :placeholder="t('vault.master')"
        @enter="submit"
      />
    </div>
    <div class="type-caption h-4" :class="error ? 'text-sys-red' : 'text-label-4'">
      {{ error ?? (vault.hello ? t("vault.unlock.helpHello") : t("vault.unlock.help")) }}
    </div>
    <div class="flex gap-2">
      <Button :disabled="busy || hello.prompting" :loading="viewBusy === 'logout'" @click="logout">{{ t("vault.logout") }}</Button>
      <Button v-if="vault.hello" :variant="password ? 'secondary' : 'default'" :disabled="busy" :loading="hello.prompting" @click="withHello">
        <PhFingerprint weight="bold" />{{ t("hello.unlock") }}
      </Button>
      <Button v-if="!vault.hello || password" variant="default" :disabled="!password || hello.prompting" :loading="busy" @click="submit">
        {{ busy ? t("vault.unlock.busy") : t("vault.unlock.action") }}
      </Button>
    </div>
  </div>
</template>
