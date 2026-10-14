<script setup lang="ts">
// Liste du coffre : segmented Mots de passe | 2FA + recherche (nom, identifiant), tri
// alphabétique, lignes 44 px en cascade.
//   - mot de passe : au survol copier l'identifiant, révéler (un seul à la fois, masqué
//     après 15 s), modifier ; toujours visible : copier le mot de passe ;
//   - 2FA : toute la ligne copie le code, affiché en deux groupes avec anneau de compte à
//     rebours.
import { useDraft } from "../../stores/drafts";
import { computed, onBeforeUnmount, ref } from "vue";
import {
  PhCopy,
  PhEye,
  PhEyeSlash,
  PhKey,
  PhMagnifyingGlass,
  PhPencilSimple,
  PhQrCode,
  PhUser,
  PhArrowSquareOut,
} from "@phosphor-icons/vue";
import Segmented from "../../ui/Segmented.vue";
import Empty from "../../ui/Empty.vue";
import { Button } from "@/sdk/ui/button";
import { Input } from "@/sdk/ui/input";
import { Item, ItemActions, ItemContent, ItemDescription, ItemMedia, ItemTitle } from "@/sdk/ui/item";
import CountdownRing from "../../ui/CountdownRing.vue";
import { copied, copy, editing, reveal, screen, segment, vault, type Meta } from "./store";
import { takeKeyboard } from "../../notch/state";
import { t } from "../../notch/i18n";

const query = useDraft("vault.query", () => "");
const revealed = ref<{ id: string; value: string } | null>(null);
let revealTimer: number | undefined;

const passwords = computed(() => vault.items.filter((i) => i.hasPassword));
const totps = computed(() => vault.items.filter((i) => i.hasTotp));

const rows = computed(() => {
  const list = segment.value === "totp" ? totps.value : passwords.value;
  const q = query.value.trim().toLowerCase();
  if (!q) return list;
  return list.filter((i) => i.name.toLowerCase().includes(q) || (i.username ?? "").toLowerCase().includes(q));
});

async function toggleReveal(item: Meta) {
  window.clearTimeout(revealTimer);
  if (revealed.value?.id === item.id) {
    revealed.value = null;
    return;
  }
  const value = await reveal(item.id);
  if (value === undefined) return;
  revealed.value = { id: item.id, value };
  revealTimer = window.setTimeout(() => (revealed.value = null), 15_000);
}

function edit(item: Meta) {
  editing.value = item;
  screen.value = "passwordForm";
}

function add() {
  editing.value = null;
  screen.value = segment.value === "totp" ? "totpForm" : "passwordForm";
}

function openUri(uri: string) {
  if (/^https?:\/\//i.test(uri)) void import("@tauri-apps/api/core").then(({ invoke }) => invoke("open_url", { url: uri }));
}

function split(code: string) {
  const h = Math.ceil(code.length / 2);
  return `${code.slice(0, h)} ${code.slice(h)}`;
}

const copiedLabel = computed(() =>
  copied.what === "username" ? t("vault.copied.username") : copied.what === "password" ? t("vault.copied.password") : t("vault.copied.code"),
);

onBeforeUnmount(() => window.clearTimeout(revealTimer));
</script>

<template>
  <div class="flex flex-col gap-2">
    <div class="flex items-center gap-2">
      <Segmented
        v-model="segment"
        size="sm"
        :options="[
          { value: 'passwords', label: `${t('vault.passwords')} ${passwords.length || ''}`.trim() },
          { value: 'totp', label: `2FA ${totps.length || ''}`.trim() },
        ]"
      />
      <label
        class="flex h-7 flex-1 items-center gap-1.5 rounded-full bg-fill-1 px-2.5 transition-colors duration-160 ease-out focus-within:bg-fill-2"
        data-no-drag
      >
        <PhMagnifyingGlass :size="13" weight="bold" class="text-label-3" />
        <Input
          v-model="query"
          class="type-footnote h-auto flex-1 rounded-none bg-transparent px-0 text-label focus-visible:ring-0"
          :placeholder="t('vault.search')"
          spellcheck="false"
          @focus="takeKeyboard"
        />
      </label>
    </div>

    <div
      class="min-h-0 flex-1 overflow-y-auto pb-5 [mask-image:linear-gradient(to_bottom,#000_calc(100%-20px),transparent)] [scrollbar-color:var(--fill-3)_transparent] [scrollbar-width:thin]"
    >
      <Empty
        v-if="!vault.items.length || (!rows.length && !query)"
        :icon="segment === 'totp' ? PhQrCode : PhKey"
        :title="segment === 'totp' ? t('vault.empty.totp') : t('vault.empty.passwords')"
        :text="segment === 'totp' ? t('vault.empty.totpText') : t('vault.empty.passwordsText')"
      >
        <Button @click="add">{{ t("vault.add") }}</Button>
      </Empty>
      <Empty v-else-if="!rows.length" :icon="PhMagnifyingGlass" :title="t('vault.empty.search')" />

      <TransitionGroup
        v-else
        tag="div"
        appear
        enter-active-class="transition-[opacity,translate] duration-240 ease-out delay-(--delay)"
        enter-from-class="opacity-0 -translate-y-1"
        leave-active-class="hidden"
      >
        <!-- Mots de passe -->
        <template v-if="segment === 'passwords'">
          <Item
            v-for="(it, i) in rows"
            :key="it.id"
            class="duration-160 ease-out"
            :style="{ '--delay': `${50 + Math.min(i, 7) * 35}ms` }"
          >
            <ItemMedia variant="icon">{{ (it.name[0] ?? "?").toUpperCase() }}</ItemMedia>
            <ItemContent>
              <ItemTitle class="type-body block truncate">{{ it.name }}</ItemTitle>
              <ItemDescription class="type-caption truncate" :class="copied.id === it.id ? 'text-sys-green' : 'text-label-3'">
                <template v-if="copied.id === it.id">{{ copiedLabel }}</template>
                <span
                  v-else-if="revealed?.id === it.id"
                  class="tabular select-text text-label-2"
                  @copy.prevent="copy(it.id, 'password')"
                  @cut.prevent
                >{{ revealed.value }}</span>
                <template v-else>{{ it.username ?? t("vault.noUsername") }}</template>
              </ItemDescription>
            </ItemContent>
            <ItemActions class="opacity-0 transition-opacity duration-160 ease-out group-focus-within/item:opacity-100 group-hover/item:opacity-100">
              <Button v-if="it.uri" variant="ghost" size="icon" class="hover:text-label" :title="t('vault.openSite')" @click="openUri(it.uri)">
                <PhArrowSquareOut :size="14" weight="bold" />
              </Button>
              <Button v-if="it.username" variant="ghost" size="icon" class="hover:text-label" :title="t('vault.copyUsername')" @click="copy(it.id, 'username')">
                <PhUser :size="14" weight="bold" />
              </Button>
              <Button variant="ghost" size="icon" class="hover:text-label" :title="t('vault.reveal')" @click="toggleReveal(it)">
                <PhEyeSlash v-if="revealed?.id === it.id" :size="14" weight="bold" />
                <PhEye v-else :size="14" weight="bold" />
              </Button>
              <Button variant="ghost" size="icon" class="hover:text-label" :title="t('vault.edit')" @click="edit(it)">
                <PhPencilSimple :size="14" weight="bold" />
              </Button>
            </ItemActions>
            <Button variant="ghost" size="icon" class="hover:text-label" :title="t('vault.copyPassword')" @click="copy(it.id, 'password')">
              <PhKey :size="14" weight="bold" />
            </Button>
          </Item>
        </template>

        <!-- 2FA -->
        <template v-else>
          <Item
            v-for="(it, i) in rows"
            :key="it.id"
            as="button"
            type="button"
            class="duration-160 ease-out"
            :style="{ '--delay': `${50 + Math.min(i, 7) * 35}ms` }"
            @click="copy(it.id, 'totp')"
          >
            <ItemMedia variant="icon">{{ (it.name[0] ?? "?").toUpperCase() }}</ItemMedia>
            <ItemContent class="text-left">
              <ItemTitle class="type-body block truncate">{{ it.name }}</ItemTitle>
              <ItemDescription class="type-caption truncate" :class="copied.id === it.id ? 'text-sys-green' : 'text-label-3'">
                {{ copied.id === it.id ? copiedLabel : (it.username ?? "") }}
              </ItemDescription>
            </ItemContent>
            <template v-if="vault.codes.get(it.id)">
              <span class="tabular text-[15px] leading-5 font-medium tracking-[0.06em]">{{ split(vault.codes.get(it.id)!.code) }}</span>
              <CountdownRing :remaining="vault.codes.get(it.id)!.remaining" :period="vault.codes.get(it.id)!.period" />
            </template>
            <PhCopy v-else :size="14" weight="bold" class="text-label-3" />
          </Item>
        </template>
      </TransitionGroup>
    </div>
  </div>
</template>
