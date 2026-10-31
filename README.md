# Solitarite

A polished, full-featured Klondike solitaire game written in Rust with
[GPUI](https://crates.io/crates/gpui), the GPU-accelerated UI framework from
Zed.

Solitarite aims to match the everyday feature set of Microsoft Solitaire
Collection's Klondike: draw one or three, Standard or Vegas scoring, timed
play, undo/redo, hints, auto-finish, statistics, daily deals, resumable games,
themes, card backs, and a keyboard-friendly interface.

## Features

- **Classic Klondike** — seven tableau piles, four foundations, stock and waste.
- **Draw 1 / Draw 3** — switch rules at any time; changes apply immediately to
  an untouched deal or to the next deal after play has begun.
- **Standard scoring** — classic bonus/penalty scoring, including the time
  penalty after 30 seconds and the win bonus.
- **Vegas scoring** — start at −$52 and earn $5 for every card sent home.
- **Timed games** — optional clock and time-based scoring.
- **Undo / redo** — full board snapshots, including score, move count and
  draw-pass state.
- **Hints** — highlights a useful source and destination, or tells you to draw
  or recycle.
- **Auto-finish** — animates the remaining legal foundation moves once every
  tableau card is face up.
- **Daily Deal** — the same deal for everyone on a given local date, derived
  from the date itself.
- **Save & resume** — the current deal, settings and statistics are saved
  automatically and restored on launch.
- **Statistics** — games played/won, win rate, streaks, best time, fewest
  moves, best Standard/Vegas scores and total time won.
- **Themes & card backs** — Green Felt, Midnight and Burgundy themes with four
  card-back designs.
- **Cinematic card motion** — every card physically travels between piles on an
  eased, slightly arced path with a lifted shadow; a real deal from the stock at
  the start of each game; staggered runs; and animated stock-to-waste draws and
  recycles. Cards turn over with a foreshortened 3D-style flip.
- **Tactile drag and drop** — a lifted card swells under your cursor, the
  source stays empty while you hold it, every legal destination glows, and the
  card settles smoothly into place on release (or springs back if rejected).
- **Layered sound design** — synthesized card slides, snaps, flicks, a riffle
  shuffle, a foundation bell and a victory fanfare, with pitch variation, sounds
  timed to the moment a card lands, and a master volume control.
- **Performance overlay** — press `F3` for live fps, frame-time percentiles,
  render build cost and dropped-frame counts (see `PROFILING.md`).
- **Quality-of-life** — double-click or right-click a card to send it to a
  foundation, auto-play to foundations, pause, toast messages, victory
  confetti and responsive layout. Sound effects can be muted in Settings and
  gracefully fall back to silence when no audio device is available.

## Controls

| Action | Shortcut |
| --- | --- |
| New deal | `F2` / `N` / `Ctrl+N` |
| Daily deal | `D` |
| Restart deal | `R` |
| Undo | `Ctrl+Z` |
| Redo | `Ctrl+Y` / `Ctrl+Shift+Z` |
| Hint | `H` |
| Auto-finish | `A` |
| Pause / resume | `P` |
| Statistics | `S` |
| Settings | `Ctrl+Shift+S` |
| Toggle four-color deck | `C` |
| Help | `F1` |
| Performance overlay | `F3` |
| Draw from stock | `Space` |
| Dismiss menu / modal / selection | `Esc` |
| Quit | `Ctrl+Q` |

Mouse controls:

- Click a face-up card to select it, then click a destination to move it.
- Drag a card or a valid run to a destination.
- Double-click or right-click a card to send it to a foundation.
- Click the stock to draw; click the empty stock outline to recycle the waste.
- Use the in-game **Game**, **Options** and **Help** menus for everything else.

## Download and run

Prebuilt binaries for Linux and macOS are attached to each
[GitHub release](https://github.com/turjaun985123/solitarite/releases/latest).
Download the archive for your operating system, extract it, and run the
`solitarite` executable. On Linux, the system
libraries required by GPUI and your graphics stack must be installed. New
releases are built when a `v*` version tag is pushed.

## Development and verification

Builds and tests run through [GitHub Actions](.github/workflows/ci.yml). The CI
workflow checks formatting, runs the test suite, and builds release binaries on
Linux and macOS. Pull requests and pushes must pass CI before changes
are considered verified. The release workflow repeats tests before packaging
tagged builds.

## Audio and motion

Sound effects are enabled by default; mute them or set the volume in
**Settings**. Card animations can be switched off there (or in the Options menu)
for instant moves. Playback is best-effort and uses a single background audio
worker that mixes overlapping effects; systems without an audio output device
continue silently. See `IMPLEMENTATION_NOTES.md` for how card flights, flips and
the sound engine work, and `PROFILING.md` for measuring rendering performance.

## Save data

The game is saved as JSON at:

- `$SOLITARITE_DATA_DIR/save.json` if `SOLITARITE_DATA_DIR` is set, otherwise
- `$XDG_DATA_HOME/solitarite/save.json` if `XDG_DATA_HOME` is set, otherwise
- `~/.local/share/solitarite/save.json`.

Saves are written atomically via a temporary file. A corrupt save is moved
aside to `save.json.corrupt` and the game starts fresh. The current deal is
saved every few seconds and after every action; completed games are cleared
from the save so the next launch starts a new deal.

## Project layout

| File | Purpose |
| --- | --- |
| `src/model.rs` | Cards, rules, moves, scoring, undo/redo, hints, auto-finish, tests |
| `src/cards.rs` | Card faces, card backs and the drag ghost |
| `src/persistence.rs` | Settings, statistics, save/load and data-directory handling |
| `src/motion.rs` | Board layout, per-card poses, flight planning and easing (pure, unit-tested) |
| `src/audio.rs` | Synthesized, layered sound effects and the mixing worker |
| `src/perf.rs` | Frame-time profiler behind the `F3` overlay |
| `src/theme.rs` | Theme palettes and color helpers |
| `src/ui.rs` | Application view, board, menus, modals, animations and input |
| `src/main.rs` | Window creation, key bindings and app lifecycle |

## Notes

- The deal is deterministic from its seed, so a given game number always deals
  the same cards.
- Standard scoring never drops below zero; Vegas scores can go negative.
- Undo and redo are disabled after a win.
- Auto-finish draws through the stock automatically when needed and stops with
  a message if it cannot make progress.

## License

MIT
