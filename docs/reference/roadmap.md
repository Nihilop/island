# Roadmap SDK — capacités pour les extensions

Suivi des idées du brainstorm « quoi de plus pour les extensions ». Chaque nouveau
service natif = un dossier `src-tauri/src/services/<x>/{mod,windows}.rs` (couche
commande/permission cross-platform + impl native gated).

## ✅ Livré

| Capacité | API | Permission |
|---|---|---|
| Presse-papiers (texte + image) | `island.clipboard.*` | `clipboard` |
| Secrets chiffrés | `ctx.secrets.*` | — (scopé par id) |
| Thème (dark/light) | `island.theme.current/onChange` | — |
| Capteurs système | `island.system.battery/online/volume/idle*` | `system` |
| Providers de launcher (palette) | `ctx.launcher.provider` | — (consommé par Aniplex) |
| Bus inter-extensions | `island.bus.emit/on` | — |
| Synthèse vocale | `island.speak(text)` | — |
| Automatisation clavier | `island.input.typeText` | `input` ⚠ |
| Conscience des fenêtres | `island.windows.foreground/list/focus/onForegroundChanged` | `windows` ⚠ |

## ⏳ À faire (déféré, prochaine passe)

- **File-drop sur une view** — l'île devient cible de glisser-déposer de fichiers.
  Tauri `onDragDropEvent` + test de la région de l'île + callback SDK
  (`ctx.view`/`island.onFileDrop`). **Retouche `Island.vue`** → à faire avec un passage
  de vérification visuelle (impossible à valider à l'aveugle).
- **Réseau temps réel** — WebSocket / SSE (`ctx.realtime.ws/sse`). Le `http` actuel est
  request→response ; débloque chat, tickers, présence, données live. Service propre,
  mécanique (sans risque UI).

## 💡 Backlog (non commencé)

- `input.sendKeys(accelerator)` — combos (Ctrl+C…), en plus de `typeText`.
- `system.brightness` — luminosité (passe par WMI, plus lourd).
- Mirroring des notifications OS (`UserNotificationListener`, identité MSIX) / toast OS sortant.
- TTS : choix de voix / débit / `stop()`.
- Filesystem « capability-based » : `pickFile()` → handle → `read/write` sur ce handle
  (modèle File System Access), permission `filesystem`.
