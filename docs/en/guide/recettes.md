# 6. Recipes

Concrete patterns, taken from the reference extensions (Flow, Capture, Aniplex, Spotify,
Monitoring).

## Keyboard input in a view

The overlay does **not** take keyboard focus by default (intentional: it doesn't steal
focus from games). If your view has an `<input>`:

```ts
ctx.view.open(Search, COMPACT);
ctx.invoke("overlay_focus");   // gives keyboard focus to the overlay
// then, when the component mounts: inputEl.focus()
```

When the overlay loses focus (a click elsewhere), the island collapses by itself. That's
what **Flow** (the launcher) does.

## Launching applications

*Permission: `apps`.*

```ts
const EXT_ID = "com.island.flow";
const apps = await useIsland().invoke<{name: string; path: string}[]>("list_apps", { extId: EXT_ID });
// …filter / score…
await useIsland().invoke("launch_path", { extId: EXT_ID, path: chosen });
```

`list_apps` returns Start Menu shortcuts; `launch_path` runs the target via ShellExecute.
`app_icons({ extId, paths })` returns icons (PNG data URL) on demand. ⚠ High trust level
(launches executables).

## Calling an API (network)

### Simple case: standard `fetch`

The host runs with `csp: null` → a `fetch()` to an API that returns CORS works without
declaring anything. Example: `https://meme-api.com/gimme` (CORS `*`).

### Calling an API with a cookie session

*Permission: `network`.* When the API uses a **cookie session** (login), a browser `fetch`
is blocked by CORS/SameSite. Use Island's **native** HTTP, which keeps a per-extension
cookie-jar:

```ts
const EXT_ID = "com.island.aniplex";
const r = await useIsland().http.request({
  extId: EXT_ID,
  method: "POST",
  url: "https://api.example.com/auth/login",
  body: { user, pass },                 // object → automatic JSON
  headers: { "X-Custom": "…" },
});
// r = { status: number, body: string }
```

A cookie set by a response is **replayed** on subsequent requests, and the cookie-jar is
**persisted on disk** → the session survives an Island restart (no need to log in again),
exactly like a desktop client.

## Embedding a native binary

*Permission: `native-encoder`.* The host doesn't decide the codec: an extension can provide
its own binary (e.g. ffmpeg). It's **one trust notch above JS**:

```ts
const EXT_ID = "com.island.capture";
// 1) Download the binary INTO the extension's folder (on first need)
await useIsland().capture.fetchBinary({
  extId: EXT_ID,
  url: "https://…/ffmpeg-win64.zip",
  dest: "binaries/ffmpeg.exe",
  zipEntry: "bin/ffmpeg.exe",           // if the URL is a zip: entry to extract
});                                      // progress via the "encoder://download" event

// 2) Record by providing the encoder
await useIsland().capture.startRecording({
  region, display, fps: 30,
  encoder: { extId: EXT_ID, bin: "binaries/ffmpeg.exe", args: ["-c:v","libx265","-crf","25"] },
});
```

- The binary and the destination are **confined** to `extensions/<id>/` (against `..`/absolute
  paths) → impossible to launch a system program.
- **Don't put it in the `.island`**: keep the package light, download on first need,
  `.gitignore` the `binaries/` folder.

**System audio**: add `audio: true` + `audioArgs`:

```ts
startRecording({ …, audio: true, encoder: { …, audioArgs: ["-c:a","aac","-b:a","160k"] } });
```

The host captures the PC's audio (WASAPI loopback), encodes the video, then muxes the two.

## Floating window (player, mini-tool)

```ts
const id = ctx.window.open(Player, { title: "Player", width: 480, height: 270, resizable: true });
// movable, independent of the island; ctx.window.close(id) to close
```

## Open a URL in the browser

```ts
await ctx.openExternal("https://example.com/watch/123");
```

Handy when playback/rendering is better handled by the browser (that's what **Aniplex**
does: click an episode → opens the web player + `ctx.view.close()`).

## Feeding the launcher search (provider)

The island's launcher becomes an **extensible command palette**: register a provider, and
as soon as the user types, your results show up. As long as no extension registers a
provider, the launcher stays a plain grid (the search field only appears if there's at least
one provider).

```ts
activate(ctx) {
  ctx.launcher.provider({
    onQuery: async (q) => {
      if (!q) return [];
      const hits = await search(q);                   // sync or async (fetch, compute…)
      return hits.map((h) => ({
        id: h.id,
        title: h.name,
        subtitle: h.detail,
        icon: "<svg…>",                                // optional (default icon otherwise)
        onActivate: () => open(h),                     // Enter or click
      }));
    },
  });
}
// automatic cleanup on deactivation; or ctx.launcher.removeProvider()
```

Results from **all** active providers are merged. `Enter` activates the first result. One
extension = one provider.

## Communicating between extensions (bus)

A "producer" extension publishes an event; others subscribe — without knowing each other.
Prefix your channels to avoid collisions.

```ts
// Producer (e.g. a player):
ctx.bus.emit("nowplaying:update", { title, artist });

// Consumer (e.g. a widget):
const off = ctx.bus.on("nowplaying:update", (p) => show(p));
// `off()` to unsubscribe; otherwise cleaned up automatically on deactivation.
```

## Read out loud / type text

```ts
ctx.speak("Build done");                    // text-to-speech (no permission)
await ctx.input.typeText("Hello 👋");        // types into the active app (⚠ perm `input`)
```

## Notify the end of a task

```ts
ctx.notify({
  title: "Recording finished", body: "0:42", color: "#ff453a",
  source: "Capture", onClick: () => island.invoke("reveal_path", { path }),  // opens the file
});
```

`reveal_path` (reveal in the file explorer) is not gated: it's an app command.
