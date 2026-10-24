# Slots

Slots are declared in the manifest's `contributes`: Island knows what an extension adds
**without running its code** (and shows it at install time).

## Tool

```json
"tools": [{ "id": "inbox", "title": "GitHub", "icon": "icons/github.svg", "view": "dist/inbox.html", "size": [560, 340] }]
```

- A button in the dock (order and enablement in Settings › Tools, like built-in tools).
  Too many tools: the last ones move into the "•••" menu.
- `view`: HTML page shown in the open island, below the header and above the dock.
- `size`: width × height of the open island, header and dock included (max. 740 × 540).
- The view is reloaded every time it opens: keep useful state in `island.storage`.

## Window

```json
"windows": [{ "id": "player", "title": "Player", "view": "dist/player.html", "size": [1280, 760], "minSize": [640, 400] }]
```

- Permission `windows`. A real Windows window, opened by `island.windows.open(id, args)`:
  no system frame, Island's own title bar (title, minimize, maximize, close).
- `view`: the page shown in the window, isolated like the extension's other pages.
  Fullscreen (`requestFullscreen` → Windows fullscreen) and autoplay allowed.
- One window per `id`: reopening it brings it to front and passes the new arguments
  (`island.windows.onArgs`). Size and position are remembered.

## Live activity

```json
"live": [{ "id": "status", "tool": "inbox" }]
```

```ts
await island.live.set("status", { icon: "icons/ci.svg", text: "3", color: "orange" });
await island.live.set("status", null); // remove
```

| Field | |
|---|---|
| `icon` | Path to a monochrome SVG from the extension, tinted by `color` |
| `text` | Short text (about a dozen visible characters) |
| `color` | `label`, `green`, `orange`, `red`, `blue` |
| `priority` | `normal` (after music) or `high` (before the timer, calls and music) |

Visible while the island is closed; a click opens the tool named by `tool`. Only one activity
is shown at a time: Island's alerts (timer finished, notification) take precedence.

## Launcher

```json
"launcher": [{ "id": "repos", "prefix": "gh", "title": "GitHub" }]
```

```ts
island.launcher.provide("repos", async (q) =>
  repos.filter((r) => r.name.includes(q)).map((r) => ({ id: r.url, title: r.name, subtitle: r.description })),
);
island.launcher.onRun("repos", (id) => island.openUrl(id));
```

- Without a prefix: the query is sent from 2 characters onwards; at most 5 results, below
  Island's own.
- With a prefix (`gh island`): only this provider answers, up to 40 results. A prefix
  ending in `:` sticks to the query, like Island's own (`p:game`).
- Stale responses (the user kept typing) are ignored by Island.

## Notifications

Permission `notifications`. `island.notify({ title, body })` goes through Island's single
API: a **Minimal** or **Full** banner, depending on the user's setting, when the island is
closed; nothing if it is open; history in the Notifications tool.
`banner: false` = history only.

## Commands

```json
"commands": [{ "id": "refresh", "title": "GitHub: refresh", "shortcut": "Control+Alt+G" }]
```

```ts
island.commands.on("refresh", () => refresh());
```

The shortcut is global (Tauri accelerator format). If it is already taken, the command
remains available in the launcher (search by title).

## Settings

```json
"settings": [
  { "key": "token", "type": "secret", "label": "Access token", "help": "…" },
  { "key": "interval", "type": "select", "label": "Refresh interval", "default": "5",
    "options": [{ "value": "2", "label": "2 min" }, { "value": "5", "label": "5 min" }] },
  { "key": "notify", "type": "switch", "label": "Notifications", "default": true }
]
```

Types: `text`, `secret` (stored in the credential store, never shown again), `switch`,
`select`, `number`, `list` (one value per line → array). Island generates the page in
Settings › Extensions; the extension reads with `island.settings.get(key)` and follows
`onChange`.
