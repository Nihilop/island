# Note perf — l'île fait « freezer » la vidéo en arrière-plan

**Symptôme** : à chaque ouverture de l'île (launcher natif, Flow, Spotify, toute
vue), une vidéo qui joue en arrière-plan se fige ~1 frame puis reprend quelques
secondes plus tard.

## Cause (très probable) : bascule de plan overlay matériel (MPO / DirectFlip)

La vidéo joue via un **plan d'overlay matériel GPU** (Multiplane Overlay). Quand
l'île apparaît/grandit/change par-dessus, **DWM retire la vidéo de son plan overlay**
pour tout composer → micro-freeze, puis re-promotion en overlay une fois la
composition stabilisée → d'où « fige puis reprend ». C'est un comportement
Windows/driver, pas un bug applicatif (Discord/Afterburner le provoquent aussi).

## Déclencheurs dans NOTRE code (recomposition DWM)

1. **Overlay plein écran, transparent, `alwaysOnTop`, layered, toujours visible**
   (`cover_monitor` + tauri.conf). Pire cas pour MPO : DWM réévalue sans cesse le
   plan overlay de la vidéo dessous.
2. **`set_ignore_cursor_events(true/false)`** (click-through, `start_click_through`
   dans lib.rs) bascule `WS_EX_TRANSPARENT` à chaque entrée/sortie du curseur →
   changement de style fenêtre = recomposition.
3. **Croissance de l'île** (pill → grande vue) = gros changement de surface.
4. **`set_focus()`** (`overlay_focus` de Flow) vole le focus à l'app vidéo
   (secondaire, Flow uniquement).

## Diagnostic

- `HKLM\SOFTWARE\Microsoft\Windows\Dwm\OverlayTestMode` (DWORD) = `5`, reboot →
  désactive MPO globalement. Freeze disparaît = MPO confirmé. (Supprimer après.)
- Couper l'accélération matérielle dans le lecteur → si ça stoppe, c'est le chemin
  overlay GPU.

## Pistes (impact / effort) — NON implémentées

- **A. Réduire la fenêtre overlay** (gros levier) : window dimensionné à l'île
  (repositionné/agrandi à la demande) au lieu de plein écran. Le reste de l'écran
  n'a plus de window topmost layered → la vidéo garde son plan overlay en usage
  passif. Compromis : agrandir ponctuellement pour région-capture / backdrop modal /
  pile de notifs.
- **B. Supprimer le churn de style** : remplacer `set_ignore_cursor_events` par du
  hit-testing `WM_NCHITTEST` (WndProc → `HTTRANSPARENT` / `HTCLIENT`). Style étendu
  stable → bien moins de recompositions.
- **C. Ne pas voler le focus** (Flow) : saisie clavier sans `set_focus()` foreground.
- **D. `DwmSetWindowAttribute`** : faible espoir MPO, basse priorité.

Objectif réaliste : A + B → zéro freeze pendant le simple visionnage, freeze
résiduel court seulement lors d'actions plein écran.
