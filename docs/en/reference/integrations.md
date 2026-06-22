# Incoming integrations & real-time (plan, not implemented)

> Captures the design of two complementary primitives: `island.serve` (an **incoming** local
> server) and the **real-time** client (WebSocket/SSE, **outgoing**). Motivated by the
> "integrate Claude Code (terminal) / an AI agent" use case in an extension.

## The triggering need: driving a local AI agent (e.g. Claude Code)

An extension that:
- **listens to the processes** of an agent (several Claude Code instances in parallel);
- **notifies** the user when the agent is **done**;
- shows the **list** of running processes/sessions;
- when the agent asks a **question / permission request (multiple choice)**, mounts a
  **visual UI** to answer, and **sends** the answer back to the agent.

Generic: "Claude" = any tool/AI that can `curl localhost`.

## The key point: the DIRECTION of the flow

- **`island.serve` (INCOMING)** — Island hosts a small **local server**; local processes
  **push** to it and **read its response**. → that's what's needed for Claude Code (the agent
  emits events, Island responds).
- **Real-time WS/SSE client (OUTGOING)** — the extension **connects** to a **remote** server
  to consume a stream. → useful for **something else** (see below), not for listening to local
  processes.

> ⚠️ Don't conflate them: for Claude Code, a WS/SSE client is NOT the right tool. It's
> `island.serve` (incoming) that unblocks this case.

## Claude Code architecture (via *hooks*)

Claude Code runs a command on events (`settings.json` → `hooks`). The command does a `curl`
to Island's local server:

| Claude Code hook | Island (`serve` handler) |
|---|---|
| `SessionStart` | register the session (folder, id) → **process list** |
| `Stop` (response finished) | **notification** "done" |
| `Notification` (needs attention) | banner |
| `PreToolUse` (permission/tool) | the hook **POSTs** the request then **waits**/polls for the answer → a **choice UI** in Island → the hook returns the decision (allow/deny) |

The subtle part = "answering the agent": the hook **blocks** and **polls** Island until the
user's response (doable; it's the trickiest piece to wire).

## `island.serve` — design (security first)

Single host-side local HTTP server, **one** for all extensions, routed per extension.
Safeguards:
- **Loopback only** (`127.0.0.1`), never exposed on the network.
- **Per-extension token** (generated on `serve.start`, required in query/header) → another
  local process can't call blindly.
- **`Host` check** (anti DNS-rebinding).
- **`serve`** permission (⚠ opens a local port) declared in the manifest.
- **Cleanup**: route removed when the extension deactivates.

Envisioned SDK API:
```ts
const ep = await ctx.serve.start();              // { port, token, url } ; opens the route
ctx.serve.on(async (req) => {                    // req = { method, path, query, body }
  // …show a notification / a view, wait for a user response…
  return { status: 200, body: { decision: "allow" } };
});
ctx.serve.stop();
```

## Real-time WS/SSE client — design (outgoing)

`network` permission (reuses the existing one). What it's for (≠ Claude Code):
- **Streaming an AI API directly** (Claude/OpenAI return **SSE**) → show the response
  token-by-token in a view.
- Crypto/stock tickers, scores, **chat** (Discord/Slack via WS), presence, live dashboards,
  notifications from a web service.

Envisioned SDK API:
```ts
const sock = ctx.realtime.ws(url, { headers });
sock.onMessage((data) => …); sock.send("…"); sock.close();
const es = ctx.realtime.sse(url);   // SSE stream → es.onMessage(...)
```
Host side: native connection (outside the webview → no CORS/origin issue), handled on a
thread/loop, events relayed to the front via a Tauri `event`.

## Priorities

1. **`island.serve`** — unblocks the Claude Code / local-agent integration; the most
   strategic primitive (Island = an **integration hub** for all local tools).
2. **WS/SSE client** — for extensions consuming a remote stream (streaming AI API, live data).

The two are complementary: a single "AI copilot" extension could use `serve` (receive Claude
Code events) **and** WS/SSE (stream a remote AI API's response).
