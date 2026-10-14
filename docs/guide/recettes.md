# Recettes

Des extraits courts, tirés de l'extension GitHub (`examples/extensions/github`).

## Rafraîchir en fond à intervalle réglable

```ts
let timer: ReturnType<typeof setTimeout> | undefined;

async function refresh() {
  clearTimeout(timer);
  // … travail …
  const minutes = Number(await island.settings.get<string>("interval")) || 5;
  timer = setTimeout(refresh, minutes * 60_000);
}

island.settings.onChange(() => void refresh());
island.commands.on("refresh", () => void refresh());
await refresh();
```

## Appeler une API avec un jeton

```ts
const token = await island.settings.get<string | null>("token"); // réglage de type "secret"
if (!token) return "noToken";
const prs = await island.http.json<{ items: unknown[] }>(
  `https://api.github.com/search/issues?q=${encodeURIComponent("is:open is:pr author:@me")}`,
  { headers: { Authorization: `Bearer ${token}` } },
);
```

Manifeste : `"permissions": ["network:api.github.com"]` et un réglage
`{ "key": "token", "type": "secret" }`.

## Partager l'état entre la logique et la vue

La logique et la vue tournent dans deux iframes distinctes : passez par le stockage.

```ts
// logique
await island.storage.set("snapshot", { reviews, mine, at: Date.now() });

// vue : affiche tout de suite le dernier état connu, puis rafraîchit
let snap = await island.storage.get<Snapshot>("snapshot");
render();
```

## Notifier seulement les nouveautés

```ts
const known = new Set(before.reviews.map((p) => p.id));
for (const pr of now.reviews.filter((p) => !known.has(p.id))) {
  await island.notify({ title: island.t("notify.review", { repo: pr.repo }), body: pr.title });
}
```

## Activité en direct selon l'état

```ts
if (failed) return island.live.set("status", { icon: "icons/ci.svg", text: String(failed), color: "red", priority: "high" });
if (reviews) return island.live.set("status", { icon: "icons/review.svg", text: String(reviews), color: "orange" });
return island.live.set("status", null);
```

## Résultats du launcher avec cache

```ts
island.launcher.provide("repos", async (q) => {
  const list = await repos(island); // mis en cache 1 h dans island.storage
  return list
    .filter((r) => r.name.toLowerCase().includes(q.toLowerCase()))
    .slice(0, 20)
    .map((r) => ({ id: r.url, title: r.name, subtitle: r.description ?? undefined, icon: "icons/github.svg" }));
});
island.launcher.onRun("repos", (url) => void island.openUrl(url));
```

## Suivre le thème dans une vue

Avec `@island/sdk/ui.css`, rien à faire. Pour vos propres couleurs :

```css
:root { --accent: #fff; }
:root.light { --accent: #000; }
```
