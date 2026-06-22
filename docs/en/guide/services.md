# 5. Services & permissions

Some capabilities go through Island's **native backend** (screen capture, system stats,
media, network, launching apps). Each is a **service gated by a permission** that you
declare in `manifest.json`.

## Permission = access key

The host checks the permission **on every call** (`ext_has_permission`, in addition to the
consent given at install — defense in depth). Calling a service without its permission is
**denied**: an error, or a neutral value for services without a return (volume `-1`, zeroed
stats, no-op).

## Binding your identity: `useIsland(EXT_ID)`

Gated services need to know **who** is calling, to look up your manifest. The SDK attaches
your id automatically **if you bind it**:

```ts
const EXT_ID = "com.island.myextension";
const island = useIsland(EXT_ID);   // inside a component
await island.system.stats();        // extId attached automatically → the host checks "system"
```

- Inside `activate(ctx)`, **`ctx` is already bound** — `ctx.capture`, `ctx.media`… work
  directly.
- Inside a component, pass your `EXT_ID` to `useIsland(EXT_ID)`.
- `useIsland()` without an id still works for everything that is **not gated** (`view`,
  `drop`, `window`, `notify`, `launcher`, `idle`, `openExternal`…).

## Catalog

| Service | Permission | Trust | OS |
| ------- | ---------- | ----- | -- |
| Screen capture | `capture` | standard | Windows ✓ |
| Native encoder (recording) | `native-encoder` | ⚠ **high** | binary provided by the ext |
| System stats | `system` | standard | multi ✓ |
| Media (SMTC) | `media` | standard | Windows ✓ |
| Applications (launcher) | `apps` | ⚠ **high** | Windows ✓ |
| Clipboard | `clipboard` | standard | multi ✓ |
| Window awareness | `windows` | ⚠ **high** | Windows ✓ |
| Keyboard automation | `input` | ⚠ **high** | Windows ✓ |
| Native HTTP (cookie-jar) | `network` | standard | multi ✓ |
| Per-extension storage | `storage` | standard | multi ✓ |
| Global keyboard shortcuts | `shortcuts` | standard | multi ✓ |
| PTY terminals / exec | `terminal` | ⚠⚠ **maximum** | multi ✓ |

> **Multi-OS**: the command + permission layer is cross-platform; the native implementation
> lives in `services/<svc>/windows.rs` (Windows-gated). Porting to macOS/Linux = adding a
> `macos.rs`/`linux.rs` to the same contract, without touching the SDK or the permissions.

## Per service

### `capture` — screen capture & recording

```ts
const island = useIsland(EXT_ID);
const screens = await island.capture.listDisplays();
const png = await island.capture.screenshot({ display: 1, region });
const ok  = await island.capture.isRecording();
```

`listDisplays`, `screenshot`, `startRecording`, `stopRecording`, `isRecording`. The UI
helpers `selectRegion`, `showRegionOutline`, `pickFolder` are **not** gated. This is screen
access: reserved for extensions that justify it.

### `native-encoder` — run the extension's encoder

Recording **delegates encoding to a binary provided by the extension** (typically ffmpeg);
the host stays agnostic. ⚠ **High trust** (runs a native binary) → shown prominently at
install. Safeguard: the binary is **confined to the extension's folder** (`..`, absolute
paths, drive letters forbidden). See the recipe
[Embedding a native binary](/en/guide/recettes#embedding-a-native-binary).

### `system` — stats & sensors

```ts
const sys = useIsland(EXT_ID).system;
const s = await sys.stats();        // { cpu, cores[], memUsed, memTotal }
const bat = await sys.battery();    // { percent, charging } | null
const net = await sys.online();     // boolean
const vol = await sys.volume();     // { level, muted } | null  (MASTER output volume)
await sys.setVolume(0.3);
await sys.setMuted(true);
const off = sys.onUserIdle(60_000, () => pause(), () => resume()); // idle > 60 s
```

`stats` (sysinfo) is cross-platform; the sensors (battery, network, master volume, idle)
are Windows for now (`GetSystemPowerStatus`, `InternetGetConnectedState`, WASAPI,
`GetLastInputInfo`). The **master volume** is distinct from `media.setVolume` (which drives
the media app). On an OS without an impl: neutral values (`null` / `0` / `false`).

### `media` — native media control

```ts
const island = useIsland(EXT_ID);
island.media.toggle(); island.media.next(); island.media.setVolume(0.5);
const m = island.media.state; // reactive (title/artist/playback) — FREE TO READ, ungated
```

Follows the **OS active media session** (no hardcoded app). `media.state` (the event stream)
is not gated; only the **actions** are. Windows (SMTC + WASAPI volume).

### `apps` — list & launch applications

```ts
const apps  = await island.invoke("list_apps", { extId: EXT_ID });
await island.invoke("launch_path", { extId: EXT_ID, path });
const icons = await island.invoke("app_icons", { extId: EXT_ID, paths });
```

⚠ **High trust** (can launch installed programs). Windows (Start Menu `.lnk` shortcuts +
ShellExecute).

### `network` — native HTTP with a cookie-jar

```ts
const r = await useIsland().http.request({ extId: EXT_ID, url, method, body, headers });
```

Consumes a third-party API with a **per-cookie session**, free of a browser `fetch`'s
CORS/SameSite restrictions (one cookie-jar per extension, **persisted** → session kept
across restarts). The extension provides the full URL → an SSRF surface bounded by the
permission. See the recipe
[Calling an API with a session](/en/guide/recettes#calling-an-api-with-a-cookie-session).

### `clipboard` — clipboard

```ts
const island = useIsland(EXT_ID);
await island.clipboard.writeText("copied!");
const txt = await island.clipboard.readText();
const png = await island.clipboard.readImage();      // PNG data URL, or null
await island.clipboard.writeImage(canvas.toDataURL());
```

Text **and image**. Gated because reading the clipboard exposes potentially sensitive data.
Cross-platform (`arboard`).

### `windows` — window awareness

```ts
const w = useIsland(EXT_ID).windows;
const fg = await w.foreground();      // { id, title, app } | null
const all = await w.list();           // visible top-level windows
await w.focus(fg!.id);                // bring a window to the foreground
const off = w.onForegroundChanged((win) => update(win));  // on active-app change
```

- **Commands**: `window_foreground`, `window_list`, `window_focus`.
- **Trust**: ⚠ **high** — reveals activity (apps and window **titles** = potentially
  sensitive) and can activate a window.
- **OS**: Windows (`GetForegroundWindow`/`EnumWindows`/`SetForegroundWindow`).

### `input` — keyboard automation

```ts
await useIsland(EXT_ID).input.typeText("text typed into the active app");
```

- **Command**: `input_type_text` (Unicode SendInput).
- **Trust**: ⚠ **high** — writes into any foreground application (text-expander, auto-paste…).
  Shown prominently at install.
- **OS**: Windows.

### `terminal` — PTY terminals & exec

```ts
const id = await useIsland(EXT_ID).terminal.spawn({ cwd, cols: 80, rows: 24 }); // → xterm.js
const { stdout } = await useIsland(EXT_ID).terminal.exec({ cmd: "git", args: ["status"], cwd });
```

- **Methods**: `spawn/write/resize/kill/exec/onData/onExit`. PTY output is base64 (decode
  for xterm). `exec` = captured one-shot command (git, docker…).
- **Commands**: `pty_spawn`, `pty_write`, `pty_resize`, `pty_kill`, `pty_exec`.
- **Trust**: ⚠⚠ **MAXIMUM** — runs **arbitrary processes** (≈ code execution). Only grant to
  trusted extensions (dev tools). Shown prominently at install.
- **OS**: cross-platform (crate `portable-pty` → ConPTY on Windows).

### `bus`, `speak` (no backend permission)

- **`bus`** (`island.bus.emit/on`): pub/sub **between extensions** (composition). Pick
  prefixed channels (`"nowplaying:update"`). Subscriptions cleaned up on deactivation.
- **`speak`** (`island.speak(text)`): text-to-speech (SAPI). Audio output, ungated like
  `notify`/`openExternal`.

### `storage`, `secrets` & `shortcuts` (no backend permission)

See [The SDK](/en/guide/sdk): `storage` (plaintext key→value) and `secrets` (**encrypted**
vault) are isolated per id — an extension only accesses its own data, so **no permission**
is required. `shortcuts` (global shortcuts) likewise. All cross-platform.

## Recap "which permission for which need"

- Read the screen / record → **`capture`** (+ **`native-encoder`** if you bundle an encoder).
- CPU/RAM → **`system`**. · Media playback/volume → **`media`**.
- List/launch apps → **`apps`** (⚠). · Web API with a cookie session → **`network`**.
- Persist settings → **`storage`**. · Global shortcut → **`shortcuts`**.
- See/activate desktop windows → **`windows`** (⚠). · Type into the active app → **`input`** (⚠).
- Run processes / interactive terminal → **`terminal`** (⚠⚠ maximum trust).

Purely-UI contributions (`launcher`, `idle`, `view`, `drop`, `window`, `notify`) require
**no** backend permission.
