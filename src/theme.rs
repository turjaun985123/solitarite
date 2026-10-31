use gpui::{Rgba, rgb, rgba};

use crate::model::{CardColor, Suit};
use crate::persistence::{CardBack, Theme};

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg_top: u32,
    pub bg_bottom: u32,
    pub felt_top: u32,
    pub felt_bottom: u32,
    pub board_top: u32,
    pub board_bottom: u32,
    pub accent: u32,
    pub accent_soft: u32,
    pub text: u32,
    pub muted: u32,
    pub panel: u32,
    pub border: u32,
    pub success: u32,
    pub danger: u32,
}

impl Theme {
    pub fn palette(self) -> Palette {
        match self {
            Theme::Green => Palette {
                bg_top: 0x071f18,
                bg_bottom: 0x020b08,
                felt_top: 0x0d5c43,
                felt_bottom: 0x063426,
                board_top: 0x0b4d38,
                board_bottom: 0x073325,
                accent: 0xe8bf68,
                accent_soft: 0xf3d99a,
                text: 0xf8f4e8,
                muted: 0xa9c3b8,
                panel: 0x0b1714f4,
                border: 0xffffff1f,
                success: 0x7ddba3,
                danger: 0xff8b82,
            },
            Theme::Midnight => Palette {
                bg_top: 0x07101f,
                bg_bottom: 0x02050a,
                felt_top: 0x12395d,
                felt_bottom: 0x081d33,
                board_top: 0x0f3354,
                board_bottom: 0x07182a,
                accent: 0x7bd0ff,
                accent_soft: 0xb7e8ff,
                text: 0xf2f7fc,
                muted: 0x9eb6cb,
                panel: 0x08111df4,
                border: 0xffffff1f,
                success: 0x7ddba3,
                danger: 0xff9d95,
            },
            Theme::Burgundy => Palette {
                bg_top: 0x1b080c,
                bg_bottom: 0x080203,
                felt_top: 0x6b1f2a,
                felt_bottom: 0x3c0d16,
                board_top: 0x5a1923,
                board_bottom: 0x2f0a11,
                accent: 0xf0c36a,
                accent_soft: 0xffe2a8,
                text: 0xfff7ed,
                muted: 0xd1aea9,
                panel: 0x18090cf4,
                border: 0xffffff22,
                success: 0x8fe0a8,
                danger: 0xff9d95,
            },
        }
    }
}

pub fn card_red() -> Rgba {
    rgb(0xc3313a)
}

pub fn card_black() -> Rgba {
    rgb(0x18232d)
}

pub fn card_suit_ink(suit: Suit, four_color: bool) -> Rgba {
    if four_color {
        match suit {
            Suit::Diamonds => rgb(0x1b6ec2),
            Suit::Clubs => rgb(0x1e824c),
            Suit::Hearts => card_red(),
            Suit::Spades => card_black(),
        }
    } else if suit.color() == CardColor::Red {
        card_red()
    } else {
        card_black()
    }
}

pub fn card_cream() -> Rgba {
    rgb(0xfcfaf4)
}

pub fn card_paper() -> Rgba {
    rgb(0xf8f6f0)
}

pub fn card_edge() -> Rgba {
    rgb(0xd7d1c2)
}

pub fn gold() -> Rgba {
    rgb(0xe8bf68)
}

pub fn overlay() -> Rgba {
    rgba(0x030a08c9)
}

pub fn card_back_colors(back: CardBack) -> (u32, u32, u32, &'static str) {
    match back {
        CardBack::Midnight => (0x0b1d3a, 0x07101f, 0xe7c477, "♠"),
        CardBack::Crimson => (0x6f1d2b, 0x2b0910, 0xf3d7a0, "♥"),
        CardBack::Emerald => (0x0e4b39, 0x04251c, 0xe6ca83, "♣"),
        CardBack::Royal => (0x3d256b, 0x170d31, 0xd9d4ff, "♦"),
    }
}

/// Multiply a color's alpha channel. GPUI's `Rgba` has no `opacity` helper of
/// its own (unlike `Hsla`), so this keeps the UI code expressive.
pub trait Fade {
    fn fade(self, factor: f32) -> Rgba;
}

impl Fade for Rgba {
    fn fade(self, factor: f32) -> Rgba {
        Rgba {
            a: (self.a * factor).clamp(0.0, 1.0),
            ..self
        }
    }
}
