# Solitarite implementation notes

## Validation status — please read

Do not build or run Cargo commands locally. Formatting, tests, and release
builds are verified through the GitHub Actions workflows; until those checks
pass for the current revision, validation is **pending**. The pure logic
(`motion.rs`, `perf.rs`, audio synthesis, and settings migration) includes unit
tests. Likely GPUI API trouble spots are listed under "Things to check first"
below.

## Card motion (`src/motion.rs`, `src/ui.rs`)

Motion is built on one idea: **diff the layout before and after any change.**

1. Before an action (move, draw, recycle, undo, redo, auto-finish step, new
   deal, drop) the UI calls `pose_snapshot()`, which computes where every card
   is drawn from the `Game` and the current `Layout`.
2. The action mutates the `Game` as before.
3. `animate_from()` snapshots again and `motion::plan()` turns every card that
   changed position or face into a `Flight`.
4. Flights are drawn in an overlay on top of the board while the resting copy of
   that card is hidden (opacity 0). When the flight ends the resting copy is
   revealed at exactly the same pixel position, so there is no visible hand-off.

Because the diff works from layout alone, *every* feature gets smooth motion
without per-action animation code: auto-play cascades, undo/redo, Draw-3 fan
shifts, waste recycling, hints that cause moves, and auto-finish.

`Layout::compute` is the single source of truth for board geometry; `render_board`
uses it too, so snapshots and rendering cannot drift apart. `poses()` mirrors the
tableau offset accumulation used when piles render.

Flight details (`Flight::frame`):

* **Easing** — Travel blends ease-out cubic with smoothstep (quick launch, soft
  landing); Deal and Settle use ease-out cubic; Return uses ease-out-back for a
  small spring-back overshoot.
* **Arc and shadow** — Travel and Deal lift on a sine arc proportional to
  distance (capped), with the larger shadow while airborne.
* **Staggering** — big movers are staggered in iteration order (55 ms for a few
  cards, 24 ms for medium groups, 12 ms for large ones, capped to ~380 ms total);
  small fan shifts move immediately.
* **Deal** — `deal_plan` flies the 28 tableau cards out of the stock row by row,
  flipping the top card of each column.
* **Flip** — for cards that change face, `flip_card` rebuilds the card face (or
  back) at a foreshortened width each frame, darkening as it turns edge-on and
  growing slightly in height at mid-turn. Cards that flip in place (a revealed
  tableau card) wait ~110 ms so the card covering them can leave first.

Non-flipping flights build their card element once; only flipping flights
rebuild children per frame, which keeps per-frame work low.

## Drag and drop

* `on_drag` registers an `ActiveDrag` on the app (via the entity handle) so the
  source cards are hidden while held and every legal destination (`DragCards.valid`)
  shows a glow where the card would land.
* `DragGhost` animates a 130 ms "pick up": it swells about the exact point the
  mouse grabbed and deepens its shadow.
* On release over a pile the card flies on from the **ghost's position**
  (`ghost_origin` converts window to board coordinates via `Layout::board_origin`)
  using the short `Settle` style. A rejected drop, or a drop on empty felt
  (caught by an `on_drop` on the window root), springs back with `Return`.
* `self.drag.take()` guards against a drop being handled twice, and a mouse-down
  on the window clears any stale drag.

## Audio (`src/audio.rs`)

All effects are synthesized once into PCM buffers on the audio thread (filtered
noise "paper" textures, pitch sweeps for snaps, decaying sine bells, a riffle and
a victory arpeggio) and mixed concurrently with `OutputStreamHandle::play_raw`, so
quick sequences overlap instead of queueing. Each play applies the master volume
(perceptual curve) and a small random pitch variation. UI code only enqueues a
message; with no audio device everything silently does nothing. Impact sounds are
scheduled for when a card **lands**, not when it is clicked.

Settings gained `volume` (default 0.8) and `animations` (default on); older save
files load with these defaults.

## Performance (`src/perf.rs`)

`F3` shows a live overlay; `SOLITARITE_PROFILE=1` logs a summary line to stderr
about once a second during animation. See `PROFILING.md` for GPU-level tools.

Known characteristic: GPUI rebuilds this view's element tree on every animation
frame, so the cost during an animation is proportional to the number of elements
in the board. Resting cards no longer carry per-card animations, which removes the
largest source of continuous work compared with the previous version.

## Things to check first when compiling

1. `Div::opacity`, `shadow_lg`, `rounded`, `when` usage inside `with_animation`
   closures (patterns match the existing toast/confetti code).
2. The `on_drag` constructor calling `Entity::update(cx, ..)` to register the
   active drag, and `window.mouse_position()` in `on_drop` listeners.
3. `OutputStreamHandle::play_raw` and `SamplesBuffer::new` in rodio 0.20.
4. Root-level `.on_drop(..)` and `.on_mouse_down(..)` on the keyed root div.
5. If a drop on a pile also reaches the root handler in your GPUI version, the
   `self.drag.take()` guard makes the second call a no-op.

## Linux dependencies

Linux CI and release builds need ALSA development headers (`libasound2-dev`) in
addition to the GPUI dependencies. The workflows run tests before building the
release binary.
