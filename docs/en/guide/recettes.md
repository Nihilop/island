# Recipes

Short snippets, taken from the GitHub extension (`examples/extensions/github`).

## Refresh in the background at a configurable interval

```ts
let timer: ReturnType<typeof setTimeout> | undefined;

async function refresh() {
  clearTimeout(timer);
  // … work …
  const minutes = Number(await island.settings.get<string>("interval")) || 5;
  timer = setTimeout(refresh, minutes * 60_000);
}

island.settings.onChange(() => void refresh());
island.commands.on("refresh", () => void refresh());
await refresh();
```

## Call an API with a token

```ts
const token = await island.settings.get<string | null>("token"); // "secret"-type setting
if (!token) return "noToken";
const prs = await island.http.json<{ items: unknown[] }>(
  `https://api.github.com/search/issues?q=${encodeURIComponent("is:open is:pr author:@me")}`,
  { headers: { Authorization: `Bearer ${token}` } },
);
```

Manifest: `"permissions": ["network:api.github.com"]` and a setting
`{ "key": "token", "type": "secret" }`.

## Share state between the logic and the view

The logic and the view run in two separate iframes: go through storage.

```ts
// logic
await island.storage.set("snapshot", { reviews, mine, at: Date.now() });

// view: show the last known state right away, then refresh
let snap = await island.storage.get<Snapshot>("snapshot");
render();
```

## Notify only about new items

```ts
const known = new Set(before.reviews.map((p) => p.id));
for (const pr of now.reviews.filter((p) => !known.has(p.id))) {
  await island.notify({ title: island.t("notify.review", { repo: pr.repo }), body: pr.title });
}
```

## Live activity based on state

```ts
if (failed) return island.live.set("status", { icon: "icons/ci.svg", text: String(failed), color: "red", priority: "high" });
if (reviews) return island.live.set("status", { icon: "icons/review.svg", text: String(reviews), color: "orange" });
return island.live.set("status", null);
```

## Cached launcher results

```ts
island.launcher.provide("repos", async (q) => {
  const list = await repos(island); // cached for 1 h in island.storage
  return list
    .filter((r) => r.name.toLowerCase().includes(q.toLowerCase()))
    .slice(0, 20)
    .map((r) => ({ id: r.url, title: r.name, subtitle: r.description ?? undefined, icon: "icons/github.svg" }));
});
island.launcher.onRun("repos", (url) => void island.openUrl(url));
```

## Follow the theme in a view

With `@island/sdk/ui.css`, nothing to do. For your own colors:

```css
:root { --accent: #fff; }
:root.light { --accent: #000; }
```
