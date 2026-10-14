// Discord : état poussé par Rust (`discord://state`). La connexion ne tourne que si
// l'outil Discord est actif et qu'un Client ID est renseigné.

import { computed, reactive, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { settings } from "../../notch/settings";

export interface Member {
  id: string;
  name: string;
  avatar: string;
  speaking: boolean;
  muted: boolean;
}

export interface DiscordState {
  status: "off" | "missingConfig" | "notRunning" | "connecting" | "authorizing" | "ready" | "error";
  error: string | null;
  mute: boolean;
  deaf: boolean;
  me: string | null;
  call: null | { channelId: string; channel: string; guild: string | null; guildIcon: string | null; members: Member[] };
}

export const discord = reactive<DiscordState>({ status: "off", error: null, mute: false, deaf: false, me: null, call: null });

export const inCall = computed(() => discord.status === "ready" && !!discord.call);
/** Quelqu'un parle dans le salon. */
export const someoneSpeaking = computed(() => !!discord.call?.members.some((m) => m.speaking));
/** C'est nous qui parlons (pulsation du voyant vert). */
export const meSpeaking = computed(() => !!discord.call?.members.some((m) => m.id === discord.me && m.speaking));

export const action = (a: "mute" | "deaf" | "hangup") => invoke("discord_action", { action: a }).catch(() => {});

const enabled = computed(() => settings.tools.some((t) => t.id === "discord" && t.enabled));

export function reconnect() {
  invoke("discord_configure", { clientId: settings.discordClientId || null, enabled: enabled.value }).catch(() => {});
}

export async function initDiscord() {
  listen<DiscordState>("discord://state", (e) => Object.assign(discord, e.payload));
  Object.assign(discord, await invoke<DiscordState>("discord_state").catch(() => ({})));
  watch([enabled, () => settings.discordClientId], reconnect, { immediate: true });
}

/** Initiales du serveur (3 max) quand il n'a pas d'icône. */
export function initials(name: string | null): string {
  return (name ?? "")
    .split(/\s+/)
    .filter(Boolean)
    .map((w) => w[0])
    .join("")
    .slice(0, 3)
    .toUpperCase();
}
