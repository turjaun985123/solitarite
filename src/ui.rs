use std::time::Duration;

use chrono::{Datelike, Local};
use gpui::{
    Animation, AnimationExt as _, App, ClickEvent, Context, CursorStyle, Div, ElementId,
    FocusHandle, Focusable, FontWeight, MouseButton, MouseDownEvent, Render, SharedString,
    Stateful, Window, actions, div, linear_color_stop, linear_gradient, prelude::*, px, rgb, rgba,
};

use crate::cards::{DragGhost, card_back, card_label, card_view};
use crate::model::{Card, Game, Hint, PileRef, Rules, Scoring, format_score, format_time};
use crate::persistence::{self, CardBack, SaveData, Settings, Stats, Theme, save_path_display};
use crate::theme::{Fade as _, Palette, card_red, gold, overlay};

actions!(
    solitarite,
    [
        NewGame,
        RestartDeal,
        DailyDeal,
        UndoMove,
        RedoMove,
        ShowHint,
        AutoFinish,
        PauseGame,
        ShowStats,
        ShowSettings,
        ShowHelp,
        DrawCards,
        Dismiss,
        QuitApp,
    ]
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuKind {
    Game,
    Options,
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NewKind {
    Random,
    Daily,
    Restart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Modal {
    Settings,
    Stats,
    Help,
    About,
    ConfirmNew(NewKind),
    ConfirmResetStats,
    Pause,
    Victory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Selection {
    source: PileRef,
    index: usize,
}

#[derive(Clone)]
struct DragCards {
    source: PileRef,
    index: usize,
    cards: Vec<Card>,
}

pub struct SolitaireApp {
    focus_handle: FocusHandle,
    settings: Settings,
    stats: Stats,
    game: Game,
    selected: Option<Selection>,
    hint: Option<Hint>,
    menu: Option<MenuKind>,
    modal: Option<Modal>,
    toast: Option<SharedString>,
    toast_gen: u64,
    auto_finishing: bool,
    deal_serial: u64,
    win_recorded: bool,
}

impl SolitaireApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let data = persistence::load();
        let mut app = Self {
            focus_handle: cx.focus_handle(),
            settings: data.settings,
            stats: data.stats,
            game: Game::new(0, Rules::default(), false),
            selected: None,
            hint: None,
            menu: None,
            modal: None,
            toast: None,
            toast_gen: 0,
            auto_finishing: false,
            deal_serial: 0,
            win_recorded: false,
        };

        if let Some(game) = data.game.filter(|game| game.is_valid() && !game.won) {
            app.game = game;
            if app.game.paused {
                app.modal = Some(Modal::Pause);
            }
        } else {
            let seed = random_seed();
            app.game = Game::new(seed, app.settings.rules, false);
            app.stats.record_game_started();
            app.save_now();
        }

        window.set_window_title("Solitarite — Klondike Solitaire");
        app.spawn_ticker(cx);
        app
    }

    fn spawn_ticker(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |weak, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive = weak
                    .update(cx, |this, cx| {
                        if this.game.tick() {
                            if this.game.elapsed % 5 == 0 {
                                this.save_now();
                            }
                            cx.notify();
                        }
                    })
                    .is_ok();
                if !alive {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn save_now(&self) {
        let data = SaveData {
            version: persistence::SAVE_VERSION,
            settings: self.settings.clone(),
            stats: self.stats.clone(),
            game: if self.game.won {
                None
            } else {
                Some(self.game.clone())
            },
        };
        if let Err(error) = persistence::save(&data) {
            eprintln!("could not save game: {error}");
        }
    }

    fn interaction_enabled(&self) -> bool {
        !self.game.won && !self.game.paused && self.modal.is_none() && !self.auto_finishing
    }

    fn toggle_menu(&mut self, menu: MenuKind, cx: &mut Context<Self>) {
        self.menu = if self.menu == Some(menu) {
            None
        } else {
            Some(menu)
        };
        cx.notify();
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    fn request_new_game(&mut self, kind: NewKind, cx: &mut Context<Self>) {
        self.menu = None;
        self.auto_finishing = false;
        if self.game.moves > 0 && !self.game.won {
            self.modal = Some(Modal::ConfirmNew(kind));
            cx.notify();
        } else {
            self.start_new_game(kind, cx);
        }
    }

    fn start_new_game(&mut self, kind: NewKind, cx: &mut Context<Self>) {
        self.auto_finishing = false;
        self.menu = None;
        self.modal = None;
        self.selected = None;
        self.hint = None;
        self.win_recorded = false;
        self.deal_serial = self.deal_serial.wrapping_add(1);

        let toast = match kind {
            NewKind::Restart => {
                let seed = self.game.seed;
                let rules = self.game.rules;
                let daily = self.game.is_daily;
                self.game = Game::new(seed, rules, daily);
                "Deal restarted".to_string()
            }
            NewKind::Random => {
                self.stats.abandon(&self.game);
                let seed = random_seed();
                self.game = Game::new(seed, self.settings.rules, false);
                self.stats.record_game_started();
                format!("New deal · Game #{:06}", self.game.deal_number)
            }
            NewKind::Daily => {
                self.stats.abandon(&self.game);
                let seed = daily_seed();
                self.game = Game::new(seed, self.settings.rules, true);
                self.stats.record_game_started();
                format!("Daily Deal · {}", Local::now().format("%b %-d"))
            }
        };

        self.save_now();
        self.toast(toast, cx);
        cx.notify();
    }

    fn rules_changed(&mut self, cx: &mut Context<Self>) {
        if self.game.moves == 0 && !self.game.won {
            let seed = self.game.seed;
            let daily = self.game.is_daily;
            self.game = Game::new(seed, self.settings.rules, daily);
            self.selected = None;
            self.hint = None;
            self.win_recorded = false;
            self.deal_serial = self.deal_serial.wrapping_add(1);
            self.toast("Rules applied to this deal", cx);
        } else {
            self.toast("New rules apply to the next deal", cx);
        }
        self.save_now();
        cx.notify();
    }

    fn set_draw_three(&mut self, draw_three: bool, cx: &mut Context<Self>) {
        if self.settings.rules.draw_three == draw_three {
            return;
        }
        self.settings.rules.draw_three = draw_three;
        self.rules_changed(cx);
    }

    fn set_scoring(&mut self, scoring: Scoring, cx: &mut Context<Self>) {
        if self.settings.rules.scoring == scoring {
            return;
        }
        self.settings.rules.scoring = scoring;
        self.rules_changed(cx);
    }

    fn set_timed(&mut self, timed: bool, cx: &mut Context<Self>) {
        if self.settings.rules.timed == timed {
            return;
        }
        self.settings.rules.timed = timed;
        self.rules_changed(cx);
    }

    fn toggle_auto_play(&mut self, cx: &mut Context<Self>) {
        self.settings.auto_play = !self.settings.auto_play;
        self.save_now();
        let message = if self.settings.auto_play {
            "Auto-play enabled"
        } else {
            "Auto-play disabled"
        };
        self.toast(message, cx);
        cx.notify();
    }

    fn cycle_theme(&mut self, cx: &mut Context<Self>) {
        self.settings.theme = self.settings.theme.next();
        self.save_now();
        cx.notify();
    }

    fn cycle_card_back(&mut self, cx: &mut Context<Self>) {
        self.settings.card_back = self.settings.card_back.next();
        self.save_now();
        cx.notify();
    }

    fn draw(&mut self, cx: &mut Context<Self>) {
        if !self.interaction_enabled() || !self.game.can_draw() {
            return;
        }
        if self.game.draw() {
            self.selected = None;
            self.hint = None;
            self.after_action(cx);
        }
    }

    fn try_user_move(
        &mut self,
        source: PileRef,
        index: usize,
        dest: PileRef,
        cx: &mut Context<Self>,
    ) {
        if !self.interaction_enabled() {
            return;
        }
        if self
            .game
            .try_move(source, index, dest, self.settings.auto_play)
        {
            self.selected = None;
            self.hint = None;
            self.after_action(cx);
        } else {
            self.toast("That move is not allowed", cx);
        }
    }

    fn on_card_click(
        &mut self,
        source: PileRef,
        index: usize,
        event: &ClickEvent,
        cx: &mut Context<Self>,
    ) {
        if !self.interaction_enabled() {
            return;
        }

        let double_click = matches!(
            event,
            ClickEvent::Mouse(mouse) if mouse.down.click_count >= 2
        );
        if double_click {
            if let Some(dest) = self.game.foundation_destination(source, index) {
                self.try_user_move(source, index, dest, cx);
            }
            return;
        }

        let clicked = Selection { source, index };
        if self.selected == Some(clicked) {
            self.selected = None;
            self.hint = None;
            cx.notify();
            return;
        }

        if let Some(selected) = self.selected
            && let Some(dest) = destination_for_pile(source)
            && self.game.can_move(selected.source, selected.index, dest)
        {
            self.try_user_move(selected.source, selected.index, dest, cx);
            return;
        }

        if self.game.cards_at(source, index).is_some() {
            self.selected = Some(clicked);
            self.hint = None;
        } else {
            self.selected = None;
        }
        cx.notify();
    }

    fn on_card_right_click(&mut self, source: PileRef, index: usize, cx: &mut Context<Self>) {
        if !self.interaction_enabled() {
            return;
        }
        if let Some(dest) = self.game.foundation_destination(source, index) {
            self.try_user_move(source, index, dest, cx);
        } else {
            self.toast("No foundation move for that card", cx);
        }
    }

    fn on_drop(&mut self, drag: &DragCards, dest: PileRef, cx: &mut Context<Self>) {
        if !self.interaction_enabled() {
            return;
        }
        if self.game.can_move(drag.source, drag.index, dest) {
            self.try_user_move(drag.source, drag.index, dest, cx);
        } else if let Some(card) = drag.cards.first() {
            self.toast(format!("{} cannot go there", card_label(*card)), cx);
        }
    }

    fn on_empty_destination(&mut self, dest: PileRef, cx: &mut Context<Self>) {
        if !self.interaction_enabled() {
            return;
        }
        let Some(selected) = self.selected else {
            return;
        };
        if self.game.can_move(selected.source, selected.index, dest) {
            self.try_user_move(selected.source, selected.index, dest, cx);
        } else {
            self.selected = None;
            self.hint = None;
            self.toast("That card cannot be placed there", cx);
        }
    }

    fn after_action(&mut self, cx: &mut Context<Self>) {
        self.record_win_if_needed(cx);
        self.save_now();
        cx.notify();
    }

    fn record_win_if_needed(&mut self, cx: &mut Context<Self>) {
        if self.game.won && !self.win_recorded {
            self.win_recorded = true;
            self.stats.record_win(&self.game);
            self.auto_finishing = false;
            self.modal = Some(Modal::Victory);
            self.toast("You won!", cx);
        }
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        if !self.interaction_enabled() {
            return;
        }
        self.auto_finishing = false;
        if self.game.undo() {
            self.selected = None;
            self.hint = None;
            self.save_now();
            cx.notify();
        }
    }

    fn redo(&mut self, cx: &mut Context<Self>) {
        if !self.interaction_enabled() {
            return;
        }
        self.auto_finishing = false;
        if self.game.redo() {
            self.selected = None;
            self.hint = None;
            self.record_win_if_needed(cx);
            self.save_now();
            cx.notify();
        }
    }

    fn show_hint(&mut self, cx: &mut Context<Self>) {
        if !self.interaction_enabled() {
            return;
        }
        self.selected = None;
        match self.game.find_hint() {
            Some(hint) => {
                let message = hint_message(&self.game, hint);
                self.hint = Some(hint);
                self.toast(message, cx);
            }
            None => {
                self.hint = None;
                self.toast("No moves left — undo or start a new deal", cx);
            }
        }
        cx.notify();
    }

    fn start_auto_finish(&mut self, cx: &mut Context<Self>) {
        if !self.interaction_enabled() || !self.game.begin_auto_finish() {
            return;
        }
        self.auto_finishing = true;
        self.selected = None;
        self.hint = None;
        self.toast("Finishing the game…", cx);
        cx.notify();

        cx.spawn(async move |weak, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(95))
                    .await;
                let keep_going = weak.update(cx, |this, cx| {
                    if !this.auto_finishing {
                        return false;
                    }
                    if this.game.auto_finish_one() {
                        this.selected = None;
                        this.hint = None;
                        if this.game.won {
                            this.record_win_if_needed(cx);
                            this.save_now();
                            this.auto_finishing = false;
                            cx.notify();
                            return false;
                        }
                        cx.notify();
                        true
                    } else {
                        this.auto_finishing = false;
                        this.toast("Auto-finish is stuck — undo or keep playing", cx);
                        this.save_now();
                        cx.notify();
                        false
                    }
                });
                match keep_going {
                    Ok(true) => {}
                    _ => break,
                }
            }
        })
        .detach();
    }

    fn toggle_pause(&mut self, cx: &mut Context<Self>) {
        if self.game.won || self.modal.is_some() {
            return;
        }
        self.auto_finishing = false;
        self.game.toggle_pause();
        self.modal = if self.game.paused {
            Some(Modal::Pause)
        } else {
            None
        };
        self.selected = None;
        self.hint = None;
        self.save_now();
        cx.notify();
    }

    fn resume(&mut self, cx: &mut Context<Self>) {
        self.game.paused = false;
        self.modal = None;
        self.save_now();
        cx.notify();
    }

    fn open_modal(&mut self, modal: Modal, cx: &mut Context<Self>) {
        self.menu = None;
        self.auto_finishing = false;
        self.selected = None;
        self.hint = None;
        self.modal = Some(modal);
        cx.notify();
    }

    fn close_modal(&mut self, cx: &mut Context<Self>) {
        self.modal = None;
        if self.game.paused && !self.game.won {
            self.game.paused = false;
            self.save_now();
        }
        cx.notify();
    }

    fn confirm_new_game(&mut self, cx: &mut Context<Self>) {
        let Some(Modal::ConfirmNew(kind)) = self.modal else {
            return;
        };
        self.start_new_game(kind, cx);
    }

    fn confirm_reset_stats(&mut self, cx: &mut Context<Self>) {
        self.stats.reset();
        self.save_now();
        self.modal = Some(Modal::Stats);
        self.toast("Statistics reset", cx);
        cx.notify();
    }

    fn quit(&mut self, cx: &mut Context<Self>) {
        self.save_now();
        cx.quit();
    }

    fn toast(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.toast = Some(message.into());
        self.toast_gen = self.toast_gen.wrapping_add(1);
        let generation = self.toast_gen;
        cx.spawn(async move |weak, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(2800))
                .await;
            let _ = weak.update(cx, |this, cx| {
                if this.toast_gen == generation {
                    this.toast = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn is_selected(&self, source: PileRef, index: usize) -> bool {
        let Some(selection) = self.selected else {
            return false;
        };
        if selection.source != source {
            return false;
        }
        match source {
            PileRef::Tableau(_) => index >= selection.index,
            _ => index == selection.index,
        }
    }

    fn is_hint_source(&self, source: PileRef, index: usize) -> bool {
        match self.hint {
            Some(Hint::Move {
                source: hint_source,
                index: hint_index,
                ..
            }) if hint_source == source => match source {
                PileRef::Tableau(_) => index >= hint_index,
                _ => index == hint_index,
            },
            _ => false,
        }
    }

    fn hint_destination(&self) -> Option<PileRef> {
        match self.hint {
            Some(Hint::Move { dest, .. }) => Some(dest),
            _ => None,
        }
    }

    fn hint_stock(&self) -> bool {
        matches!(self.hint, Some(Hint::Draw) | Some(Hint::Recycle))
    }

    fn render_app(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let viewport = window.viewport_size();
        let vw: f32 = viewport.width.into();
        let vh: f32 = viewport.height.into();
        let palette = self.settings.theme.palette();

        let mut root = div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(linear_gradient(
                160.,
                linear_color_stop(rgb(palette.bg_top), 0.0),
                linear_color_stop(rgb(palette.bg_bottom), 1.0),
            ))
            .text_color(rgb(palette.text))
            .text_size(px(14.0))
            .key_context("Solitaire")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_new_game))
            .on_action(cx.listener(Self::on_restart_deal))
            .on_action(cx.listener(Self::on_daily_deal))
            .on_action(cx.listener(Self::on_undo_move))
            .on_action(cx.listener(Self::on_redo_move))
            .on_action(cx.listener(Self::on_show_hint))
            .on_action(cx.listener(Self::on_auto_finish))
            .on_action(cx.listener(Self::on_pause_game))
            .on_action(cx.listener(Self::on_show_stats))
            .on_action(cx.listener(Self::on_show_settings))
            .on_action(cx.listener(Self::on_show_help))
            .on_action(cx.listener(Self::on_draw_cards))
            .on_action(cx.listener(Self::on_dismiss))
            .on_action(cx.listener(Self::on_quit_app));

        root = root
            .child(self.render_menu_bar(cx))
            .child(self.render_toolbar(cx))
            .child(self.render_board(vw, vh, cx))
            .child(self.render_status_bar());

        if let Some(menu) = self.menu {
            root = root
                .child(
                    div()
                        .id("menu-backdrop")
                        .absolute()
                        .top(px(34.0))
                        .left(px(0.0))
                        .right(px(0.0))
                        .bottom(px(0.0))
                        .on_click(cx.listener(|this, _, _, cx| this.close_menu(cx))),
                )
                .child(self.render_menu_dropdown(menu, vw, cx));
        }

        if let Some(modal) = self.modal {
            root = root.child(self.render_modal(modal, vw, vh, cx));
        }

        if let Some(toast) = self.toast.clone() {
            root = root.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .right(px(0.0))
                    .bottom(px(44.0))
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .px(px(18.0))
                            .py(px(10.0))
                            .rounded(px(999.0))
                            .bg(rgba(palette.panel))
                            .border_1()
                            .border_color(rgb(palette.accent).fade(0.45))
                            .shadow_lg()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(palette.text))
                            .child(toast)
                            .with_animation(
                                ("toast", self.toast_gen),
                                Animation::new(Duration::from_millis(180)),
                                |element, delta| element.opacity(delta),
                            ),
                    ),
            );
        }

        root
    }

    fn render_menu_bar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let palette = self.settings.theme.palette();
        let menu_button =
            |id: &'static str, label: &'static str, kind: MenuKind, cx: &mut Context<Self>| {
                let active = self.menu == Some(kind);
                div()
                    .id(id)
                    .w(px(match kind {
                        MenuKind::Game => 72.0,
                        MenuKind::Options => 92.0,
                        MenuKind::Help => 64.0,
                    }))
                    .h(px(26.0))
                    .flex()
                    .justify_center()
                    .items_center()
                    .rounded(px(6.0))
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if active {
                        rgb(palette.text)
                    } else {
                        rgb(palette.muted)
                    })
                    .when(active, |this| this.bg(rgba(0xffffff14)))
                    .cursor(CursorStyle::PointingHand)
                    .hover(|this| this.bg(rgba(0xffffff12)).text_color(rgb(palette.text)))
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle_menu(kind, cx)))
                    .child(label)
            };

        let mut right = div()
            .flex_1()
            .flex()
            .justify_end()
            .items_center()
            .gap(px(10.0))
            .text_size(px(11.0))
            .text_color(rgb(palette.muted));
        right = right.child(format!(
            "{}  ·  {}  ·  {}",
            self.game.rules.draw_label(),
            self.game.rules.scoring.label(),
            if self.game.rules.timed {
                "Timed"
            } else {
                "Untimed"
            }
        ));

        div()
            .h(px(34.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(px(16.0))
            .bg(rgba(0x0000004d))
            .border_b_1()
            .border_color(rgba(palette.border))
            .child(
                div()
                    .w(px(150.0))
                    .flex()
                    .items_center()
                    .gap(px(9.0))
                    .child(
                        div()
                            .w(px(22.0))
                            .h(px(22.0))
                            .rounded(px(6.0))
                            .bg(linear_gradient(
                                145.,
                                linear_color_stop(rgb(palette.accent), 0.0),
                                linear_color_stop(rgb(palette.accent_soft), 1.0),
                            ))
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_color(rgb(0x17221d))
                            .text_size(px(15.0))
                            .font_weight(FontWeight::BOLD)
                            .child("♠"),
                    )
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(palette.text))
                            .child("SOLITARITE"),
                    ),
            )
            .child(menu_button("menu-game", "Game", MenuKind::Game, cx))
            .child(menu_button(
                "menu-options",
                "Options",
                MenuKind::Options,
                cx,
            ))
            .child(menu_button("menu-help", "Help", MenuKind::Help, cx))
            .child(right)
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let palette = self.settings.theme.palette();
        let can_undo = self.game.can_undo();
        let can_hint = !self.game.won;
        let can_auto = self.game.can_auto_finish() && !self.auto_finishing;
        let can_pause = !self.game.won;

        let auto_label = if self.auto_finishing {
            "Finishing…"
        } else {
            "Auto Finish"
        };

        div()
            .h(px(58.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(18.0))
            .bg(rgba(0x0000002e))
            .border_b_1()
            .border_color(rgba(palette.border))
            .child(tool_button(
                "toolbar-new",
                "＋  New Deal",
                true,
                true,
                palette,
                cx.listener(|this, _, _, cx| this.request_new_game(NewKind::Random, cx)),
            ))
            .child(tool_button(
                "toolbar-undo",
                "↶  Undo",
                can_undo,
                false,
                palette,
                cx.listener(|this, _, _, cx| this.undo(cx)),
            ))
            .child(tool_button(
                "toolbar-hint",
                "✦  Hint",
                can_hint,
                false,
                palette,
                cx.listener(|this, _, _, cx| this.show_hint(cx)),
            ))
            .child(tool_button(
                "toolbar-auto",
                auto_label,
                can_auto,
                false,
                palette,
                cx.listener(|this, _, _, cx| this.start_auto_finish(cx)),
            ))
            .child(
                div()
                    .w(px(1.0))
                    .h(px(24.0))
                    .bg(rgba(palette.border))
                    .mx(px(4.0)),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_end()
                    .items_center()
                    .gap(px(8.0))
                    .child(tool_button(
                        "toolbar-stats",
                        "Statistics",
                        true,
                        false,
                        palette,
                        cx.listener(|this, _, _, cx| this.open_modal(Modal::Stats, cx)),
                    ))
                    .child(tool_button(
                        "toolbar-settings",
                        "Settings",
                        true,
                        false,
                        palette,
                        cx.listener(|this, _, _, cx| this.open_modal(Modal::Settings, cx)),
                    ))
                    .child(tool_button(
                        "toolbar-pause",
                        if self.game.paused {
                            "▶  Resume"
                        } else {
                            "Ⅱ  Pause"
                        },
                        can_pause,
                        false,
                        palette,
                        cx.listener(|this, _, _, cx| this.toggle_pause(cx)),
                    )),
            )
    }

    fn render_board(&self, vw: f32, vh: f32, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let palette = self.settings.theme.palette();
        let gap = 12.0;
        let cw = ((vw - 80.0 - gap * 6.0) / 7.0).clamp(68.0, 124.0);
        let ch = cw * 1.42;
        let board_w = cw * 7.0 + gap * 6.0;
        let board_h = (vh - 34.0 - 58.0 - 30.0 - 20.0).max(430.0);
        let inner_h = board_h - 36.0;
        let tableau_h = (inner_h - ch - 18.0).max(180.0);

        let max_units = self
            .game
            .tableau
            .iter()
            .map(|pile| {
                pile.iter()
                    .skip(1)
                    .map(|card| if card.face_up { 1.0 } else { 0.42 })
                    .sum::<f32>()
            })
            .fold(0.0f32, f32::max);
        let step = if max_units > 0.0 {
            ((tableau_h - ch) / max_units).clamp(6.0, ch * 0.24)
        } else {
            ch * 0.24
        };
        let up_step = step;
        let down_step = (step * 0.42).max(4.0);

        let stock = self.render_stock(cw, ch, cx);
        let waste = self.render_waste(cw, ch, cx);
        let mut foundations = Vec::with_capacity(4);
        for pile in 0..4 {
            foundations.push(self.render_foundation(pile, cw, ch, cx));
        }

        let top_row = div()
            .flex()
            .items_start()
            .gap(px(gap))
            .w_full()
            .h(px(ch))
            .child(stock)
            .child(waste)
            .child(div().w(px(cw)).h(px(ch)))
            .children(foundations);

        let mut tableau_row = div()
            .flex()
            .items_start()
            .gap(px(gap))
            .w_full()
            .h(px(tableau_h))
            .mt(px(18.0));

        for pile in 0..7 {
            tableau_row = tableau_row
                .child(self.render_tableau(pile, cw, ch, tableau_h, up_step, down_step, cx));
        }

        let board = div()
            .relative()
            .w(px(board_w + 36.0))
            .h(px(board_h))
            .p(px(18.0))
            .rounded(px(20.0))
            .overflow_hidden()
            .border_1()
            .border_color(rgba(palette.border))
            .shadow_lg()
            .bg(linear_gradient(
                145.,
                linear_color_stop(rgb(palette.board_top), 0.0),
                linear_color_stop(rgb(palette.board_bottom), 1.0),
            ))
            .child(
                div()
                    .absolute()
                    .top(px(-100.0))
                    .left(px(-90.0))
                    .w(px(360.0))
                    .h(px(360.0))
                    .rounded_full()
                    .bg(rgb(palette.accent).fade(0.035)),
            )
            .child(
                div()
                    .absolute()
                    .bottom(px(-150.0))
                    .right(px(-120.0))
                    .w(px(430.0))
                    .h(px(430.0))
                    .rounded_full()
                    .bg(rgb(0xffffff).fade(0.022)),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .w(px(board_w))
                    .h(px(inner_h))
                    .child(top_row)
                    .child(tableau_row),
            );

        div()
            .flex_1()
            .flex()
            .justify_center()
            .items_center()
            .px(px(18.0))
            .py(px(10.0))
            .child(board)
    }

    fn render_stock(&self, cw: f32, ch: f32, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let palette = self.settings.theme.palette();
        let back = self.settings.card_back;
        let highlighted = self.hint_stock();
        let count = self.game.stock.len();

        let mut stock = div()
            .id("stock-pile")
            .relative()
            .w(px(cw))
            .h(px(ch))
            .rounded(px(cw * 0.065))
            .border_1()
            .border_color(if highlighted {
                rgb(palette.accent)
            } else {
                rgba(palette.border)
            })
            .bg(rgba(0x0000002e))
            .cursor(CursorStyle::PointingHand)
            .hover(|this| this.bg(rgba(0x00000045)))
            .on_click(cx.listener(|this, _, _, cx| this.draw(cx)));

        if count == 0 {
            let recycle = !self.game.waste.is_empty();
            stock = stock.child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .w(px(cw))
                    .h(px(ch))
                    .rounded(px(cw * 0.065))
                    .border_1()
                    .border_dashed()
                    .border_color(rgb(palette.muted).fade(0.32))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .gap(px(6.0))
                    .text_color(rgb(palette.muted).fade(0.75))
                    .child(
                        div()
                            .text_size(px(cw * 0.28))
                            .line_height(px(cw * 0.28))
                            .child(if recycle { "↻" } else { "♠" }),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if recycle { "RECYCLE" } else { "EMPTY" }),
                    ),
            );
        } else {
            for index in 0..count.min(3) {
                stock = stock.child(
                    div()
                        .absolute()
                        .top(px(index as f32 * 2.0))
                        .left(px(index as f32 * 2.0))
                        .child(card_back(cw, ch, back)),
                );
            }
        }

        stock.child(
            div()
                .absolute()
                .bottom(px(7.0))
                .right(px(7.0))
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(999.0))
                .bg(rgba(0x000000a6))
                .text_size(px(10.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(palette.text))
                .child(count.to_string()),
        )
    }

    fn render_waste(&self, cw: f32, ch: f32, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let palette = self.settings.theme.palette();
        let back = self.settings.card_back;
        let mut waste = div().relative().w(px(cw)).h(px(ch));

        if self.game.waste.is_empty() {
            waste = waste.child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .w(px(cw))
                    .h(px(ch))
                    .rounded(px(cw * 0.065))
                    .border_1()
                    .border_dashed()
                    .border_color(rgb(palette.muted).fade(0.25))
                    .flex()
                    .justify_center()
                    .items_center()
                    .text_color(rgb(palette.muted).fade(0.45))
                    .text_size(px(11.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("WASTE"),
            );
        } else {
            let visible = if self.game.rules.draw_three {
                self.game.waste.len().min(3)
            } else {
                1
            };
            let start = self.game.waste.len() - visible;
            for index in start..self.game.waste.len() {
                let card = self.game.waste[index];
                let left = (index - start) as f32 * cw * 0.16;
                if index + 1 == self.game.waste.len() {
                    waste = waste.child(self.interactive_card(
                        card,
                        PileRef::Waste,
                        index,
                        cw,
                        ch,
                        0.0,
                        left,
                        cx,
                    ));
                } else {
                    waste = waste.child(
                        div()
                            .absolute()
                            .top(px(0.0))
                            .left(px(left))
                            .w(px(cw))
                            .h(px(ch))
                            .child(card_view(card, cw, ch, back)),
                    );
                }
            }
        }

        waste
    }

    fn render_foundation(
        &self,
        pile: usize,
        cw: f32,
        ch: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let palette = self.settings.theme.palette();
        let back = self.settings.card_back;
        let suit = crate::model::Suit::ALL[pile];
        let highlighted = self.hint_destination() == Some(PileRef::Foundation(pile));
        let cards = &self.game.foundations[pile];

        let mut foundation = div()
            .id(("foundation", pile))
            .relative()
            .w(px(cw))
            .h(px(ch))
            .on_drop(cx.listener(move |this, drag: &DragCards, _, cx| {
                this.on_drop(drag, PileRef::Foundation(pile), cx)
            }));

        if cards.is_empty() {
            foundation = foundation.child(
                div()
                    .id(("foundation-empty", pile))
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .w(px(cw))
                    .h(px(ch))
                    .rounded(px(cw * 0.065))
                    .border_1()
                    .border_dashed()
                    .border_color(if highlighted {
                        rgb(palette.accent)
                    } else {
                        rgb(palette.muted).fade(0.28)
                    })
                    .bg(rgba(0x0000001f))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .gap(px(4.0))
                    .cursor(CursorStyle::PointingHand)
                    .hover(|this| this.bg(rgba(0xffffff08)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.on_empty_destination(PileRef::Foundation(pile), cx)
                    }))
                    .child(
                        div()
                            .text_size(px(cw * 0.26))
                            .line_height(px(cw * 0.28))
                            .text_color(if suit.color() == crate::model::CardColor::Red {
                                card_red().fade(0.34)
                            } else {
                                rgb(0x111111).fade(0.28)
                            })
                            .child(suit.symbol()),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(palette.muted).fade(0.6))
                            .child("ACE"),
                    ),
            );
        } else {
            for (index, card) in cards.iter().enumerate() {
                if index + 1 == cards.len() {
                    foundation = foundation.child(self.interactive_card(
                        *card,
                        PileRef::Foundation(pile),
                        index,
                        cw,
                        ch,
                        0.0,
                        0.0,
                        cx,
                    ));
                } else {
                    foundation = foundation.child(
                        div()
                            .absolute()
                            .top(px(0.0))
                            .left(px(0.0))
                            .w(px(cw))
                            .h(px(ch))
                            .child(card_view(*card, cw, ch, back)),
                    );
                }
            }
        }

        if highlighted {
            foundation = foundation.child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .w(px(cw))
                    .h(px(ch))
                    .rounded(px(cw * 0.065))
                    .border_2()
                    .border_color(rgb(palette.accent)),
            );
        }
        foundation
    }

    fn render_tableau(
        &self,
        pile: usize,
        cw: f32,
        ch: f32,
        tableau_h: f32,
        up_step: f32,
        down_step: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let palette = self.settings.theme.palette();
        let back = self.settings.card_back;
        let cards = &self.game.tableau[pile];
        let highlighted = self.hint_destination() == Some(PileRef::Tableau(pile));

        let mut tableau = div()
            .id(("tableau", pile))
            .relative()
            .w(px(cw))
            .h(px(tableau_h))
            .on_drop(cx.listener(move |this, drag: &DragCards, _, cx| {
                this.on_drop(drag, PileRef::Tableau(pile), cx)
            }));

        if cards.is_empty() {
            tableau = tableau.child(
                div()
                    .id(("tableau-empty", pile))
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .w(px(cw))
                    .h(px(ch))
                    .rounded(px(cw * 0.065))
                    .border_1()
                    .border_dashed()
                    .border_color(if highlighted {
                        rgb(palette.accent)
                    } else {
                        rgb(palette.muted).fade(0.24)
                    })
                    .bg(rgba(0x00000017))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .gap(px(3.0))
                    .cursor(CursorStyle::PointingHand)
                    .hover(|this| this.bg(rgba(0xffffff08)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.on_empty_destination(PileRef::Tableau(pile), cx)
                    }))
                    .child(
                        div()
                            .text_size(px(cw * 0.26))
                            .line_height(px(cw * 0.28))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(palette.muted).fade(0.35))
                            .child("K"),
                    )
                    .child(
                        div()
                            .text_size(px(9.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(palette.muted).fade(0.42))
                            .child("KING"),
                    ),
            );
        } else {
            let mut offset = 0.0f32;
            for (index, card) in cards.iter().enumerate() {
                let top = offset;
                if card.face_up {
                    tableau = tableau.child(self.interactive_card(
                        *card,
                        PileRef::Tableau(pile),
                        index,
                        cw,
                        ch,
                        top,
                        0.0,
                        cx,
                    ));
                } else {
                    tableau = tableau.child(
                        div()
                            .absolute()
                            .top(px(top))
                            .left(px(0.0))
                            .w(px(cw))
                            .h(px(ch))
                            .child(card_back(cw, ch, back)),
                    );
                }
                offset += if card.face_up { up_step } else { down_step };
            }
        }

        if highlighted {
            tableau = tableau.child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .w(px(cw))
                    .h(px(ch))
                    .rounded(px(cw * 0.065))
                    .border_2()
                    .border_color(rgb(palette.accent)),
            );
        }
        tableau
    }

    fn interactive_card(
        &self,
        card: Card,
        source: PileRef,
        index: usize,
        cw: f32,
        ch: f32,
        top: f32,
        left: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let palette = self.settings.theme.palette();
        let back = self.settings.card_back;
        let selected = self.is_selected(source, index);
        let hinted = self.is_hint_source(source, index);
        let mut face = card_view(card, cw, ch, back);
        if selected {
            face = face.border_2().border_color(gold()).shadow_lg();
        } else if hinted {
            face = face
                .border_2()
                .border_color(rgb(palette.accent_soft))
                .shadow_lg();
        }

        let drag_cards = self
            .game
            .cards_at(source, index)
            .map(|cards| cards.to_vec());
        let mut element = div()
            .id(("card", card.id as usize))
            .absolute()
            .top(px(top))
            .left(px(left))
            .w(px(cw))
            .h(px(ch))
            .cursor(CursorStyle::OpenHand)
            .hover(|this| this.opacity(0.98))
            .on_click(
                cx.listener(move |this, event, _, cx| this.on_card_click(source, index, event, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    this.on_card_right_click(source, index, cx)
                }),
            )
            .child(face);

        if let Some(cards) = drag_cards {
            let payload = DragCards {
                source,
                index,
                cards: cards.clone(),
            };
            element = element.on_drag(payload, move |_drag: &DragCards, _offset, _window, cx| {
                cx.new(|_| DragGhost::new(cards.clone(), cw, ch, back))
            });
        }
        element
    }

    fn render_status_bar(&self) -> impl IntoElement + use<> {
        let palette = self.settings.theme.palette();
        let score = format_score(self.game.score, self.game.rules.scoring);
        let score_color = match self.game.rules.scoring {
            Scoring::Vegas if self.game.score < 0 => rgb(palette.danger),
            Scoring::Vegas => rgb(palette.success),
            Scoring::Standard => rgb(palette.accent),
        };
        let progress = self.game.progress();
        let game_label = if self.game.is_daily {
            "Daily Deal".to_string()
        } else {
            format!("Game #{:06}", self.game.deal_number)
        };
        let state = if self.game.won {
            "WON"
        } else if self.game.paused {
            "PAUSED"
        } else if self.auto_finishing {
            "AUTO"
        } else {
            ""
        };

        div()
            .h(px(30.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(24.0))
            .px(px(18.0))
            .bg(rgba(0x00000040))
            .border_t_1()
            .border_color(rgba(palette.border))
            .text_size(px(11.0))
            .text_color(rgb(palette.muted))
            .child(status_item("SCORE", score, score_color))
            .child(status_item(
                "TIME",
                format_time(self.game.elapsed),
                rgb(palette.text),
            ))
            .child(status_item(
                "MOVES",
                self.game.moves.to_string(),
                rgb(palette.text),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("PROGRESS"),
                    )
                    .child(
                        div()
                            .w(px(116.0))
                            .h(px(6.0))
                            .rounded(px(999.0))
                            .bg(rgba(0xffffff1c))
                            .child(
                                div()
                                    .w(px(116.0 * progress))
                                    .h(px(6.0))
                                    .rounded(px(999.0))
                                    .bg(rgb(palette.accent)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(palette.text))
                            .child(format!("{}/52", self.game.total_home())),
                    ),
            )
            .child(div().flex_1())
            .when(!state.is_empty(), |this| {
                this.child(
                    div()
                        .px(px(7.0))
                        .py(px(2.0))
                        .rounded(px(999.0))
                        .bg(rgb(palette.accent).fade(0.15))
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(palette.accent))
                        .child(state),
                )
            })
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(palette.muted))
                    .child(game_label),
            )
    }

    fn render_menu_dropdown(
        &self,
        menu: MenuKind,
        vw: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let palette = self.settings.theme.palette();
        let width: f32 = match menu {
            MenuKind::Game => 280.0,
            MenuKind::Options => 300.0,
            MenuKind::Help => 250.0,
        };
        let candidate: f32 = match menu {
            MenuKind::Game => 166.0,
            MenuKind::Options => 238.0,
            MenuKind::Help => 330.0,
        };
        let left = candidate.min((vw - width - 12.0).max(8.0));

        let content = match menu {
            MenuKind::Game => self.game_menu(cx).into_any_element(),
            MenuKind::Options => self.options_menu(cx).into_any_element(),
            MenuKind::Help => self.help_menu(cx).into_any_element(),
        };

        div()
            .absolute()
            .top(px(38.0))
            .left(px(left))
            .w(px(width))
            .py(px(6.0))
            .rounded(px(10.0))
            .bg(rgba(palette.panel))
            .border_1()
            .border_color(rgba(palette.border))
            .shadow_lg()
            .child(content)
    }

    fn game_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        let can_undo = self.game.can_undo();
        let can_redo = self.game.can_redo();
        let can_auto = self.game.can_auto_finish() && !self.auto_finishing;

        div()
            .flex()
            .flex_col()
            .child(menu_item(
                "gm-new",
                "New Game",
                "F2",
                false,
                true,
                p,
                cx.listener(|this, _, _, cx| this.request_new_game(NewKind::Random, cx)),
            ))
            .child(menu_item(
                "gm-daily",
                "Daily Deal",
                "D",
                false,
                true,
                p,
                cx.listener(|this, _, _, cx| this.request_new_game(NewKind::Daily, cx)),
            ))
            .child(menu_item(
                "gm-restart",
                "Restart Deal",
                "R",
                false,
                true,
                p,
                cx.listener(|this, _, _, cx| this.request_new_game(NewKind::Restart, cx)),
            ))
            .child(menu_separator(p))
            .child(menu_item(
                "gm-undo",
                "Undo",
                "Ctrl+Z",
                false,
                can_undo,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.undo(cx);
                }),
            ))
            .child(menu_item(
                "gm-redo",
                "Redo",
                "Ctrl+Y",
                false,
                can_redo,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.redo(cx);
                }),
            ))
            .child(menu_item(
                "gm-hint",
                "Hint",
                "H",
                false,
                !self.game.won,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.show_hint(cx);
                }),
            ))
            .child(menu_item(
                "gm-auto",
                if self.auto_finishing {
                    "Finishing…"
                } else {
                    "Auto Finish"
                },
                "A",
                false,
                can_auto,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.start_auto_finish(cx);
                }),
            ))
            .child(menu_separator(p))
            .child(menu_item(
                "gm-pause",
                "Pause",
                "P",
                false,
                !self.game.won,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.toggle_pause(cx);
                }),
            ))
            .child(menu_item(
                "gm-stats",
                "Statistics",
                "S",
                false,
                true,
                p,
                cx.listener(|this, _, _, cx| this.open_modal(Modal::Stats, cx)),
            ))
            .child(menu_separator(p))
            .child(menu_item(
                "gm-exit",
                "Exit",
                "Ctrl+Q",
                false,
                true,
                p,
                cx.listener(|this, _, _, cx| this.quit(cx)),
            ))
    }

    fn options_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        let rules = self.settings.rules;
        div()
            .flex()
            .flex_col()
            .child(menu_item(
                "om-draw-one",
                "Draw One",
                "",
                !rules.draw_three,
                true,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.set_draw_three(false, cx);
                }),
            ))
            .child(menu_item(
                "om-draw-three",
                "Draw Three",
                "",
                rules.draw_three,
                true,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.set_draw_three(true, cx);
                }),
            ))
            .child(menu_separator(p))
            .child(menu_item(
                "om-standard",
                "Standard Scoring",
                "",
                rules.scoring == Scoring::Standard,
                true,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.set_scoring(Scoring::Standard, cx);
                }),
            ))
            .child(menu_item(
                "om-vegas",
                "Vegas Scoring",
                "",
                rules.scoring == Scoring::Vegas,
                true,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.set_scoring(Scoring::Vegas, cx);
                }),
            ))
            .child(menu_item(
                "om-timed",
                "Timed Game",
                "",
                rules.timed,
                true,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.set_timed(!this.settings.rules.timed, cx);
                }),
            ))
            .child(menu_separator(p))
            .child(menu_item(
                "om-autoplay",
                "Auto-play to Foundations",
                "",
                self.settings.auto_play,
                true,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.toggle_auto_play(cx);
                }),
            ))
            .child(menu_item(
                "om-theme",
                format!("Theme: {}", self.settings.theme.label()),
                "",
                false,
                true,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.cycle_theme(cx);
                }),
            ))
            .child(menu_item(
                "om-back",
                format!("Card Back: {}", self.settings.card_back.label()),
                "",
                false,
                true,
                p,
                cx.listener(|this, _, _, cx| {
                    this.menu = None;
                    this.cycle_card_back(cx);
                }),
            ))
            .child(menu_separator(p))
            .child(menu_item(
                "om-settings",
                "Settings…",
                "Ctrl+Shift+S",
                false,
                true,
                p,
                cx.listener(|this, _, _, cx| this.open_modal(Modal::Settings, cx)),
            ))
    }

    fn help_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        div()
            .flex()
            .flex_col()
            .child(menu_item(
                "hm-how",
                "How to Play",
                "F1",
                false,
                true,
                p,
                cx.listener(|this, _, _, cx| this.open_modal(Modal::Help, cx)),
            ))
            .child(menu_item(
                "hm-stats",
                "Statistics",
                "S",
                false,
                true,
                p,
                cx.listener(|this, _, _, cx| this.open_modal(Modal::Stats, cx)),
            ))
            .child(menu_separator(p))
            .child(menu_item(
                "hm-about",
                "About Solitarite",
                "",
                false,
                true,
                p,
                cx.listener(|this, _, _, cx| this.open_modal(Modal::About, cx)),
            ))
    }

    fn render_modal(
        &self,
        modal: Modal,
        vw: f32,
        vh: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let overlay = div()
            .id("modal-backdrop")
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .bg(overlay())
            .flex()
            .justify_center()
            .items_center()
            .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx)));

        match modal {
            Modal::Settings => overlay.child(self.render_settings(vw, vh, cx)),
            Modal::Stats => overlay.child(self.render_stats(vw, vh, cx)),
            Modal::Help => overlay.child(self.render_help(vw, vh, cx)),
            Modal::About => overlay.child(self.render_about(cx)),
            Modal::ConfirmNew(kind) => overlay.child(self.render_confirm_new(kind, cx)),
            Modal::ConfirmResetStats => overlay.child(self.render_confirm_reset(cx)),
            Modal::Pause => overlay.child(self.render_pause(cx)),
            Modal::Victory => {
                let mut win_overlay = overlay.bg(rgba(0x02120de0));
                for index in 0..28 {
                    win_overlay = win_overlay.child(self.confetti_piece(index, vw, vh));
                }
                win_overlay.child(self.render_victory(cx))
            }
        }
    }

    fn render_settings(
        &self,
        vw: f32,
        vh: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        let width = 680.0f32.min(vw - 40.0);
        let rules = self.settings.rules;
        let rules_note = if self.game.moves == 0 {
            "Changes apply to the current deal."
        } else {
            "Draw and scoring changes apply to the next deal."
        };

        let draw_segments = div()
            .flex()
            .gap(px(6.0))
            .child(segment_button(
                "set-draw-one",
                "Draw 1",
                !rules.draw_three,
                p,
                cx.listener(|this, _, _, cx| this.set_draw_three(false, cx)),
            ))
            .child(segment_button(
                "set-draw-three",
                "Draw 3",
                rules.draw_three,
                p,
                cx.listener(|this, _, _, cx| this.set_draw_three(true, cx)),
            ));

        let scoring_segments = div()
            .flex()
            .gap(px(6.0))
            .child(segment_button(
                "set-standard",
                "Standard",
                rules.scoring == Scoring::Standard,
                p,
                cx.listener(|this, _, _, cx| this.set_scoring(Scoring::Standard, cx)),
            ))
            .child(segment_button(
                "set-vegas",
                "Vegas",
                rules.scoring == Scoring::Vegas,
                p,
                cx.listener(|this, _, _, cx| this.set_scoring(Scoring::Vegas, cx)),
            ));

        let mut theme_swatches = div().flex().gap(px(8.0));
        for theme in Theme::ALL {
            theme_swatches = theme_swatches.child(self.theme_swatch(theme, cx));
        }

        let mut back_swatches = div().flex().gap(px(8.0));
        for back in CardBack::ALL {
            back_swatches = back_swatches.child(self.back_swatch(back, cx));
        }

        let left = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(setting_group(
                "DRAW MODE",
                draw_segments,
                Some("Choose how many stock cards turn over at once."),
                p,
            ))
            .child(setting_group(
                "SCORING",
                scoring_segments,
                Some("Standard tracks points and time. Vegas tracks dollars."),
                p,
            ))
            .child(toggle_row(
                "TIMED GAME",
                "Standard scoring deducts points as time passes.",
                rules.timed,
                "set-timed",
                p,
                cx.listener(|this, _, _, cx| this.set_timed(!this.settings.rules.timed, cx)),
            ))
            .child(toggle_row(
                "AUTO-PLAY",
                "Send available cards to the foundations automatically.",
                self.settings.auto_play,
                "set-autoplay",
                p,
                cx.listener(|this, _, _, cx| this.toggle_auto_play(cx)),
            ));

        let right = div()
            .w(px(290.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(setting_group(
                "TABLE THEME",
                theme_swatches,
                Some("The felt and interface color palette."),
                p,
            ))
            .child(setting_group(
                "CARD BACK",
                back_swatches,
                Some("Choose a design for face-down cards."),
                p,
            ));

        div()
            .id("settings-panel")
            .occlude()
            .w(px(width))
            .max_h(px(vh - 40.0))
            .overflow_y_scroll()
            .rounded(px(18.0))
            .bg(rgba(p.panel))
            .border_1()
            .border_color(rgba(p.border))
            .shadow_lg()
            .p(px(24.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(21.0))
                            .font_weight(FontWeight::BOLD)
                            .child("Settings"),
                    )
                    .child(tool_button(
                        "settings-close",
                        "Done",
                        true,
                        false,
                        p,
                        cx.listener(|this, _, _, cx| this.close_modal(cx)),
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap(px(22.0))
                    .mt(px(20.0))
                    .child(left)
                    .child(right),
            )
            .child(
                div()
                    .mt(px(18.0))
                    .text_size(px(11.0))
                    .text_color(rgb(p.muted))
                    .child(rules_note),
            )
            .child(
                div()
                    .mt(px(16.0))
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(tool_button(
                        "settings-new",
                        "New Deal with These Settings",
                        true,
                        true,
                        p,
                        cx.listener(|this, _, _, cx| this.start_new_game(NewKind::Random, cx)),
                    )),
            )
    }

    fn theme_swatch(&self, theme: Theme, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        let palette = theme.palette();
        let active = self.settings.theme == theme;
        div()
            .id((theme.label(), theme as usize))
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(5.0))
            .p(px(5.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(if active {
                rgb(p.accent)
            } else {
                rgba(p.border)
            })
            .cursor(CursorStyle::PointingHand)
            .hover(|this| this.bg(rgba(0xffffff0c)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.settings.theme = theme;
                this.save_now();
                cx.notify();
            }))
            .child(
                div()
                    .w_full()
                    .h(px(38.0))
                    .rounded(px(5.0))
                    .bg(linear_gradient(
                        145.,
                        linear_color_stop(rgb(palette.felt_top), 0.0),
                        linear_color_stop(rgb(palette.felt_bottom), 1.0),
                    )),
            )
            .child(
                div()
                    .text_size(px(9.0))
                    .text_color(rgb(p.muted))
                    .child(theme.label()),
            )
    }

    fn back_swatch(&self, back: CardBack, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        let active = self.settings.card_back == back;
        div()
            .id((back.label(), back as usize))
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(5.0))
            .p(px(5.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(if active {
                rgb(p.accent)
            } else {
                rgba(p.border)
            })
            .cursor(CursorStyle::PointingHand)
            .hover(|this| this.bg(rgba(0xffffff0c)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.settings.card_back = back;
                this.save_now();
                cx.notify();
            }))
            .child(card_back(42.0, 58.0, back))
            .child(
                div()
                    .text_size(px(9.0))
                    .text_color(rgb(p.muted))
                    .child(back.label()),
            )
    }

    fn render_stats(&self, vw: f32, vh: f32, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        let width = 620.0f32.min(vw - 40.0);
        let rate = self.stats.win_rate();
        let standard_best = self
            .stats
            .best_standard_score
            .map(|score| score.to_string())
            .unwrap_or_else(|| "—".to_string());
        let vegas_best = self
            .stats
            .best_vegas_score
            .map(|score| format_score(score, Scoring::Vegas))
            .unwrap_or_else(|| "—".to_string());

        let mut cards = div().flex().flex_wrap().gap(px(10.0)).mt(px(18.0));
        for (label, value) in [
            ("Games played", self.stats.games_played.to_string()),
            ("Games won", self.stats.games_won.to_string()),
            ("Current streak", self.stats.current_streak.to_string()),
            ("Best streak", self.stats.best_streak.to_string()),
            ("Best time", self.stats.best_time_label()),
            (
                "Fewest moves",
                self.stats
                    .fewest_moves
                    .map(|moves| moves.to_string())
                    .unwrap_or_else(|| "—".to_string()),
            ),
            ("Best standard score", standard_best),
            ("Best Vegas score", vegas_best),
            ("Time won", self.stats.total_time_label()),
        ] {
            cards = cards.child(stat_card(label, value, p));
        }

        div()
            .id("stats-panel")
            .occlude()
            .w(px(width))
            .max_h(px(vh - 40.0))
            .overflow_y_scroll()
            .rounded(px(18.0))
            .bg(rgba(p.panel))
            .border_1()
            .border_color(rgba(p.border))
            .shadow_lg()
            .p(px(24.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(21.0))
                            .font_weight(FontWeight::BOLD)
                            .child("Statistics"),
                    )
                    .child(tool_button(
                        "stats-close",
                        "Done",
                        true,
                        false,
                        p,
                        cx.listener(|this, _, _, cx| this.close_modal(cx)),
                    )),
            )
            .child(
                div()
                    .mt(px(16.0))
                    .flex()
                    .items_end()
                    .gap(px(18.0))
                    .child(
                        div()
                            .text_size(px(46.0))
                            .line_height(px(48.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.accent))
                            .child(self.stats.win_rate_label()),
                    )
                    .child(
                        div()
                            .pb(px(6.0))
                            .text_size(px(12.0))
                            .text_color(rgb(p.muted))
                            .child(format!("win rate across {} games", self.stats.games_played)),
                    ),
            )
            .child(
                div()
                    .mt(px(10.0))
                    .w_full()
                    .h(px(8.0))
                    .rounded(px(999.0))
                    .bg(rgba(0xffffff16))
                    .child(
                        div()
                            .w(px((width - 48.0) * rate))
                            .h(px(8.0))
                            .rounded(px(999.0))
                            .bg(linear_gradient(
                                90.,
                                linear_color_stop(rgb(p.accent), 0.0),
                                linear_color_stop(rgb(p.accent_soft), 1.0),
                            )),
                    ),
            )
            .child(cards)
            .child(
                div()
                    .mt(px(18.0))
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div().text_size(px(11.0)).text_color(rgb(p.muted)).child(
                            self.stats
                                .last_played
                                .as_ref()
                                .map(|date| format!("Last played {date}"))
                                .unwrap_or_else(|| "No games played yet".to_string()),
                        ),
                    )
                    .child(tool_button(
                        "stats-reset",
                        "Reset Statistics",
                        self.stats.games_played > 0,
                        false,
                        p,
                        cx.listener(|this, _, _, cx| this.open_modal(Modal::ConfirmResetStats, cx)),
                    )),
            )
    }

    fn render_help(&self, vw: f32, vh: f32, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        let width = 720.0f32.min(vw - 40.0);

        let rule = |number: &'static str, text: &'static str| {
            div()
                .flex()
                .items_start()
                .gap(px(10.0))
                .child(
                    div()
                        .w(px(22.0))
                        .h(px(22.0))
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(rgb(p.accent).fade(0.14))
                        .flex()
                        .justify_center()
                        .items_center()
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(p.accent))
                        .child(number),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(19.0))
                        .text_color(rgb(p.text))
                        .child(text),
                )
        };

        let shortcut = |keys: &'static str, action: &'static str| {
            div()
                .flex()
                .justify_between()
                .items_center()
                .py(px(3.0))
                .child(
                    div()
                        .px(px(6.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .bg(rgba(0xffffff12))
                        .border_1()
                        .border_color(rgba(p.border))
                        .text_size(px(10.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(p.text))
                        .child(keys),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(p.muted))
                        .child(action),
                )
        };

        div()
            .id("help-panel")
            .occlude()
            .w(px(width))
            .max_h(px(vh - 40.0))
            .overflow_y_scroll()
            .rounded(px(18.0))
            .bg(rgba(p.panel))
            .border_1()
            .border_color(rgba(p.border))
            .shadow_lg()
            .p(px(24.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(21.0))
                            .font_weight(FontWeight::BOLD)
                            .child("How to Play"),
                    )
                    .child(tool_button(
                        "help-close",
                        "Done",
                        true,
                        false,
                        p,
                        cx.listener(|this, _, _, cx| this.close_modal(cx)),
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap(px(26.0))
                    .mt(px(18.0))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(p.accent))
                                    .child("KLONDIKE RULES"),
                            )
                            .child(rule("1", "Build the four foundations from Ace to King, one suit per foundation."))
                            .child(rule("2", "In the tableau, build down in alternating colors — red on black, black on red."))
                            .child(rule("3", "Only a King (or a stack led by a King) can move into an empty tableau column."))
                            .child(rule("4", "Click the stock to draw. When it is empty, click again to recycle the waste."))
                            .child(rule("5", "Drag cards, or click a card and then click its destination. Double-click sends a card to its foundation."))
                            .child(rule("6", "Clear all 52 cards to the foundations to win.")),
                    )
                    .child(
                        div()
                            .w(px(250.0))
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(p.accent))
                                    .child("KEYBOARD"),
                            )
                            .child(shortcut("F2 / N", "New game"))
                            .child(shortcut("Space", "Draw from stock"))
                            .child(shortcut("Ctrl+Z", "Undo"))
                            .child(shortcut("Ctrl+Y", "Redo"))
                            .child(shortcut("H", "Hint"))
                            .child(shortcut("A", "Auto finish"))
                            .child(shortcut("P", "Pause"))
                            .child(shortcut("S", "Statistics"))
                            .child(shortcut("F1", "Help"))
                            .child(shortcut("Esc", "Close / deselect")),
                    ),
            )
            .child(
                div()
                    .mt(px(18.0))
                    .p(px(12.0))
                    .rounded(px(8.0))
                    .bg(rgb(p.accent).fade(0.08))
                    .text_size(px(11.0))
                    .line_height(px(17.0))
                    .text_color(rgb(p.muted))
                    .child("Tip: right-click a card to send it to a foundation. Your game, settings, and statistics save automatically."),
            )
    }

    fn render_about(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        div()
            .id("about-panel")
            .occlude()
            .w(px(500.0))
            .rounded(px(18.0))
            .bg(rgba(p.panel))
            .border_1()
            .border_color(rgba(p.border))
            .shadow_lg()
            .p(px(26.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.0))
                    .child(
                        div()
                            .w(px(52.0))
                            .h(px(52.0))
                            .rounded(px(14.0))
                            .bg(linear_gradient(
                                145.,
                                linear_color_stop(rgb(p.accent), 0.0),
                                linear_color_stop(rgb(p.accent_soft), 1.0),
                            ))
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_color(rgb(0x17221d))
                            .text_size(px(31.0))
                            .child("♠"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(24.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child("Solitarite"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(rgb(p.muted))
                                    .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(px(18.0))
                    .text_size(px(13.0))
                    .line_height(px(20.0))
                    .text_color(rgb(p.text))
                    .child("A full-featured Klondike solitaire game built in Rust with GPUI. Classic rules, Standard and Vegas scoring, drag-and-drop, hints, auto-finish, daily deals, statistics, and automatic save and resume."),
            )
            .child(
                div()
                    .mt(px(16.0))
                    .p(px(10.0))
                    .rounded(px(7.0))
                    .bg(rgba(0x00000033))
                    .text_size(px(10.0))
                    .text_color(rgb(p.muted))
                    .child(format!("Progress saved to {}", save_path_display())),
            )
            .child(
                div()
                    .mt(px(20.0))
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(tool_button(
                        "about-stats",
                        "Statistics",
                        true,
                        false,
                        p,
                        cx.listener(|this, _, _, cx| this.open_modal(Modal::Stats, cx)),
                    ))
                    .child(tool_button(
                        "about-close",
                        "Done",
                        true,
                        true,
                        p,
                        cx.listener(|this, _, _, cx| this.close_modal(cx)),
                    )),
            )
    }

    fn render_confirm_new(
        &self,
        kind: NewKind,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        let (title, body) = match kind {
            NewKind::Random => (
                "Start a new deal?",
                "Your current game will be abandoned. Statistics for this deal will count as a loss.",
            ),
            NewKind::Daily => (
                "Play today's Daily Deal?",
                "Your current game will be abandoned. Daily deals use the same numbered deck for everyone today.",
            ),
            NewKind::Restart => (
                "Restart this deal?",
                "The cards will return to their original positions and your score, time, and moves will reset.",
            ),
        };

        div()
            .id("confirm-new-panel")
            .occlude()
            .w(px(460.0))
            .rounded(px(18.0))
            .bg(rgba(p.panel))
            .border_1()
            .border_color(rgba(p.border))
            .shadow_lg()
            .p(px(24.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .child(title),
            )
            .child(
                div()
                    .mt(px(10.0))
                    .text_size(px(12.0))
                    .line_height(px(19.0))
                    .text_color(rgb(p.muted))
                    .child(body),
            )
            .child(
                div()
                    .mt(px(22.0))
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(tool_button(
                        "confirm-new-cancel",
                        "Cancel",
                        true,
                        false,
                        p,
                        cx.listener(|this, _, _, cx| this.close_modal(cx)),
                    ))
                    .child(tool_button(
                        "confirm-new-start",
                        "Start New Deal",
                        true,
                        true,
                        p,
                        cx.listener(|this, _, _, cx| this.confirm_new_game(cx)),
                    )),
            )
    }

    fn render_confirm_reset(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        div()
            .id("confirm-reset-panel")
            .occlude()
            .w(px(440.0))
            .rounded(px(18.0))
            .bg(rgba(p.panel))
            .border_1()
            .border_color(rgba(p.border))
            .shadow_lg()
            .p(px(24.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .child("Reset statistics?"),
            )
            .child(
                div()
                    .mt(px(10.0))
                    .text_size(px(12.0))
                    .line_height(px(19.0))
                    .text_color(rgb(p.muted))
                    .child("This clears games played, wins, streaks, best times, and best scores. It cannot be undone."),
            )
            .child(
                div()
                    .mt(px(22.0))
                    .flex()
                    .justify_end()
                    .gap(px(8.0))
                    .child(tool_button(
                        "confirm-reset-cancel",
                        "Cancel",
                        true,
                        false,
                        p,
                        cx.listener(|this, _, _, cx| this.close_modal(cx)),
                    ))
                    .child(tool_button(
                        "confirm-reset-do",
                        "Reset Statistics",
                        true,
                        true,
                        p,
                        cx.listener(|this, _, _, cx| this.confirm_reset_stats(cx)),
                    )),
            )
    }

    fn render_pause(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        div()
            .id("pause-panel")
            .occlude()
            .w(px(420.0))
            .rounded(px(18.0))
            .bg(rgba(p.panel))
            .border_1()
            .border_color(rgba(p.border))
            .shadow_lg()
            .p(px(26.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .w(px(58.0))
                            .h(px(58.0))
                            .rounded_full()
                            .bg(rgb(p.accent).fade(0.13))
                            .flex()
                            .justify_center()
                            .items_center()
                            .text_size(px(24.0))
                            .text_color(rgb(p.accent))
                            .child("Ⅱ"),
                    )
                    .child(
                        div()
                            .text_size(px(22.0))
                            .font_weight(FontWeight::BOLD)
                            .child("Game Paused"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(p.muted))
                            .child(format!(
                                "{}  ·  {}  ·  {} moves",
                                format_time(self.game.elapsed),
                                format_score(self.game.score, self.game.rules.scoring),
                                self.game.moves
                            )),
                    ),
            )
            .child(
                div()
                    .mt(px(22.0))
                    .flex()
                    .justify_center()
                    .gap(px(8.0))
                    .child(tool_button(
                        "pause-new",
                        "New Deal",
                        true,
                        false,
                        p,
                        cx.listener(|this, _, _, cx| this.start_new_game(NewKind::Random, cx)),
                    ))
                    .child(tool_button(
                        "pause-resume",
                        "Resume",
                        true,
                        true,
                        p,
                        cx.listener(|this, _, _, cx| this.resume(cx)),
                    )),
            )
    }

    fn render_victory(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let p = self.settings.theme.palette();
        div()
            .id("victory-panel")
            .occlude()
            .w(px(560.0))
            .rounded(px(20.0))
            .bg(rgba(p.panel))
            .border_1()
            .border_color(rgb(p.accent).fade(0.55))
            .shadow_lg()
            .p(px(28.0))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.accent))
                            .child("KLONDIKE COMPLETE"),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(px(38.0))
                            .line_height(px(44.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text))
                            .child("You won!"),
                    )
                    .child(
                        div()
                            .mt(px(4.0))
                            .text_size(px(11.0))
                            .text_color(rgb(p.muted))
                            .child(if self.game.is_daily {
                                "Daily Deal cleared".to_string()
                            } else {
                                format!("Game #{:06}", self.game.deal_number)
                            }),
                    ),
            )
            .child(
                div()
                    .mt(px(22.0))
                    .flex()
                    .gap(px(10.0))
                    .child(stat_card("TIME", format_time(self.game.elapsed), p))
                    .child(stat_card("MOVES", self.game.moves.to_string(), p))
                    .child(stat_card(
                        "SCORE",
                        format_score(self.game.score, self.game.rules.scoring),
                        p,
                    )),
            )
            .child(
                div()
                    .mt(px(22.0))
                    .flex()
                    .justify_center()
                    .gap(px(8.0))
                    .child(tool_button(
                        "victory-view",
                        "View Board",
                        true,
                        false,
                        p,
                        cx.listener(|this, _, _, cx| this.close_modal(cx)),
                    ))
                    .child(tool_button(
                        "victory-new",
                        "Play Again",
                        true,
                        true,
                        p,
                        cx.listener(|this, _, _, cx| this.start_new_game(NewKind::Random, cx)),
                    )),
            )
    }

    fn confetti_piece(&self, index: usize, vw: f32, vh: f32) -> impl IntoElement + use<> {
        let colors = [0xe8bf68, 0xd84b4b, 0x5ecb8a, 0x5aa9e6, 0xf7f3e8, 0xc792ea];
        let color = colors[index % colors.len()];
        let x = ((index as f32 * 73.0 + 23.0) % (vw - 20.0)).max(6.0);
        let width = 5.0 + (index % 3) as f32 * 2.0;
        let height = 9.0 + (index % 4) as f32 * 3.0;
        let duration = 1900 + (index % 5) as u64 * 310;
        let phase = index as f32 * 0.9;
        div()
            .absolute()
            .top(px(-20.0))
            .left(px(x))
            .w(px(width))
            .h(px(height))
            .rounded(px(2.0))
            .bg(rgb(color))
            .with_animation(
                ("confetti", index),
                Animation::new(Duration::from_millis(duration)).repeat(),
                move |element, delta| {
                    let sway = (delta * std::f32::consts::TAU + phase).sin() * 16.0;
                    element
                        .top(px(-20.0 + delta * (vh + 40.0)))
                        .left(px((x + sway).clamp(0.0, vw - 8.0)))
                        .opacity(0.88)
                },
            )
    }

    fn on_new_game(&mut self, _: &NewGame, _: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_none() {
            self.request_new_game(NewKind::Random, cx);
        }
    }

    fn on_restart_deal(&mut self, _: &RestartDeal, _: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_none() {
            self.request_new_game(NewKind::Restart, cx);
        }
    }

    fn on_daily_deal(&mut self, _: &DailyDeal, _: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_none() {
            self.request_new_game(NewKind::Daily, cx);
        }
    }

    fn on_undo_move(&mut self, _: &UndoMove, _: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_none() {
            self.undo(cx);
        }
    }

    fn on_redo_move(&mut self, _: &RedoMove, _: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_none() {
            self.redo(cx);
        }
    }

    fn on_show_hint(&mut self, _: &ShowHint, _: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_none() {
            self.show_hint(cx);
        }
    }

    fn on_auto_finish(&mut self, _: &AutoFinish, _: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_none() {
            self.start_auto_finish(cx);
        }
    }

    fn on_pause_game(&mut self, _: &PauseGame, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_pause(cx);
    }

    fn on_show_stats(&mut self, _: &ShowStats, _: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_none() {
            self.open_modal(Modal::Stats, cx);
        }
    }

    fn on_show_settings(&mut self, _: &ShowSettings, _: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_none() {
            self.open_modal(Modal::Settings, cx);
        }
    }

    fn on_show_help(&mut self, _: &ShowHelp, _: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_none() {
            self.open_modal(Modal::Help, cx);
        }
    }

    fn on_draw_cards(&mut self, _: &DrawCards, _: &mut Window, cx: &mut Context<Self>) {
        if self.modal.is_none() {
            self.draw(cx);
        }
    }

    fn on_dismiss(&mut self, _: &Dismiss, _: &mut Window, cx: &mut Context<Self>) {
        if self.auto_finishing {
            self.auto_finishing = false;
            self.save_now();
            self.toast("Auto-finish stopped", cx);
            cx.notify();
        } else if self.menu.is_some() {
            self.menu = None;
            cx.notify();
        } else if self.modal.is_some() {
            self.close_modal(cx);
        } else {
            self.selected = None;
            self.hint = None;
            cx.notify();
        }
    }

    fn on_quit_app(&mut self, _: &QuitApp, _: &mut Window, cx: &mut Context<Self>) {
        self.quit(cx);
    }
}

impl Render for SolitaireApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_app(window, cx)
    }
}

impl Focusable for SolitaireApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn destination_for_pile(source: PileRef) -> Option<PileRef> {
    match source {
        PileRef::Waste => None,
        PileRef::Foundation(pile) => Some(PileRef::Foundation(pile)),
        PileRef::Tableau(pile) => Some(PileRef::Tableau(pile)),
    }
}

fn hint_message(game: &Game, hint: Hint) -> String {
    match hint {
        Hint::Draw => "Hint: draw from the stock".to_string(),
        Hint::Recycle => "Hint: recycle the waste pile".to_string(),
        Hint::Move {
            source,
            index,
            dest,
        } => {
            let card = game
                .cards_at(source, index)
                .and_then(|cards| cards.first())
                .copied();
            let card = card
                .map(card_label)
                .unwrap_or_else(|| SharedString::from("card"));
            match dest {
                PileRef::Foundation(pile) => format!(
                    "Hint: move {card} to the {} foundation",
                    crate::model::Suit::ALL[pile].symbol()
                ),
                PileRef::Tableau(pile) => {
                    format!("Hint: move {card} to tableau {}", pile + 1)
                }
                PileRef::Waste => "Hint: draw from the stock".to_string(),
            }
        }
    }
}

fn random_seed() -> u64 {
    let seed = rand::random::<u64>();
    if seed == 0 { 0x5eed_1234_abcd } else { seed }
}

fn daily_seed() -> u64 {
    let days = Local::now().date_naive().num_days_from_ce() as u64;
    days.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x005d_411e_2026
}

fn tool_button<F>(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    enabled: bool,
    primary: bool,
    palette: Palette,
    on_click: F,
) -> Stateful<Div>
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    let label = label.into();
    div()
        .id(id)
        .h(px(34.0))
        .px(px(13.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(8.0))
        .text_size(px(12.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if primary {
            rgb(0x17221d)
        } else {
            rgb(palette.text)
        })
        .border_1()
        .border_color(if primary {
            rgb(palette.accent)
        } else {
            rgba(palette.border)
        })
        .when(primary, |this| {
            this.bg(linear_gradient(
                145.,
                linear_color_stop(rgb(palette.accent), 0.0),
                linear_color_stop(rgb(palette.accent_soft), 1.0),
            ))
        })
        .when(!primary, |this| this.bg(rgba(0xffffff0d)))
        .when(enabled, |this| {
            this.cursor(CursorStyle::PointingHand)
                .hover(|this| {
                    if primary {
                        this.opacity(0.92)
                    } else {
                        this.bg(rgba(0xffffff18))
                    }
                })
                .active(|this| this.opacity(0.82))
                .on_click(on_click)
        })
        .when(!enabled, |this| this.opacity(0.36))
        .child(label)
}

fn menu_item<F>(
    id: &'static str,
    label: impl Into<SharedString>,
    shortcut: &'static str,
    checked: bool,
    enabled: bool,
    palette: Palette,
    on_click: F,
) -> Stateful<Div>
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    let label = label.into();
    div()
        .id(id)
        .h(px(30.0))
        .mx(px(6.0))
        .px(px(9.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .rounded(px(6.0))
        .text_size(px(12.0))
        .text_color(rgb(palette.text))
        .when(enabled, |this| {
            this.cursor(CursorStyle::PointingHand)
                .hover(|this| this.bg(rgba(0xffffff10)))
                .active(|this| this.bg(rgba(0xffffff18)))
                .on_click(on_click)
        })
        .when(!enabled, |this| this.opacity(0.36))
        .child(
            div()
                .w(px(14.0))
                .text_size(px(11.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(palette.accent))
                .child(if checked { "✓" } else { "" }),
        )
        .child(div().flex_1().child(label))
        .child(
            div()
                .text_size(px(9.0))
                .text_color(rgb(palette.muted))
                .child(shortcut),
        )
}

fn menu_separator(palette: Palette) -> impl IntoElement + use<> {
    div()
        .h(px(1.0))
        .mx(px(10.0))
        .my(px(5.0))
        .bg(rgba(palette.border))
}

fn segment_button<F>(
    id: &'static str,
    label: &'static str,
    active: bool,
    palette: Palette,
    on_click: F,
) -> Stateful<Div>
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    div()
        .id(id)
        .h(px(30.0))
        .px(px(12.0))
        .flex()
        .justify_center()
        .items_center()
        .rounded(px(6.0))
        .border_1()
        .border_color(if active {
            rgb(palette.accent)
        } else {
            rgba(palette.border)
        })
        .bg(if active {
            rgb(palette.accent)
        } else {
            rgba(0xffffff0b)
        })
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if active {
            rgb(0x17221d)
        } else {
            rgb(palette.text)
        })
        .cursor(CursorStyle::PointingHand)
        .hover(|this| this.opacity(0.88))
        .on_click(on_click)
        .child(label)
}

fn toggle_row<F>(
    label: &'static str,
    description: &'static str,
    enabled: bool,
    id: &'static str,
    palette: Palette,
    on_click: F,
) -> impl IntoElement
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(3.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(palette.accent))
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .line_height(px(15.0))
                        .text_color(rgb(palette.muted))
                        .child(description),
                ),
        )
        .child(segment_button(
            id,
            if enabled { "On" } else { "Off" },
            enabled,
            palette,
            on_click,
        ))
}

fn setting_group(
    label: &'static str,
    content: impl IntoElement,
    description: Option<&'static str>,
    palette: Palette,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(7.0))
        .child(
            div()
                .text_size(px(10.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(palette.accent))
                .child(label),
        )
        .child(content)
        .when_some(description, |this, description| {
            this.child(
                div()
                    .text_size(px(10.0))
                    .line_height(px(15.0))
                    .text_color(rgb(palette.muted))
                    .child(description),
            )
        })
}

fn stat_card(
    label: &'static str,
    value: impl Into<SharedString>,
    palette: Palette,
) -> impl IntoElement {
    div()
        .w(px(178.0))
        .h(px(70.0))
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(3.0))
        .px(px(12.0))
        .rounded(px(8.0))
        .bg(rgba(0xffffff0a))
        .border_1()
        .border_color(rgba(palette.border))
        .child(
            div()
                .text_size(px(9.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(palette.muted))
                .child(label),
        )
        .child(
            div()
                .text_size(px(18.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(palette.text))
                .child(value.into()),
        )
}

fn status_item(label: &'static str, value: String, color: gpui::Rgba) -> impl IntoElement + use<> {
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .text_size(px(9.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(label),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(color)
                .child(value),
        )
}
