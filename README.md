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
- **Quality-of-life** — double-click or right-click a card to send it to a
  foundation, auto-play to foundations, pause, toast messages, victory
  confetti and responsive layout.

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
| Help | `F1` |
| Draw from stock | `Space` |
| Dismiss menu / modal / selection | `Esc` |
| Quit | `Ctrl+Q` |

Mouse controls:

- Click a face-up card to select it, then click a destination to move it.
- Drag a card or a valid run to a destination.
- Double-click or right-click a card to send it to a foundation.
- Click the stock to draw; click the empty stock outline to recycle the waste.
- Use the in-game **Game**, **Options** and **Help** menus for everything else.

## Build and run

Solitarite requires a Rust toolchain with support for edition 2024 (Rust 1.85
or newer) and the usual GPUI Linux build dependencies.

```bash
cargo run --release
```

For development:

```bash
cargo check
cargo test
cargo run
```

### Download a release

Prebuilt binaries for Linux, macOS and Windows are attached to each
[GitHub release](https://github.com/turjaun985123/solitarite/releases/latest).
Download the archive for your operating system, extract it, and run the
`solitarite` executable (`solitarite.exe` on Windows). On Linux, the system
libraries listed below are also needed at runtime. New releases are built when
a `v*` version tag is pushed.

### Linux dependencies

On Debian/Ubuntu, GPUI needs development headers for X11/Wayland, fontconfig,
FreeType, and the graphics stack. A typical set is:

```bash
sudo apt-get install \
  build-essential pkg-config \
  libx11-dev libxkbcommon-dev libwayland-dev \
  libfontconfig1-dev libfreetype6-dev \
  libvulkan-dev libgl1-mesa-dev libxcb1-dev
```

GPUI also needs a working display server. For a headless smoke test:

```bash
xvfb-run -a cargo run
```

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
