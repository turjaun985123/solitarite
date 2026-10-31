# Profiling Solitarite

Solitarite has two layers of profiling: a built-in frame profiler for CPU-side
smoothness, and external tools for the GPU.

## 1. Built-in overlay (F3)

Press `F3` (or **Options → Performance Overlay**). GPUI only redraws while
something moves, so numbers appear during animation bursts: deal a new game,
draw cards, or drag a stack and watch the overlay.

| Field | Meaning | Healthy |
| --- | --- | --- |
| fps | frames per second over the current burst | ≈ display refresh (60/120) |
| frame avg / p95 / max | time between frames | avg ≈ 16.7 ms @ 60 Hz; p95 close to avg |
| build avg / max | CPU time `render` spent building the element tree | well under 4 ms |
| dropped frames | intervals over 25 ms since the burst began | 0 |
| flights | cards currently animating | — |

If **frame** times are high but **build** is small, the time is going to layout,
paint or the GPU — move to the external tools below. If **build** is high, the
element tree is too heavy: look at how many elements the board produces.

Log mode: run a CI-built release binary with `SOLITARITE_PROFILE=1` in its
environment. It prints one summary line per second of continuous animation to
stderr. For example, on Linux or macOS, run `SOLITARITE_PROFILE=1 ./solitarite`
from the directory containing the extracted binary.

## 2. A repeatable benchmark

1. Download and run a release binary produced by GitHub Actions, then press
   `F3`.
2. Start a new deal (`N`) and read the numbers while the deal plays (the heaviest
   animation: 28 simultaneous flights with 7 flips).
3. Recycle the waste (`Space` on an empty stock) for the second heaviest.
4. Compare before/after any change to `motion.rs` or the board rendering.

Use a release binary for profiling; debug builds are far slower.

## 3. GPU-level tools (external)

The in-app overlay cannot measure GPU time. Typical options:

* **macOS (Metal):** launch the release binary with `MTL_HUD_ENABLED=1` for the
  Metal performance HUD, or capture a frame in Xcode (Instruments → Metal System
  Trace / GPU Frame Capture).
* **Linux (Vulkan):** RenderDoc for frame captures; Mesa's overlay layer
  (`VK_INSTANCE_LAYERS=VK_LAYER_MESA_overlay`) for an fps/GPU-time HUD on supported
  drivers. `vulkaninfo`/`vkconfig` help confirm the layer is available.
* **Windows:** PIX on Windows, RenderDoc, or the GPU graphs in Task Manager /
  vendor tools (NVIDIA Nsight, AMD Radeon GPU Profiler).

Tool availability varies by driver and OS version; consult each tool's docs.

## 4. What to try if animation stutters

* Turn **Card Animations** off in Settings to confirm the cost is animation work.
* Lower the number of simultaneous flights by editing the stagger constants in
  `motion::plan` / `deal_plan`.
* Reduce per-frame rebuilding: only flipping flights rebuild children each frame;
  keep it that way.
* Check the window size: a very large window means larger cards and more pixels.
