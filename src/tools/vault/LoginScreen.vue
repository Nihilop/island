<script setup lang="ts">
// Connexion Bitwarden : serveur, e-mail + mot de passe maître. Un champ de code n'apparaît
// QUE si Bitwarden en demande un (2FA configurée, ou vérification de nouvel appareil
// envoyée par e-mail). La saisie vit dans le store : fermer le notch ne perd rien, et une
// connexion en attente de code reprend à la même étape. Pendant un échange avec
// Bitwarden : indicateur d'attente, champs figés, rien n'est vidé avant le résultat.
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhCircleNotch, PhEnvelopeSimple, PhShieldCheck } from "@phosphor-icons/vue";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import Segmented from "../../ui/Segmented.vue";
import TextField from "../../ui/TextField.vue";
import { Button } from "@/sdk/ui/button";
import { cancelLogin, errorText, login, loginDraft, submitCode, vault } from "./store";
import { t } from "../../notch/i18n";

const SERVERS = { com: "https://vault.bitwarden.com", eu: "https://vault.bitwarden.eu" } as const;

const code = ref("");
const method = ref(0);
const busy = ref(false);
const shake = ref(false);
/** Message sous le champ : erreur (rouge) ou info (code renvoyé). */
const notice = ref<{ text: string; error: boolean } | null>(null);

const step = computed<"credentials" | "twoFactor" | "newDevice" | "method">(() =>
  vault.pending ?? (loginDraft.chooseMethod ? "method" : "credentials"),
);

const canSubmit = computed(() => {
  if (busy.value) return false;
  if (step.value === "credentials" || step.value === "method")
    return !!loginDraft.email.trim() && !!loginDraft.password;
  return !!code.value.trim();
});

function bump() {
  shake.value = false;
  requestAnimationFrame(() => (shake.value = true));
}

async function submit() {
  if (!canSubmit.value) return;
  busy.value = true;
  notice.value = null;
  try {
    if (step.value === "credentials") {
      const url = loginDraft.server === "custom" ? loginDraft.customUrl.trim() : SERVERS[loginDraft.server];
      await invoke("vault_set_server", { url }).catch(() => {});
      await login();
    } else if (step.value === "method") {
      await login(method.value);
    } else {
      const r = await submitCode(code.value);
      code.value = "";
      if (r.retry) {
        bump();
        notice.value = { text: t("vault.code.wrong"), error: true };
      }
    }
  } catch (e) {
    bump();
    notice.value = { text: errorText(t, String(e)), error: true };
  } finally {
    busy.value = false;
  }
}

async function cancel() {
  notice.value = null;
  code.value = "";
  await cancelLogin();
}
</script>

<template>
  <div class="flex flex-col gap-2.5 overflow-y-auto pt-0.5" :class="{ 'animate-shake': shake }">
    <div class="flex items-center gap-2.5">
      <span class="grid size-8 shrink-0 place-items-center rounded-full bg-fill-2 text-sys-link">
        <PhEnvelopeSimple v-if="step === 'newDevice'" :size="18" weight="fill" />
        <PhShieldCheck v-else :size="18" weight="fill" />
      </span>
      <div class="min-w-0">
        <div class="type-body font-medium">
          {{
            step === "credentials"
              ? t("vault.login.title")
              : step === "method"
                ? t("vault.code.method")
                : t(`vault.code.${step}`)
          }}
        </div>
        <div class="type-caption text-label-3">
          {{
            step === "credentials"
              ? t("vault.login.help")
              : step === "newDevice"
                ? t("vault.code.newDeviceHelp")
                : step === "twoFactor"
                  ? t("vault.code.twoFactorHelp")
                  : loginDraft.email
          }}
        </div>
      </div>
    </div>

    <fieldset :disabled="busy" class="contents disabled:opacity-55 disabled:transition-opacity disabled:duration-160 disabled:ease-out">
      <Group v-if="step === 'credentials'">
        <Row :label="t('vault.server')">
          <Segmented
            v-model="loginDraft.server"
            size="sm"
            :options="[
              { value: 'com', label: 'bitwarden.com' },
              { value: 'eu', label: 'bitwarden.eu' },
              { value: 'custom', label: t('vault.server.custom') },
            ]"
          />
        </Row>
        <Row v-if="loginDraft.server === 'custom'" :label="t('vault.server.url')">
          <TextField v-model="loginDraft.customUrl" placeholder="https://vault.example.com" class="w-[260px]" />
        </Row>
        <Row :label="t('vault.email')">
          <TextField v-model="loginDraft.email" type="email" autocomplete="username" autofocus class="w-[260px]" @enter="submit" />
        </Row>
        <Row :label="t('vault.master')">
          <TextField v-model="loginDraft.password" secret autocomplete="current-password" class="w-[260px]" @enter="submit" />
        </Row>
      </Group>

      <Group v-else-if="step === 'method'">
        <Row :label="t('vault.2fa.method')">
          <Segmented
            v-model="method"
            size="sm"
            :options="[
              { value: 0, label: t('vault.2fa.app') },
              { value: 1, label: t('vault.2fa.email') },
              { value: 3, label: 'YubiKey' },
            ]"
          />
        </Row>
      </Group>

      <div v-else class="flex justify-center py-3">
        <TextField
          v-model="code"
          inputmode="numeric"
          autocomplete="one-time-code"
          autofocus
          :placeholder="t('vault.2fa.code')"
          class="w-[200px]"
          @enter="submit"
        />
      </div>
    </fieldset>

    <div class="flex items-center gap-2">
      <div class="type-caption min-w-0 flex-1">
        <span v-if="busy" class="flex items-center gap-1.5 text-label-2">
          <PhCircleNotch :size="13" weight="bold" class="animate-[spin_0.9s_linear_infinite]" />
          {{ step === "twoFactor" || step === "newDevice" ? t("vault.wait.code") : t("vault.wait.login") }}
        </span>
        <span v-else-if="notice" :class="notice.error ? 'text-sys-red' : 'text-label-2'">{{ notice.text }}</span>
      </div>
      <Button v-if="step !== 'credentials'" :disabled="busy" @click="cancel">{{ t("common.cancel") }}</Button>
      <Button variant="default" :disabled="!canSubmit && !busy" :loading="busy" @click="submit">
        {{ step === "credentials" || step === "method" ? t("vault.login.action") : t("vault.code.action") }}
      </Button>
    </div>
  </div>
</template>
