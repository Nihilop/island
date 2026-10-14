<script setup lang="ts">
// Formulaire 2FA : coller un QR (Ctrl+V hors champ texte, ou bouton « Coller ») ou saisir
// la clé (base32 ou otpauth://). Le contenu du QR reste dans Rust ; seuls émetteur et
// compte remontent. Ajouté dans Bitwarden comme un login avec champ TOTP.
import { clearDrafts, useDraft } from "../../stores/drafts";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { PhCheckCircle, PhQrCode } from "@phosphor-icons/vue";
import Group from "../../ui/Group.vue";
import Row from "../../ui/Row.vue";
import TextField from "../../ui/TextField.vue";
import { Button } from "@/sdk/ui/button";
import { guarded, screen, errorText } from "./store";
import { t } from "../../notch/i18n";

const scan = ref<{ issuer: string | null; account: string | null } | null>(null);
const scanError = ref<string | null>(null);
const name = useDraft("vault.totp.name", () => "");
const secret = useDraft("vault.totp.secret", () => "");
const account = useDraft("vault.totp.account", () => "");
const busy = ref(false);

async function paste() {
  scanError.value = null;
  try {
    scan.value = await invoke("vault_scan_qr");
    if (!name.value && scan.value?.issuer) name.value = scan.value.issuer;
  } catch (e) {
    scanError.value = String(e);
  }
}

function skip() {
  scan.value = null;
  invoke("vault_clear_scan").catch(() => {});
}

function onKey(e: KeyboardEvent) {
  const inField = (e.target as HTMLElement | null)?.closest("input, textarea");
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "v" && !inField) {
    e.preventDefault();
    void paste();
  }
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  invoke("vault_clear_scan").catch(() => {});
});

const canSave = computed(() => !!name.value.trim() && (!!scan.value || !!secret.value.trim()) && !busy.value);

async function save() {
  if (!canSave.value) return;
  busy.value = true;
  const ok = await guarded(() =>
    invoke("vault_add_totp", {
      name: name.value,
      secret: scan.value ? null : secret.value,
      account: account.value || null,
    }).then(() => true),
  );
  busy.value = false;
  if (ok) {
    clearDrafts("vault.totp.");
    screen.value = "list";
  }
}
</script>

<template>
  <div class="flex flex-col gap-2 overflow-y-auto">
    <div class="flex h-14 items-center gap-2.5 rounded-md bg-fill-1 px-3">
      <span
        class="grid size-8 place-items-center rounded-full bg-fill-2 transition-[color,background-color] duration-200 ease-out"
        :class="scan ? 'text-sys-green' : 'text-label-2'"
      >
        <PhCheckCircle v-if="scan" :size="18" weight="fill" />
        <PhQrCode v-else :size="18" weight="bold" />
      </span>
      <div class="min-w-0 flex-1">
        <div class="type-body">{{ scan ? t("vault.qr.ok") : t("vault.qr.title") }}</div>
        <div class="type-caption truncate" :class="scanError ? 'text-sys-red' : 'text-label-3'">
          <template v-if="scanError">{{ errorText(t, scanError) }}</template>
          <template v-else-if="scan">{{ [scan.issuer, scan.account].filter(Boolean).join(" · ") }}</template>
          <template v-else>{{ t("vault.qr.help") }}</template>
        </div>
      </div>
      <Button v-if="scan" @click="skip">{{ t("vault.qr.skip") }}</Button>
      <Button v-else @click="paste">{{ t("vault.qr.paste") }}</Button>
    </div>

    <Group>
      <Row :label="t('vault.form.name')" :help="scan?.issuer ? t('vault.qr.suggestion', { issuer: scan.issuer }) : undefined">
        <TextField v-model="name" :placeholder="t('vault.form.namePh')" class="w-[300px]" @enter="save" />
      </Row>
      <template v-if="!scan">
        <Row :label="t('vault.form.key')">
          <TextField v-model="secret" secret :placeholder="t('vault.form.keyPh')" class="w-[300px]" @enter="save" />
        </Row>
        <Row :label="t('vault.form.account')">
          <TextField v-model="account" class="w-[300px]" @enter="save" />
        </Row>
      </template>
    </Group>

    <div class="flex justify-end gap-2">
      <Button @click="clearDrafts('vault.totp.'); screen = 'list'">{{ t("common.cancel") }}</Button>
      <Button variant="default" :disabled="!canSave && !busy" :loading="busy" @click="save">{{ t("vault.add") }}</Button>
    </div>
  </div>
</template>
