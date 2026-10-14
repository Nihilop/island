<script setup lang="ts">
// Outil « Discord » (520 × 210) : salon vocal en cours, membres (anneau vert quand ils
// parlent, grisés + micro barré si muets), micro / sourdine / raccrocher.
import { computed } from "vue";
import {
  PhDiscordLogo,
  PhGear,
  PhHeadphones,
  PhMicrophone,
  PhMicrophoneSlash,
  PhPhoneDisconnect,
  PhSpeakerSimpleSlash,
} from "@phosphor-icons/vue";
import Empty from "../../ui/Empty.vue";
import { Button } from "@/sdk/ui/button";
import IconButton from "../../ui/IconButton.vue";
import { cn } from "@/lib/utils";
import { action, discord, initials } from "./store";
import { openSettings } from "../settings/nav";
import { t } from "../../notch/i18n";

const MAX_AVATARS = 6;
const shown = computed(() => discord.call?.members.slice(0, MAX_AVATARS) ?? []);
const more = computed(() => Math.max(0, (discord.call?.members.length ?? 0) - MAX_AVATARS));
</script>

<template>
  <div class="flex h-full flex-col justify-between px-5 pb-1.5">
    <Teleport defer to="#notch-header-actions">
      <IconButton :icon="PhGear" :label="t('discord.settings')" @click="openSettings('discord')" />
    </Teleport>

    <template v-if="discord.status === 'ready' && discord.call">
      <div class="flex items-center gap-3">
        <img v-if="discord.call.guildIcon" :src="discord.call.guildIcon" alt="" class="size-11 shrink-0 rounded-[30%] object-cover" />
        <div v-else class="type-footnote grid size-11 shrink-0 place-items-center rounded-[30%] bg-fill-3 font-semibold">{{ initials(discord.call.guild) || "DM" }}</div>
        <div class="min-w-0">
          <div class="type-title truncate font-semibold">{{ discord.call.channel }}</div>
          <div class="type-body truncate text-label-2">{{ discord.call.guild ?? t("discord.private") }}</div>
        </div>
      </div>
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5">
          <div
            v-for="m in shown"
            :key="m.id"
            :class="
              cn(
                'relative size-8 rounded-full transition-[box-shadow,opacity] duration-160 ease-out',
                m.speaking && 'shadow-[0_0_0_2px_var(--notch),0_0_0_4px_var(--sys-green)]',
                m.muted && 'opacity-50',
              )
            "
            :title="m.name"
          >
            <img :src="m.avatar" alt="" class="size-full rounded-full" />
            <span v-if="m.muted" class="absolute -right-0.5 -bottom-0.5 grid size-3.5 place-items-center rounded-full bg-sys-red text-white">
              <PhMicrophoneSlash :size="9" weight="fill" />
            </span>
          </div>
          <span v-if="more" class="type-caption text-label-3">+{{ more }}</span>
        </div>
        <div class="flex gap-2.5">
          <Button size="icon-xl" :variant="discord.mute ? 'secondary' : 'default'" :title="t('discord.mute')" @click="action('mute')">
            <PhMicrophone v-if="!discord.mute" :size="20" weight="fill" />
            <PhMicrophoneSlash v-else :size="20" weight="fill" />
          </Button>
          <Button size="icon-xl" :class="cn(discord.deaf && 'text-sys-red')" :title="t('discord.deaf')" @click="action('deaf')">
            <PhSpeakerSimpleSlash v-if="discord.deaf" :size="20" weight="fill" />
            <PhHeadphones v-else :size="20" weight="fill" />
          </Button>
          <Button size="icon-xl" variant="destructive" :title="t('discord.hangup')" @click="action('hangup')">
            <PhPhoneDisconnect :size="20" weight="fill" />
          </Button>
        </div>
      </div>
    </template>

    <Empty
      v-else
      :icon="PhDiscordLogo"
      :title="discord.status === 'ready' ? t('discord.noCall') : t(`discord.status.${discord.status}`)"
      :text="discord.status === 'error' ? (discord.error === 'redirectUri' ? t('discord.err.redirect') : discord.error ?? '') : t(`discord.help.${discord.status}`)"
    >
      <Button v-if="discord.status === 'missingConfig' || discord.status === 'error'" @click="openSettings('discord')">
        {{ t("discord.configure") }}
      </Button>
    </Empty>
  </div>
</template>
