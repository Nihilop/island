# The SDK

`@island/sdk` (the `packages/sdk` folder in the repository) is an extension's only gateway to
Island. It is used the same way in the logic (`main`) and in views.

```ts
import { connect } from "@island/sdk";
const island = await connect();
```

`connect()` waits for Island's handshake, then returns the `island` object. Every method
returns a promise; a missing permission rejects with `permission:<name>`.

## Identity, language, theme

| | |
|---|---|
| `island.id` | Extension identifier (provided by Island, not declared by the code) |
| `island.locale` | Island's language at startup (`fr`, `en`…) |
| `island.theme` | `"dark"` or `"light"`; the SDK sets the `light` class on `<html>` |
| `island.onTheme(cb)` | Live theme changes |
| `island.t(key, params?)` | Translated text (`locales` files), `{param}` replaced |
| `island.log(...args)` | Logs to Island's development console |

## Data

| | Permission |
|---|---|
| `island.storage.get(key)` / `set(key, value)` | none — JSON storage private to the extension |
| `island.secrets.get(key)` / `set(key, value)` / `delete(key)` | `secrets` — Windows credential store, private to the extension |
| `island.settings.get(key)` | none — value of a declared setting (manifest default otherwise) |
| `island.settings.onChange(cb)` | none — called with the changed key |

## Network

```ts
const r = await island.http.request({ url: "https://api.github.com/user", method: "GET", headers: { Authorization: `Bearer ${token}` } });
// r = { status, headers: [name, value][], body: string }

const user = await island.http.json<{ login: string }>("https://api.github.com/user", { headers });
// throws HttpError(status, body) if status ≥ 400
```

Permission `network:<domain>` (`network:*.exemple.com` for subdomains). **HTTPS only**, no
redirects followed, response capped at 5 MB, local and private addresses refused.
`timeout` (seconds, 1 to 120, default 20) extends a request's deadline.

```ts
// Binary body (video, images, files; 64 MB max), transferred without copying.
const r = await island.http.bytes({ url: "https://example.com/video/seg0001.ts", timeout: 90 });
// r = { status, headers, body: ArrayBuffer }
```

**Cookie sessions** (permission `cookies`): Island keeps the cookies set by the declared
domains and sends them back on later requests, like a browser. They stay invisible to the
extension (no `Set-Cookie` in responses, no `Cookie` accepted as input), are stored in the
Windows credential vault and erased on uninstall. `island.http.clearCookies()` forgets
them all (sign-out).

## Windows and events

```ts
// Opens the declared window `player` (or brings it to front) with arguments.
await island.windows.open("player", { episode: 12 }, { title: "Player" });

// In the window's page:
const args = await island.windows.args<{ episode: number }>();
island.windows.onArgs((a) => …); // reopened with other arguments
await island.windows.setTitle("Episode 12");

// Between the extension's pages (island, windows, logic):
island.events.on("progress", (p) => …);
await island.events.emit("progress", { episode: 12, position: 340 });
```

| | Permission |
|---|---|
| `island.windows.open(id, args?, { title? })` | `windows` — a window from [`contributes.windows`](./emplacements#window) |
| `island.windows.args()` / `onArgs(cb)` | none — in a window's page |
| `island.windows.setTitle(title)` / `close()` | none — in a window's page |
| `island.events.emit(name, data)` / `on(name, cb)` | none — never leaves the extension |

## Capture

Permission `capture`. **Island** shows the selection (drawn region, hovered window or
screen), captures and encodes — the extension only drives. The island never appears in
captures.

```ts
const target = await island.capture.select("region"); // null if cancelled (Esc)
if (target) {
  const shot = await island.capture.screenshot(target, { clipboard: true }); // PNG in Pictures\Island
  await island.capture.startRecording(target, { systemAudio: true, mic: false, quality: "medium", fps: 30, codec: "h264" });
  const video = await island.capture.stopRecording(); // { path, durationMs } — MP4 in Videos\Island
}
island.capture.onRecording((s) => …); // start / end (live “REC” activity…)
await island.capture.reveal(path);     // show a capture in File Explorer
```

Video: Windows hardware encoder (H.264 or H.265), PC sound and mic mixed. One recording at
a time; while recording, the region outline is visible (not in the video) and the island's
red indicator is on.

## Wallpaper

Permission `wallpaper`. Sets a **static** Windows wallpaper, on one screen or all of them.
The extension passes the image itself (a `Blob`, an `ArrayBuffer` or a `Uint8Array`):
Island checks it is a real image (30 MB max), stores it in its own folder and sets it. No
file path is ever accepted.

```ts
const screens = await island.wallpaper.displays(); // [{ id, x, y, width, height, work, scale, primary }]
const { body } = await island.http.bytes({ url: "https://exemple.com/photo-of-the-day.jpg" }); // network:exemple.com
await island.wallpaper.setImage(body, { display: screens[0].id, fit: "fill" });
```

No `display`: all screens. `fit` (`fill`, `fit`, `stretch`, `center`, `tile`, `span`) is a
global Windows setting: it applies to every screen. **Live** wallpapers aren't exposed to
extensions: they're Island's own “Live wallpapers” tool.

## In the island

| | Permission |
|---|---|
| `island.live.set(id, spec \| null)` | none — [live activity](./emplacements#live-activity) |
| `island.launcher.provide(id, search)` | none — [launcher results](./emplacements#launcher) |
| `island.launcher.onRun(id, action)` | none — result picked |
| `island.commands.on(id, action)` | none — [command](./emplacements#commands) triggered |
| `island.notify({ title, body?, banner? })` | `notifications` |
| `island.openUrl(url)` | none — http(s) only, default browser |
| `island.clipboard.readText()` / `writeText(text)` | `clipboard` |

## UI kit

```ts
import "@island/sdk/ui.css";
```

The island's colors and typography (variables `--label`, `--label-2`, `--fill-1`…, `--green`,
`--orange`, `--red`, `--blue`), a transparent background, and a few classes: `.list`, `.item`,
`.btn` (`.primary`), `.badge` (`.green`, `.orange`…), `.seg` (tabs), `.empty`, `.caption`,
`.muted`, `.truncate`. The light theme is handled automatically.
