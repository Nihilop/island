// Ask : conversation EN MÉMOIRE seulement (perdue au redémarrage). Réponse en streaming
// depuis Rust (`ai://delta`, `ai://done`) ; la clé ne transite jamais par le front.

import { computed, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { settings } from "../../notch/settings";

export interface ChatMsg {
  role: "user" | "assistant";
  content: string;
}

export const chat = reactive<{ messages: ChatMsg[] }>({ messages: [] });
export const streaming = ref<string | null>(null);
export const error = ref<string | null>(null);

export const provider = computed(() => settings.ask.provider);
export const current = computed(() => settings.ask.byProvider[settings.ask.provider]);

export function send(text: string) {
  const t = text.trim();
  if (!t || streaming.value) return;
  error.value = null;
  chat.messages.push({ role: "user", content: t });
  const history = chat.messages.map((m) => ({ role: m.role, content: m.content }));
  chat.messages.push({ role: "assistant", content: "" });
  const id = crypto.randomUUID();
  streaming.value = id;
  const c = current.value;
  invoke("ai_chat", {
    id,
    provider: settings.ask.provider,
    model: c.model,
    effort: c.effort,
    baseUrl: c.baseUrl ?? "",
    messages: history,
  }).catch((e) => finish(id, String(e)));
}

function finish(id: string, err: string | null) {
  if (streaming.value !== id) return;
  streaming.value = null;
  const last = chat.messages[chat.messages.length - 1];
  if (err) {
    error.value = err;
    // La bulle vide de réponse est retirée en cas d'erreur.
    if (last?.role === "assistant" && !last.content) chat.messages.pop();
  }
}

export function stop() {
  if (streaming.value) invoke("ai_cancel", { id: streaming.value }).catch(() => {});
}

export function clear() {
  stop();
  streaming.value = null;
  chat.messages.splice(0);
  error.value = null;
}

export function initAsk() {
  listen<{ id: string; text: string }>("ai://delta", (e) => {
    if (e.payload.id !== streaming.value) return;
    const last = chat.messages[chat.messages.length - 1];
    if (last?.role === "assistant") last.content += e.payload.text;
  });
  listen<{ id: string; error: string | null }>("ai://done", (e) => finish(e.payload.id, e.payload.error));
}
