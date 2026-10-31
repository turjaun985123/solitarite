//! Card motion: board layout, per-card poses and flight planning.
//!
//! This module is deliberately free of any GPUI types so the maths can be unit
//! tested. The idea is simple and robust: before a state change the UI takes a
//! snapshot of where every card is drawn (`poses`), applies the change to the
//! `Game`, snapshots again, and `plan` diffs the two snapshots. Every card that
//! changed position or face becomes a `Flight` that is animated in an overlay
//! while the real card is hidden at its destination. Because the diff is
//! computed from layout alone, moves, draws, recycles, undo/redo, auto-play
//! cascades, auto-finish and new deals all get smooth motion with no
//! per-action animation code.

use std::collections::HashMap;
use std::f32::consts::PI;

use crate::model::{Card, Game};

pub const GAP: f32 = 12.0;
/// Space between the top row and the tableau.
pub const ROW_GAP: f32 = 18.0;
/// Fan offset (fraction of card width) between visible draw-three waste cards.
pub const WASTE_FAN: f32 = 0.16;

const MENU_H: f32 = 34.0;
const TOOLBAR_H: f32 = 58.0;
const STATUS_H: f32 = 30.0;
const BOARD_PAD: f32 = 18.0;
const BOARD_BORDER: f32 = 1.0;

/// All board measurements, in logical pixels. Positions produced by `poses`
/// are relative to the board's inner container (its top-left is the stock).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub cw: f32,
    pub ch: f32,
    pub board_w: f32,
    pub board_h: f32,
    pub inner_h: f32,
    pub tableau_h: f32,
    pub up_step: f32,
    pub down_step: f32,
}

impl Layout {
    pub fn compute(vw: f32, vh: f32, game: &Game) -> Self {
        let cw = ((vw - 80.0 - GAP * 6.0) / 7.0).clamp(68.0, 124.0);
        let ch = cw * 1.42;
        let board_w = cw * 7.0 + GAP * 6.0;
        let board_h = (vh - MENU_H - TOOLBAR_H - STATUS_H - 20.0).max(430.0);
        let inner_h = board_h - BOARD_PAD * 2.0;
        let tableau_h = (inner_h - ch - ROW_GAP).max(180.0);

        let max_units = game
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

        Self {
            cw,
            ch,
            board_w,
            board_h,
            inner_h,
            tableau_h,
            up_step: step,
            down_step: (step * 0.42).max(4.0),
        }
    }

    pub fn column_x(&self, column: usize) -> f32 {
        column as f32 * (self.cw + GAP)
    }

    /// Top of the tableau row inside the board's inner container.
    pub fn tableau_y(&self) -> f32 {
        self.ch + ROW_GAP
    }

    /// Window-space position of the board's inner container (the stock's
    /// top-left corner). Used to convert mouse coordinates into board space.
    pub fn board_origin(&self, vw: f32, vh: f32) -> (f32, f32) {
        let outer_w = self.board_w + BOARD_PAD * 2.0;
        let left = vw / 2.0 - outer_w / 2.0;
        let area_h = vh - MENU_H - TOOLBAR_H - STATUS_H;
        let top = MENU_H + TOOLBAR_H + 10.0 + (area_h - 20.0 - self.board_h) / 2.0;
        (
            left + BOARD_BORDER + BOARD_PAD,
            top + BOARD_BORDER + BOARD_PAD,
        )
    }
}

/// Where one card is drawn, in board space.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub card: Card,
    pub x: f32,
    pub y: f32,
}

/// Draw position of every card in the game (always 52 for a valid deal).
pub fn poses(game: &Game, layout: &Layout) -> Vec<Pose> {
    let mut out = Vec::with_capacity(52);

    for card in &game.stock {
        out.push(Pose {
            card: *card,
            x: 0.0,
            y: 0.0,
        });
    }

    let waste_x = layout.column_x(1);
    let visible = if game.rules.draw_three { 3 } else { 1 };
    let start = game.waste.len().saturating_sub(visible);
    for (index, card) in game.waste.iter().enumerate() {
        let fan = if index >= start {
            (index - start) as f32 * layout.cw * WASTE_FAN
        } else {
            0.0
        };
        out.push(Pose {
            card: *card,
            x: waste_x + fan,
            y: 0.0,
        });
    }

    for (pile, cards) in game.foundations.iter().enumerate() {
        let x = layout.column_x(3 + pile);
        for card in cards {
            out.push(Pose {
                card: *card,
                x,
                y: 0.0,
            });
        }
    }

    for (pile, cards) in game.tableau.iter().enumerate() {
        let x = layout.column_x(pile);
        let mut offset = 0.0f32;
        for card in cards {
            out.push(Pose {
                card: *card,
                x,
                y: layout.tableau_y() + offset,
            });
            // Must mirror the accumulation used when the pile is rendered.
            offset += if card.face_up {
                layout.up_step
            } else {
                layout.down_step
            };
        }
    }

    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Normal move: quick launch, soft landing, small arc.
    Travel,
    /// Dealing from the stock.
    Deal,
    /// Settling from a mouse drag into its destination: short and direct.
    Settle,
    /// A rejected drag springing back to where it came from.
    Return,
}

/// One card's trip from `from` to `to`, with optional face flip.
#[derive(Clone, Copy, Debug)]
pub struct Flight {
    /// The card in its final state (`face_up` is the face it ends with).
    pub card: Card,
    pub from_face_up: bool,
    pub from: (f32, f32),
    pub to: (f32, f32),
    pub delay_ms: f32,
    pub duration_ms: f32,
    pub style: Style,
}

#[derive(Clone, Copy, Debug)]
pub struct FlipFrame {
    /// Horizontal foreshortening, 1.0 = flat to the camera, ~0 = edge-on.
    pub scale_x: f32,
    pub show_face_up: bool,
    /// Slight vertical growth at mid-flip: the card rising toward the player.
    pub boost: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    /// Normalised progress of this card's own flight, 0..=1.
    pub t: f32,
    /// True while the card should cast its larger "in the air" shadow.
    pub lifted: bool,
    pub flip: Option<FlipFrame>,
}

impl Flight {
    pub fn total_ms(&self) -> f32 {
        self.delay_ms + self.duration_ms
    }

    pub fn distance(&self) -> f32 {
        let dx = self.to.0 - self.from.0;
        let dy = self.to.1 - self.from.1;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn moves(&self) -> bool {
        self.distance() > 0.75
    }

    pub fn flips(&self) -> bool {
        self.from_face_up != self.card.face_up
    }

    /// The pose of this flight `elapsed_ms` after its batch began.
    pub fn frame(&self, elapsed_ms: f32) -> Frame {
        let t = ((elapsed_ms - self.delay_ms) / self.duration_ms.max(1.0)).clamp(0.0, 1.0);
        let eased = match self.style {
            Style::Travel => travel_ease(t),
            Style::Deal | Style::Settle => ease_out_cubic(t),
            Style::Return => ease_out_back(t),
        };
        let dx = self.to.0 - self.from.0;
        let dy = self.to.1 - self.from.1;
        let dist = self.distance();

        let arc = match self.style {
            Style::Travel => (dist * 0.08).min(26.0),
            Style::Deal => (dist * 0.05).min(18.0),
            Style::Settle | Style::Return => 0.0,
        };
        let lift = (PI * t).sin();
        let mut x = self.from.0 + dx * eased;
        let mut y = self.from.1 + dy * eased - lift * arc;

        // A gentle sideways drift on long Travel moves gives paths a natural
        // curve instead of a ruler-straight line.
        if self.style == Style::Travel && dist > 80.0 {
            let side = if dx >= 0.0 { 1.0 } else { -1.0 };
            x +=
                lift * side * (dist * 0.012).min(6.0) * if dy.abs() > dx.abs() { 1.0 } else { 0.0 };
        }
        y = y.max(-40.0);

        let flip = if self.flips() {
            let (f0, f1) = if self.moves() {
                (0.04, 0.72)
            } else {
                (0.0, 1.0)
            };
            let fp = ((t - f0) / (f1 - f0)).clamp(0.0, 1.0);
            let angle = PI * fp;
            Some(FlipFrame {
                scale_x: angle.cos().abs().max(0.02),
                show_face_up: if fp < 0.5 {
                    self.from_face_up
                } else {
                    self.card.face_up
                },
                boost: 1.0 + 0.06 * angle.sin(),
            })
        } else {
            None
        };

        let airborne = t > 0.0 && t < 1.0;
        Frame {
            x,
            y,
            t,
            lifted: airborne
                && (arc > 0.0 || flip.is_some() || matches!(self.style, Style::Return)),
            flip,
        }
    }
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t;
    1.0 - inv * inv * inv
}

/// Quick launch and soft landing: mostly ease-out with a touch of smoothstep
/// so the card visibly accelerates off its pile rather than teleporting.
pub fn travel_ease(t: f32) -> f32 {
    0.62 * ease_out_cubic(t) + 0.38 * smoothstep(t)
}

/// Ease-out with a small overshoot (a spring-back feel).
pub fn ease_out_back(t: f32) -> f32 {
    const C1: f32 = 1.35;
    const C3: f32 = C1 + 1.0;
    let u = t - 1.0;
    1.0 + C3 * u * u * u + C1 * u * u
}

fn duration_for(style: Style, dist: f32) -> f32 {
    match style {
        Style::Travel => (170.0 + dist.sqrt() * 9.5).clamp(210.0, 430.0),
        Style::Deal => (230.0 + dist.sqrt() * 6.0).clamp(260.0, 420.0),
        Style::Settle => (110.0 + dist.sqrt() * 7.0).clamp(130.0, 260.0),
        Style::Return => (190.0 + dist.sqrt() * 8.0).clamp(240.0, 400.0),
    }
}

/// Face-flip in place (no travel): how long it takes, and how long to wait so
/// the card that was covering it can leave first.
const FLIP_ONLY_MS: f32 = 320.0;
const FLIP_ONLY_DELAY_MS: f32 = 110.0;

/// Diff two pose snapshots into flights.
pub fn plan(before: &[Pose], after: &[Pose], style: Style) -> Vec<Flight> {
    let previous: HashMap<u8, &Pose> = before.iter().map(|pose| (pose.card.id, pose)).collect();

    struct Mover<'a> {
        pose: &'a Pose,
        from: &'a Pose,
        dist: f32,
    }

    let mut movers = Vec::new();
    for pose in after {
        let Some(from) = previous.get(&pose.card.id).copied() else {
            continue;
        };
        let dx = pose.x - from.x;
        let dy = pose.y - from.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let flipped = from.card.face_up != pose.card.face_up;
        if dist > 0.75 || flipped {
            movers.push(Mover { pose, from, dist });
        }
    }

    // Only big movers are staggered; small fan shifts (draw-three waste) move
    // immediately so the pile re-settles while the new cards arrive.
    let travellers = movers.iter().filter(|m| m.dist > 40.0).count().max(1);
    let base_stagger: f32 = match style {
        Style::Settle | Style::Return => 12.0,
        _ if travellers > 12 => 12.0,
        _ if travellers > 4 => 24.0,
        _ => 55.0,
    };
    let stagger = base_stagger.min(380.0 / travellers as f32);

    let mut rank = 0usize;
    let mut flights = Vec::with_capacity(movers.len());
    for mover in movers {
        let moving = mover.dist > 0.75;
        let (delay, duration) = if !moving {
            (FLIP_ONLY_DELAY_MS, FLIP_ONLY_MS)
        } else if mover.dist > 40.0 {
            let delay = rank as f32 * stagger;
            rank += 1;
            (delay, duration_for(style, mover.dist))
        } else {
            (0.0, duration_for(style, mover.dist).min(240.0))
        };
        flights.push(Flight {
            card: mover.pose.card,
            from_face_up: mover.from.card.face_up,
            from: (mover.from.x, mover.from.y),
            to: (mover.pose.x, mover.pose.y),
            delay_ms: delay,
            duration_ms: duration,
            style,
        });
    }
    flights
}

/// Flights that deal a fresh game: every tableau card travels from the stock,
/// one row at a time like a real deal, flipping the top card of each column.
pub fn deal_plan(game: &Game, layout: &Layout) -> Vec<Flight> {
    let after = poses(game, layout);

    let mut order: HashMap<u8, usize> = HashMap::new();
    let mut next = 0usize;
    for row in 0..7 {
        for pile in row..7 {
            if let Some(card) = game.tableau[pile].get(row) {
                order.insert(card.id, next);
                next += 1;
            }
        }
    }

    let mut flights: Vec<Flight> = after
        .iter()
        .filter_map(|pose| {
            let rank = *order.get(&pose.card.id)?;
            Some(Flight {
                card: pose.card,
                from_face_up: false,
                from: (0.0, 0.0),
                to: (pose.x, pose.y),
                delay_ms: 30.0 + rank as f32 * 26.0,
                duration_ms: duration_for(
                    Style::Deal,
                    ((pose.x * pose.x) + (pose.y * pose.y)).sqrt(),
                ),
                style: Style::Deal,
            })
        })
        .collect();
    flights.sort_by(|a, b| a.delay_ms.total_cmp(&b.delay_ms));
    flights
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Game, PileRef, Rules};

    fn game() -> Game {
        Game::new(42, Rules::default(), false)
    }

    fn layout(game: &Game) -> Layout {
        Layout::compute(1120.0, 760.0, game)
    }

    #[test]
    fn every_card_has_exactly_one_pose() {
        let game = game();
        let poses = poses(&game, &layout(&game));
        assert_eq!(poses.len(), 52);
        let mut ids: Vec<u8> = poses.iter().map(|pose| pose.card.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 52);
    }

    #[test]
    fn tableau_columns_are_evenly_spaced() {
        let game = game();
        let l = layout(&game);
        for pile in 0..7 {
            assert!((l.column_x(pile) - pile as f32 * (l.cw + GAP)).abs() < 1e-3);
        }
        let all = poses(&game, &l);
        let first = game.tableau[3][0].id;
        let pose = all.iter().find(|pose| pose.card.id == first).unwrap();
        assert!((pose.x - l.column_x(3)).abs() < 1e-3);
        assert!((pose.y - l.tableau_y()).abs() < 1e-3);
    }

    #[test]
    fn deal_plan_animates_exactly_the_tableau() {
        let game = game();
        let flights = deal_plan(&game, &layout(&game));
        assert_eq!(flights.len(), 28);
        assert_eq!(flights.iter().filter(|f| f.flips()).count(), 7);
        assert!(flights.iter().all(|f| f.moves()));
        // The last card dealt starts last.
        let max_delay = flights.iter().map(|f| f.delay_ms).fold(0.0, f32::max);
        assert!(max_delay > 600.0 && max_delay < 1000.0);
    }

    #[test]
    fn drawing_flies_a_flipping_card_from_stock_to_waste() {
        let mut game = game();
        let l = layout(&game);
        let before = poses(&game, &l);
        assert!(game.draw());
        let after = poses(&game, &l);
        let flights = plan(&before, &after, Style::Travel);
        assert_eq!(flights.len(), 1);
        let flight = flights[0];
        assert!(flight.flips() && flight.moves());
        assert!(!flight.from_face_up && flight.card.face_up);
        assert!((flight.from.0 - 0.0).abs() < 1e-3);
        assert!((flight.to.0 - l.column_x(1)).abs() < 1e-3);
    }

    #[test]
    fn unchanged_positions_make_no_flights() {
        let game = game();
        let l = layout(&game);
        let snapshot = poses(&game, &l);
        assert!(plan(&snapshot, &snapshot, Style::Travel).is_empty());
    }

    #[test]
    fn frames_start_at_origin_and_land_exactly_on_target() {
        let game = game();
        let l = layout(&game);
        let before = poses(&game, &l);
        let mut moved = game.clone();
        assert!(moved.draw());
        let flights = plan(&before, &poses(&moved, &l), Style::Travel);
        let flight = flights[0];

        let start = flight.frame(0.0);
        assert!((start.x - flight.from.0).abs() < 1e-3 && (start.y - flight.from.1).abs() < 1e-3);

        let end = flight.frame(flight.total_ms());
        assert!((end.x - flight.to.0).abs() < 1e-3 && (end.y - flight.to.1).abs() < 1e-3);
        assert!(!end.lifted);
        let flip = end.flip.expect("flip finishes");
        assert!(flip.show_face_up && (flip.scale_x - 1.0).abs() < 1e-3);
    }

    #[test]
    fn flip_passes_edge_on_with_the_face_changing_at_the_midpoint() {
        let flight = Flight {
            card: Card::new(0, crate::model::Suit::Spades, crate::model::Rank::Ace, true),
            from_face_up: false,
            from: (0.0, 0.0),
            to: (0.0, 0.0),
            delay_ms: 0.0,
            duration_ms: 300.0,
            style: Style::Travel,
        };
        let early = flight.frame(30.0).flip.unwrap();
        let mid = flight.frame(150.0).flip.unwrap();
        let late = flight.frame(270.0).flip.unwrap();
        assert!(!early.show_face_up && early.scale_x > 0.8);
        assert!(mid.scale_x < 0.1);
        assert!(late.show_face_up && late.scale_x > 0.8);
    }

    #[test]
    fn return_ease_overshoots_then_settles() {
        assert!(ease_out_back(0.0).abs() < 1e-5);
        assert!((ease_out_back(1.0) - 1.0).abs() < 1e-5);
        assert!((0..100).any(|i| ease_out_back(i as f32 / 100.0) > 1.0));
    }

    #[test]
    fn board_origin_stays_inside_the_window() {
        let game = game();
        for (vw, vh) in [(1120.0, 760.0), (820.0, 600.0), (1600.0, 1000.0)] {
            let l = Layout::compute(vw, vh, &game);
            let (ox, oy) = l.board_origin(vw, vh);
            assert!(ox >= 0.0 && oy >= 92.0, "origin {ox},{oy} at {vw}x{vh}");
        }
    }

    #[test]
    fn moving_a_run_staggers_its_cards() {
        let mut game = game();
        let l = layout(&game);
        // Find any legal tableau-to-tableau or waste move to animate.
        let mut found = None;
        'outer: for source in 0..7 {
            for index in 0..game.tableau[source].len() {
                for dest in 0..7 {
                    if game.can_move(PileRef::Tableau(source), index, PileRef::Tableau(dest)) {
                        found = Some((source, index, dest));
                        break 'outer;
                    }
                }
            }
        }
        if let Some((source, index, dest)) = found {
            let before = poses(&game, &l);
            assert!(game.try_move(
                PileRef::Tableau(source),
                index,
                PileRef::Tableau(dest),
                false
            ));
            let l2 = Layout::compute(1120.0, 760.0, &game);
            let flights = plan(&before, &poses(&game, &l2), Style::Travel);
            assert!(!flights.is_empty());
            assert!(flights.iter().all(|f| f.total_ms() < 1200.0));
        }
    }
}
