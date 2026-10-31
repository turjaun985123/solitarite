//! Solitarite — a full-featured Klondike solitaire game built with GPUI.

mod audio;
mod cards;
mod model;
mod motion;
mod perf;
mod persistence;
mod theme;
mod ui;

use gpui::{
    App, AppContext as _, Application, Bounds, Focusable as _, KeyBinding, WindowBounds,
    WindowDecorations, WindowOptions, px, size,
};

use ui::{
    AutoFinish, DailyDeal, Dismiss, DrawCards, NewGame, PauseGame, QuitApp, RedoMove, RestartDeal,
    ShowHelp, ShowHint, ShowSettings, ShowStats, SolitaireApp, TogglePerf, UndoMove,
};

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("f2", NewGame, Some("Solitaire")),
            KeyBinding::new("ctrl-n", NewGame, Some("Solitaire")),
            KeyBinding::new("n", NewGame, Some("Solitaire")),
            KeyBinding::new("d", DailyDeal, Some("Solitaire")),
            KeyBinding::new("r", RestartDeal, Some("Solitaire")),
            KeyBinding::new("ctrl-z", UndoMove, Some("Solitaire")),
            KeyBinding::new("z", UndoMove, Some("Solitaire")),
            KeyBinding::new("u", UndoMove, Some("Solitaire")),
            KeyBinding::new("ctrl-y", RedoMove, Some("Solitaire")),
            KeyBinding::new("ctrl-shift-z", RedoMove, Some("Solitaire")),
            KeyBinding::new("h", ShowHint, Some("Solitaire")),
            KeyBinding::new("a", AutoFinish, Some("Solitaire")),
            KeyBinding::new("p", PauseGame, Some("Solitaire")),
            KeyBinding::new("s", ShowStats, Some("Solitaire")),
            KeyBinding::new("ctrl-shift-s", ShowSettings, Some("Solitaire")),
            KeyBinding::new("f1", ShowHelp, Some("Solitaire")),
            KeyBinding::new("f3", TogglePerf, Some("Solitaire")),
            KeyBinding::new("space", DrawCards, Some("Solitaire")),
            KeyBinding::new("escape", Dismiss, Some("Solitaire")),
            KeyBinding::new("ctrl-q", QuitApp, None),
        ]);

        let bounds = Bounds::centered(None, size(px(1120.0), px(760.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(820.0), px(600.0))),
                    is_movable: true,
                    is_resizable: true,
                    is_minimizable: true,
                    // Draw our own titlebar and window controls on compositors
                    // that support client-side decorations. GPUI falls back to
                    // server decorations automatically when they are unavailable.
                    window_decorations: Some(WindowDecorations::Client),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("Solitarite — Klondike Solitaire".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| SolitaireApp::new(window, cx)),
            )
            .expect("failed to open the Solitarite window");

        let app = window
            .update(cx, |_, _, cx| cx.entity())
            .expect("failed to access the Solitarite app");

        let focus = app.read(cx).focus_handle(cx);
        window
            .update(cx, |_, window, cx| {
                window.focus(&focus);

                let app = app.clone();
                window.on_window_should_close(cx, move |_window, cx| {
                    app.update(cx, |app, cx| {
                        app.save_now();
                        cx.notify();
                    });
                    true
                });

                cx.activate(true);
            })
            .expect("failed to initialize the Solitarite window");
    });
}
