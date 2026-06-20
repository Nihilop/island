// Pont hôte exposé aux extensions via window.__ISLAND__.
// Le SDK (bundlé dans chaque extension) délègue ici — ça marche entre instances
// Vue différentes car tout passe par invoke + events (pas de partage d'objets Vue).
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import {
  openModal, closeModal, openView, closeView, resizeView, openDrop, closeDrop, selectRegion, showRegionOutline,
  openWindow, closeWindow, focusWindow,
  type ModalSpec, type ViewSize, type Region, type WindowOpts,
} from "./composables/overlay";
import { setIdleState, setIdleAction, setIdleTap, type IdleAction } from "./composables/idle";
import { setLauncherEntry, setLauncherProvider, type LauncherEntry, type LauncherProvider } from "./composables/launcher";
import { registerShortcut, unregisterShortcut } from "./composables/shortcuts";
import { post as postNotif, markRead, clearUnread, type NotifSpec } from "./composables/notifications";
import { busEmit, busOn } from "./composables/bus";
import type { Component } from "vue";
import type { IdleState } from "./sdk";

export function installBridge() {
  (window as any).__ISLAND__ = {
    invoke,
    listen: (event: string, cb: (e: { payload: any }) => void) => listen(event, cb),
    openModal: (req: ModalSpec) => openModal(req),
    closeModal: () => closeModal(),
    openView: (c: Component, size?: ViewSize) => openView(c, size),
    closeView: () => closeView(),
    resizeView: (size: ViewSize) => resizeView(size),
    openDrop: (c: Component) => openDrop(c),
    closeDrop: () => closeDrop(),
    openWindow: (c: Component, opts?: WindowOpts) => openWindow(c, opts),
    closeWindow: (id?: string) => closeWindow(id),
    focusWindow: (id: string) => focusWindow(id),
    setIdleState: (key: string, state: IdleState | null, priority: number) => setIdleState(key, state, priority),
    setIdleAction: (key: string, action: IdleAction | null) => setIdleAction(key, action),
    setIdleTap: (key: string, handler: (() => void) | null) => setIdleTap(key, handler),
    setLauncherEntry: (key: string, entry: LauncherEntry | null) => setLauncherEntry(key, entry),
    setLauncherProvider: (key: string, provider: LauncherProvider | null) => setLauncherProvider(key, provider),
    registerShortcut: (key: string, accelerator: string, handler: () => void) => registerShortcut(key, accelerator, handler),
    unregisterShortcut: (key: string) => unregisterShortcut(key),
    pickFolder: async () => {
      const p = await open({ directory: true, multiple: false });
      return typeof p === "string" ? p : null;
    },
    selectRegion: () => selectRegion(),
    showRegionOutline: (r: Region | null) => showRegionOutline(r),
    notify: (spec: NotifSpec) => postNotif(spec),
    dismissNotification: (id: string) => markRead(id),
    clearNotifications: () => clearUnread(),
    busEmit: (channel: string, payload: unknown) => busEmit(channel, payload),
    busOn: (channel: string, handler: (payload: any) => void, owner?: string) => busOn(channel, handler, owner),
  };
}
