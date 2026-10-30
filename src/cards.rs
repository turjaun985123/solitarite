use gpui::{
    App, Context, Div, FontWeight, Render, SharedString, Window, div, linear_color_stop,
    linear_gradient, prelude::*, px, rgb,
};

use crate::model::{Card, Rank};
use crate::persistence::CardBack;
use crate::theme::{
    Fade as _, card_back_colors, card_black, card_cream, card_edge, card_paper, card_red,
};

fn pip_positions(rank: Rank) -> &'static [(f32, f32)] {
    match rank {
        Rank::Two => &[(0.50, 0.28), (0.50, 0.72)],
        Rank::Three => &[(0.50, 0.27), (0.50, 0.50), (0.50, 0.73)],
        Rank::Four => &[(0.30, 0.28), (0.70, 0.28), (0.30, 0.72), (0.70, 0.72)],
        Rank::Five => &[
            (0.30, 0.27),
            (0.70, 0.27),
            (0.50, 0.50),
            (0.30, 0.73),
            (0.70, 0.73),
        ],
        Rank::Six => &[
            (0.30, 0.27),
            (0.70, 0.27),
            (0.30, 0.50),
            (0.70, 0.50),
            (0.30, 0.73),
            (0.70, 0.73),
        ],
        Rank::Seven => &[
            (0.30, 0.26),
            (0.70, 0.26),
            (0.30, 0.48),
            (0.70, 0.48),
            (0.50, 0.37),
            (0.30, 0.74),
            (0.70, 0.74),
        ],
        Rank::Eight => &[
            (0.30, 0.25),
            (0.70, 0.25),
            (0.30, 0.48),
            (0.70, 0.48),
            (0.50, 0.365),
            (0.50, 0.635),
            (0.30, 0.75),
            (0.70, 0.75),
        ],
        Rank::Nine => &[
            (0.30, 0.24),
            (0.70, 0.24),
            (0.30, 0.46),
            (0.70, 0.46),
            (0.50, 0.50),
            (0.30, 0.76),
            (0.70, 0.76),
            (0.50, 0.34),
            (0.50, 0.66),
        ],
        Rank::Ten => &[
            (0.30, 0.23),
            (0.70, 0.23),
            (0.50, 0.27),
            (0.30, 0.45),
            (0.70, 0.45),
            (0.50, 0.50),
            (0.30, 0.68),
            (0.70, 0.68),
            (0.50, 0.73),
            (0.50, 0.39),
        ],
        _ => &[],
    }
}

pub fn card_face(card: Card, cw: f32, ch: f32) -> Div {
    let ink = if card.is_red() {
        card_red()
    } else {
        card_black()
    };
    let corner_rank = (cw * 0.185).max(12.0);
    let corner_suit = (cw * 0.135).max(9.0);

    let mut element = div()
        .relative()
        .w(px(cw))
        .h(px(ch))
        .bg(card_paper())
        .rounded(px(cw * 0.065))
        .border_1()
        .border_color(card_edge())
        .shadow_sm()
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .top(px(3.0))
                .left(px(3.0))
                .right(px(3.0))
                .bottom(px(3.0))
                .rounded(px(cw * 0.045))
                .border_1()
                .border_color(rgb(0x00000008)),
        )
        .child(
            div()
                .absolute()
                .top(px(ch * 0.035))
                .left(px(cw * 0.055))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(cw * 0.018))
                .text_color(ink)
                .child(
                    div()
                        .text_size(px(corner_rank))
                        .line_height(px(corner_rank * 1.02))
                        .font_weight(FontWeight::BOLD)
                        .child(card.rank.short()),
                )
                .child(
                    div()
                        .text_size(px(corner_suit))
                        .line_height(px(corner_suit))
                        .child(card.suit.symbol()),
                ),
        )
        .child(
            div()
                .absolute()
                .bottom(px(ch * 0.032))
                .right(px(cw * 0.055))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(cw * 0.018))
                .text_color(ink)
                .opacity(0.88)
                .child(
                    div()
                        .text_size(px(corner_suit))
                        .line_height(px(corner_suit))
                        .child(card.suit.symbol()),
                )
                .child(
                    div()
                        .text_size(px(corner_rank * 0.78))
                        .line_height(px(corner_rank * 0.8))
                        .font_weight(FontWeight::BOLD)
                        .child(card.rank.short()),
                ),
        );

    match card.rank {
        Rank::Ace => {
            element = element.child(
                div()
                    .absolute()
                    .top(px(ch * 0.255))
                    .left(px(cw * 0.235))
                    .w(px(cw * 0.53))
                    .h(px(ch * 0.49))
                    .rounded_full()
                    .border_1()
                    .border_color(ink.fade(0.22))
                    .flex()
                    .justify_center()
                    .items_center()
                    .text_color(ink)
                    .text_size(px(cw * 0.45))
                    .child(card.suit.symbol()),
            );
        }
        Rank::Jack | Rank::Queen | Rank::King => {
            element = element.child(
                div()
                    .absolute()
                    .top(px(ch * 0.205))
                    .left(px(cw * 0.145))
                    .w(px(cw * 0.71))
                    .h(px(ch * 0.56))
                    .rounded(px(cw * 0.06))
                    .border_1()
                    .border_color(ink.fade(0.22))
                    .bg(linear_gradient(
                        145.,
                        linear_color_stop(card_cream(), 0.0),
                        linear_color_stop(rgb(0xf2e8ce), 1.0),
                    ))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(cw * 0.31))
                            .line_height(px(cw * 0.31))
                            .font_weight(FontWeight::BOLD)
                            .text_color(ink)
                            .child(card.rank.short()),
                    )
                    .child(
                        div()
                            .mt(px(ch * 0.015))
                            .text_size(px(cw * 0.19))
                            .line_height(px(cw * 0.2))
                            .text_color(ink)
                            .child(card.suit.symbol()),
                    )
                    .child(
                        div()
                            .mt(px(ch * 0.018))
                            .text_size(px(cw * 0.075))
                            .line_height(px(cw * 0.08))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ink.fade(0.45))
                            .child("KLONDIKE"),
                    ),
            );
        }
        rank => {
            for (x, y) in pip_positions(rank) {
                element = element.child(
                    div()
                        .absolute()
                        .left(px(cw * x - cw * 0.095))
                        .top(px(ch * y - ch * 0.075))
                        .w(px(cw * 0.19))
                        .h(px(ch * 0.15))
                        .flex()
                        .justify_center()
                        .items_center()
                        .text_color(ink)
                        .text_size(px(cw * 0.185))
                        .line_height(px(ch * 0.15))
                        .child(card.suit.symbol()),
                );
            }
        }
    }

    element
}

pub fn card_back(cw: f32, ch: f32, back: CardBack) -> Div {
    let (top, bottom, accent, symbol) = card_back_colors(back);
    let accent = rgb(accent);
    let emblem = (cw * 0.20).max(14.0);

    div()
        .relative()
        .w(px(cw))
        .h(px(ch))
        .rounded(px(cw * 0.065))
        .border_1()
        .border_color(accent.fade(0.7))
        .bg(linear_gradient(
            135.,
            linear_color_stop(rgb(top), 0.0),
            linear_color_stop(rgb(bottom), 1.0),
        ))
        .shadow_sm()
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .top(px(4.0))
                .left(px(4.0))
                .right(px(4.0))
                .bottom(px(4.0))
                .rounded(px(cw * 0.045))
                .border_1()
                .border_color(accent.fade(0.52)),
        )
        .child(
            div()
                .absolute()
                .top(px(ch * 0.5 - cw * 0.18))
                .left(px(cw * 0.5 - cw * 0.18))
                .w(px(cw * 0.36))
                .h(px(cw * 0.36))
                .rounded_full()
                .border_1()
                .border_color(accent.fade(0.7))
                .flex()
                .justify_center()
                .items_center()
                .text_color(accent.fade(0.92))
                .text_size(px(emblem))
                .child(symbol),
        )
        .child(
            div()
                .absolute()
                .top(px(ch * 0.07))
                .left(px(cw * 0.09))
                .text_size(px(cw * 0.085))
                .text_color(accent.fade(0.38))
                .child(symbol),
        )
        .child(
            div()
                .absolute()
                .bottom(px(ch * 0.07))
                .right(px(cw * 0.09))
                .text_size(px(cw * 0.085))
                .text_color(accent.fade(0.38))
                .child(symbol),
        )
        .child(
            div()
                .absolute()
                .top(px(ch * 0.11))
                .right(px(cw * 0.12))
                .w(px(cw * 0.035))
                .h(px(cw * 0.035))
                .rounded_full()
                .bg(accent.fade(0.32)),
        )
        .child(
            div()
                .absolute()
                .bottom(px(ch * 0.11))
                .left(px(cw * 0.12))
                .w(px(cw * 0.035))
                .h(px(cw * 0.035))
                .rounded_full()
                .bg(accent.fade(0.32)),
        )
}

pub fn card_view(card: Card, cw: f32, ch: f32, back: CardBack) -> Div {
    if card.face_up {
        card_face(card, cw, ch)
    } else {
        card_back(cw, ch, back)
    }
}

pub struct DragGhost {
    cards: Vec<Card>,
    cw: f32,
    ch: f32,
    back: CardBack,
}

impl DragGhost {
    pub fn new(cards: Vec<Card>, cw: f32, ch: f32, back: CardBack) -> Self {
        Self {
            cards,
            cw,
            ch,
            back,
        }
    }
}

impl Render for DragGhost {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let step = self.ch * 0.24;
        let count = self.cards.len().max(1);
        let mut root = div()
            .relative()
            .w(px(self.cw))
            .h(px(self.ch + step * (count.saturating_sub(1)) as f32))
            .opacity(0.94);

        for (index, card) in self.cards.iter().enumerate() {
            root = root.child(
                div()
                    .absolute()
                    .top(px(step * index as f32))
                    .left(px(0.0))
                    .shadow_lg()
                    .child(card_view(*card, self.cw, self.ch, self.back)),
            );
        }
        root
    }
}

pub fn card_label(card: Card) -> SharedString {
    SharedString::from(format!("{} {}", card.rank.short(), card.suit.symbol()))
}

#[allow(dead_code)]
fn _assert_render(_: &App) {}
