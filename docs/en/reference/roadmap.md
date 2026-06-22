# SDK roadmap — capabilities for extensions

Tracking ideas from the "what else for extensions" brainstorm. Each new native service = a
`src-tauri/src/services/<x>/{mod,windows}.rs` folder (cross-platform command/permission layer
+ gated native impl).

## ✅ Shipped

| Capability | API | Permission |
|---|---|---|
| Clipboard (text + image) | `island.clipboard.*` | `clipboard` |
| Encrypted secrets | `ctx.secrets.*` | — (scoped by id) |
| Theme (dark/light) | `island.theme.current/onChange` | — |
| System sensors | `island.system.battery/online/volume/idle*` | `system` |
| Launcher providers (palette) | `ctx.launcher.provider` | — (used by Aniplex) |
| Inter-extension bus | `island.bus.emit/on` | — |
| Text-to-speech | `island.speak(text)` | — |
| Keyboard automation | `island.input.typeText` | `input` ⚠ |
| Window awareness | `island.windows.foreground/list/focus/onForegroundChanged` | `windows` ⚠ |
| PTY terminals / exec | `ctx.terminal.*` | `terminal` ⚠⚠ |

## ⏳ To do (deferred, next pass)

- **File-drop on a view** — the island becomes a drop target for files. Tauri
  `onDragDropEvent` + island-region test + SDK callback (`ctx.view`/`island.onFileDrop`).
  **Touches `Island.vue`** → to be done with a visual-check pass (can't be validated blind).
- **Real-time network** — WebSocket / SSE (`ctx.realtime.ws/sse`). The current `http` is
  request→response; this unblocks chat, tickers, presence, live data. Clean, mechanical
  service (no UI risk).

## 💡 Backlog (not started)

- `input.sendKeys(accelerator)` — combos (Ctrl+C…), in addition to `typeText`.
- `system.brightness` — brightness (via WMI, heavier).
- OS notification mirroring (`UserNotificationListener`, MSIX identity) / outgoing OS toast.
- TTS: voice choice / rate / `stop()`.
- "Capability-based" filesystem: `pickFile()` → handle → `read/write` on that handle (File
  System Access model), `filesystem` permission.
