<script setup lang="ts">
// Compte du magasin : « Se connecter avec GitHub » (device flow : un code à saisir sur
// github.com, ouvert dans le navigateur), puis l'avatar et la déconnexion. Le panneau se
// referme quand le navigateur prend le focus : c'est donc le RUST qui copie le code, le
// notifie (notification système) et attend la validation ; ce composant ne fait qu'afficher
// l'état (`market_login_state`) et la fin (`market://login`). Le jeton reste dans le
// trousseau du système.
import { onBeforeUnmount, onMounted, ref } from "vue";
import { PhCopy, PhGithubLogo, PhSignOut } from "@phosphor-icons/vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { Button } from "@/sdk/ui/button";
import { Spinner } from "@/sdk/ui/spinner";
import { locale } from "@/sdk/i18n";
import IconButton from "../../ui/IconButton.vue";
import { loginCancel, loginStart, loginState, logout, marketError, refreshAccount, session, watchAccount, type LoginStart } from "../../market/api";
import { t } from "../../notch/i18n";

const flow = ref<LoginStart | null>(null);
const busy = ref<"start" | "logout" | null>(null);
const error = ref<string | null>(null);
const copied = ref(false);

async function start() {
  if (busy.value) return;
  busy.value = "start";
  error.value = null;
  try {
    flow.value = await loginStart(locale.value);
    void invoke("open_url", { url: flow.value.verificationUri });
  } catch (e) {
    error.value = marketError(e);
  } finally {
    busy.value = null;
  }
}

function cancel() {
  flow.value = null;
  void loginCancel();
}

async function copy() {
  if (!flow.value) return;
  await navigator.clipboard.writeText(flow.value.userCode).catch(() => {});
  copied.value = true;
  window.setTimeout(() => (copied.value = false), 1500);
}

async function signOut() {
  if (busy.value) return;
  busy.value = "logout";
  try {
    await logout();
    await refreshAccount();
  } catch (e) {
    error.value = marketError(e);
  } finally {
    busy.value = null;
  }
}

let unlisten: UnlistenFn | undefined;
onMounted(async () => {
  watchAccount();
  unlisten = await listen<{ error: string | null }>("market://login", (e) => {
    flow.value = null;
    error.value = e.payload.error ? marketError(e.payload.error) : null;
    void refreshAccount().catch(() => {});
  });
  // Panneau rouvert pendant une connexion : on reprend l'affichage du code.
  flow.value = await loginState().catch(() => null);
});
onBeforeUnmount(() => unlisten?.());
</script>

<template>
  <div class="flex flex-col gap-1.5">
    <!-- Connexion en cours : le code à saisir sur GitHub -->
    <div v-if="flow" class="flex items-center gap-3 rounded-[10px] bg-fill-1 px-3 py-2">
      <div class="min-w-0 flex-1">
        <div class="type-caption text-label-3">{{ t("market.login.enterCode") }}</div>
        <div class="flex items-center gap-1.5">
          <span class="type-title font-semibold tracking-widest tabular-nums select-all">{{ flow.userCode }}</span>
          <IconButton :icon="PhCopy" :label="copied ? t('market.login.copied') : t('market.login.copy')" @click="copy" />
        </div>
        <div class="type-caption flex items-center gap-1.5 text-label-3"><Spinner class="size-3" />{{ t("market.login.waiting") }}</div>
      </div>
      <div class="flex shrink-0 flex-col gap-1">
        <Button size="sm" @click="invoke('open_url', { url: flow.verificationUri })"><PhGithubLogo />{{ t("market.login.openGithub") }}</Button>
        <Button size="sm" variant="ghost" @click="cancel">{{ t("market.login.cancel") }}</Button>
      </div>
    </div>

    <!-- Connecté -->
    <div v-else-if="session.account" class="flex items-center justify-end gap-1.5">
      <img v-if="session.account.avatarUrl" :src="session.account.avatarUrl" alt="" class="size-5 rounded-full" />
      <span class="type-caption text-label-2">{{ session.account.login }}</span>
      <IconButton :icon="PhSignOut" :label="t('market.login.logout')" :loading="busy === 'logout'" :disabled="busy === 'logout'" @click="signOut" />
    </div>

    <!-- Anonyme -->
    <div v-else-if="session.loaded" class="flex justify-end">
      <Button size="sm" variant="secondary" :disabled="busy === 'start'" @click="start">
        <Spinner v-if="busy === 'start'" /><PhGithubLogo v-else />{{ t("market.login.signIn") }}
      </Button>
    </div>

    <p v-if="error" class="type-caption text-right text-sys-red">{{ error }}</p>
  </div>
</template>
