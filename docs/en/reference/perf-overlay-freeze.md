# Perf note — the island "freezes" background video

**Symptom**: each time the island opens (native launcher, Flow, Spotify, any view), a video
playing in the background freezes for ~1 frame then resumes a few seconds later.

## Cause (very likely): hardware overlay plane switch (MPO / DirectFlip)

The video plays via a **GPU hardware overlay plane** (Multiplane Overlay). When the island
appears/grows/changes on top, **DWM pulls the video out of its overlay plane** to compose
everything → micro-freeze, then re-promotion to overlay once composition stabilizes → hence
"freeze then resume". This is Windows/driver behavior, not an app bug (Discord/Afterburner
cause it too).

## Triggers in OUR code (DWM recomposition)

1. **Fullscreen, transparent, `alwaysOnTop`, layered, always-visible overlay**
   (`cover_monitor` + tauri.conf). Worst case for MPO: DWM constantly re-evaluates the video's
   overlay plane below.
2. **`set_ignore_cursor_events(true/false)`** (click-through, `start_click_through` in lib.rs)
   toggles `WS_EX_TRANSPARENT` on every cursor enter/leave → a window-style change =
   recomposition.
3. **Island growth** (pill → large view) = a big surface change.
4. **`set_focus()`** (Flow's `overlay_focus`) steals focus from the video app (secondary, Flow
   only).

## Diagnosis

- `HKLM\SOFTWARE\Microsoft\Windows\Dwm\OverlayTestMode` (DWORD) = `5`, reboot → disables MPO
  globally. Freeze gone = MPO confirmed. (Remove afterwards.)
- Disable hardware acceleration in the player → if it stops, it's the GPU overlay path.

## Leads (impact / effort)

- **A. Shrink the overlay window** (big lever): window sized to the island (repositioned/grown
  on demand) instead of fullscreen. The rest of the screen no longer has a layered topmost
  window → the video keeps its overlay plane in passive use. Trade-off: grow temporarily for
  region-capture / modal backdrop / notification stack. — **This is the approach that was
  shipped** (box overlay, fullscreen on demand).
- **B. Remove style churn**: replace `set_ignore_cursor_events` with `WM_NCHITTEST` hit-testing
  (WndProc → `HTTRANSPARENT` / `HTCLIENT`). Stable extended style → far fewer recompositions.
- **C. Don't steal focus** (Flow): keyboard input without `set_focus()` foreground.
- **D. `DwmSetWindowAttribute`**: low hope for MPO, low priority.
