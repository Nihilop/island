<script setup lang="ts">
// « Publier » de l'éditeur : envoie la scène au magasin (résumé, étiquettes ; notes de
// version pour une mise à jour). La scène est enregistrée d'abord (`prepare`), puis le Rust
// l'exporte et l'envoie avec le jeton du compte. Sans compte : la connexion GitHub sur place.
import { computed, ref, watch } from "vue";
import { PhCheck, PhStorefront, PhWarning } from "@phosphor-icons/vue";
import { Button } from "@/sdk/ui/button";
import { Input } from "@/sdk/ui/input";
import { Textarea } from "@/sdk/ui/textarea";
import { Spinner } from "@/sdk/ui/spinner";
import { Popover, PopoverContent, PopoverTrigger } from "@/sdk/ui/popover";
import StoreAccount from "../tools/wallpaper/StoreAccount.vue";
import { cleanTags, marketError, publish, published, session, storeStatus, watchAccount, type Publication } from "../market/api";
import { t } from "../notch/i18n";

const props = defineProps<{ scene: string; prepare: () => Promise<boolean> }>();

const open = ref(false);
const loading = ref(false);
const sending = ref(false);
const error = ref<string | null>(null);
const paused = ref<string | null>(null);
const existing = ref<Publication | null>(null);
const done = ref<Publication | null>(null);
const summary = ref("");
const tags = ref("");
const changelog = ref("");
const parsedTags = computed(() => cleanTags(tags.value));

watch(open, async (o) => {
  if (!o) return;
  watchAccount();
  loading.value = true;
  error.value = null;
  done.value = null;
  try {
    const [p, s] = await Promise.all([published(props.scene), storeStatus().catch(() => null)]);
    existing.value = p;
    paused.value = s && !s.publishing.open ? s.publishing.message || t("market.publish.pausedDefault") : null;
  } catch (e) {
    error.value = marketError(e);
  } finally {
    loading.value = false;
  }
});

async function send() {
  if (sending.value) return;
  sending.value = true;
  error.value = null;
  try {
    if (!(await props.prepare())) return;
    done.value = await publish(props.scene, { summary: summary.value, tags: parsedTags.value, changelog: changelog.value });
    existing.value = done.value;
    changelog.value = "";
  } catch (e) {
    error.value = marketError(e);
  } finally {
    sending.value = false;
  }
}
</script>

<template>
  <Popover v-model:open="open">
    <PopoverTrigger as-child>
      <Button size="sm"><PhStorefront />{{ t("market.publish.button") }}</Button>
    </PopoverTrigger>
    <PopoverContent align="end" :side-offset="6" class="flex w-80 flex-col gap-2.5">
      <div class="type-footnote font-semibold">
        {{ existing ? t("market.publish.titleUpdate", { v: existing.version }) : t("market.publish.titleNew") }}
      </div>

      <div v-if="loading" class="flex justify-center py-4"><Spinner /></div>

      <template v-else>
        <div v-if="paused" class="type-caption flex gap-1.5 rounded-[8px] bg-sys-orange/10 px-2 py-1.5 text-sys-orange">
          <PhWarning :size="12" weight="bold" class="mt-0.5 shrink-0" /><span>{{ t("market.publish.paused") }} {{ paused }}</span>
        </div>

        <template v-if="!session.account">
          <p class="type-caption text-label-3">{{ t("market.publish.signInFirst") }}</p>
          <StoreAccount />
        </template>

        <template v-else-if="done">
          <div class="type-caption flex items-center gap-1.5 rounded-[8px] bg-sys-green/10 px-2 py-1.5 text-sys-green">
            <PhCheck :size="12" weight="bold" />{{ t("market.publish.done", { v: done.version }) }}
          </div>
          <Button size="sm" variant="secondary" @click="open = false">{{ t("common.close") }}</Button>
        </template>

        <template v-else>
          <p class="type-caption text-label-3">{{ t("market.publish.as", { login: session.account.login }) }}</p>
          <label class="flex flex-col gap-1">
            <span class="type-caption text-label-2">{{ t("market.publish.summary") }}</span>
            <Input v-model="summary" maxlength="200" :placeholder="existing ? t('market.publish.keep') : t('market.publish.summaryHint')" />
          </label>
          <label class="flex flex-col gap-1">
            <span class="type-caption text-label-2">{{ t("market.publish.tags") }}</span>
            <Input v-model="tags" :placeholder="existing ? t('market.publish.keep') : t('market.publish.tagsHint')" />
            <span v-if="parsedTags.length" class="flex flex-wrap gap-1">
              <span v-for="tg in parsedTags" :key="tg" class="type-caption rounded-full bg-fill-2 px-1.5 text-label-2">{{ tg }}</span>
            </span>
          </label>
          <label v-if="existing" class="flex flex-col gap-1">
            <span class="type-caption text-label-2">{{ t("market.publish.changelog") }}</span>
            <Textarea v-model="changelog" rows="3" maxlength="2000" :placeholder="t('market.publish.changelogHint')" />
          </label>
          <p class="type-caption text-label-4">{{ t("market.publish.notice") }}</p>
          <Button size="sm" variant="default" :disabled="sending || !!paused" @click="send">
            <Spinner v-if="sending" />{{ sending ? t("market.publish.sending") : existing ? t("market.publish.sendUpdate") : t("market.publish.send") }}
          </Button>
        </template>

        <p v-if="error" class="type-caption text-sys-red">{{ error }}</p>
      </template>
    </PopoverContent>
  </Popover>
</template>
