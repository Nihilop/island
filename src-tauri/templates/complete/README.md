# __EXT_NAME__

Extension Island générée depuis le **template complet**. Elle montre les briques
principales du SDK :

- **launcher** — une entrée qui ouvre la view (`index.ts`).
- **view** — surface montée dans l'île (`View.vue`).
- **config** — surface de réglages ouverte en modal (`Config.vue`).
- **storage** — compteur persistant entre les sessions (`store.ts`).
- **idle** — le compteur s'affiche sur l'extrémité droite de l'île au repos.
- **notify** — une bannière à chaque incrément (désactivable dans les réglages).

## Développer

```bash
pnpm install
pnpm dev        # = vite build --watch (rebuild auto du dist/)
```

Puis active l'extension dans Island (tray → Réglages → Extensions). En dev
(`pnpm tauri dev`), le `dist/` est live-reloadé à chaque rebuild.

## Build de prod

```bash
pnpm build      # produit dist/index.mjs + dist/style.css
```

Voir la doc complète du contrat d'extension : `docs/authoring-extensions.md` du repo Island.
