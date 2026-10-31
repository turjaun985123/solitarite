use std::collections::HashSet;

use rand::{SeedableRng, seq::SliceRandom};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Suit {
    Spades,
    Hearts,
    Diamonds,
    Clubs,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];

    pub fn index(self) -> usize {
        match self {
            Suit::Spades => 0,
            Suit::Hearts => 1,
            Suit::Diamonds => 2,
            Suit::Clubs => 3,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            Suit::Spades => "♠",
            Suit::Hearts => "♥",
            Suit::Diamonds => "♦",
            Suit::Clubs => "♣",
        }
    }

    pub fn color(self) -> CardColor {
        match self {
            Suit::Spades | Suit::Clubs => CardColor::Black,
            Suit::Hearts | Suit::Diamonds => CardColor::Red,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardColor {
    Black,
    Red,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rank {
    Ace,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
}

impl Rank {
    pub fn value(self) -> u8 {
        match self {
            Rank::Ace => 1,
            Rank::Two => 2,
            Rank::Three => 3,
            Rank::Four => 4,
            Rank::Five => 5,
            Rank::Six => 6,
            Rank::Seven => 7,
            Rank::Eight => 8,
            Rank::Nine => 9,
            Rank::Ten => 10,
            Rank::Jack => 11,
            Rank::Queen => 12,
            Rank::King => 13,
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Rank::Ace => "A",
            Rank::Two => "2",
            Rank::Three => "3",
            Rank::Four => "4",
            Rank::Five => "5",
            Rank::Six => "6",
            Rank::Seven => "7",
            Rank::Eight => "8",
            Rank::Nine => "9",
            Rank::Ten => "10",
            Rank::Jack => "J",
            Rank::Queen => "Q",
            Rank::King => "K",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub id: u8,
    pub suit: Suit,
    pub rank: Rank,
    pub face_up: bool,
}

impl Card {
    pub fn new(id: u8, suit: Suit, rank: Rank, face_up: bool) -> Self {
        Self {
            id,
            suit,
            rank,
            face_up,
        }
    }

    pub fn is_red(self) -> bool {
        self.suit.color() == CardColor::Red
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scoring {
    Standard,
    Vegas,
}

impl Scoring {
    pub fn label(self) -> &'static str {
        match self {
            Scoring::Standard => "Standard",
            Scoring::Vegas => "Vegas",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rules {
    pub draw_three: bool,
    pub scoring: Scoring,
    pub timed: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            draw_three: false,
            scoring: Scoring::Standard,
            timed: true,
        }
    }
}

impl Rules {
    pub fn draw_label(self) -> &'static str {
        if self.draw_three { "Draw 3" } else { "Draw 1" }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PileRef {
    Waste,
    Foundation(usize),
    Tableau(usize),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    stock: Vec<Card>,
    waste: Vec<Card>,
    foundations: [Vec<Card>; 4],
    tableau: [Vec<Card>; 7],
    score: i64,
    moves: u32,
    draw_passes: u32,
    won: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    Move {
        source: PileRef,
        index: usize,
        dest: PileRef,
    },
    Draw,
    Recycle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    pub seed: u64,
    pub deal_number: u32,
    pub is_daily: bool,
    pub rules: Rules,
    pub stock: Vec<Card>,
    pub waste: Vec<Card>,
    pub foundations: [Vec<Card>; 4],
    pub tableau: [Vec<Card>; 7],
    pub score: i64,
    pub moves: u32,
    pub elapsed: u64,
    pub paused: bool,
    pub won: bool,
    pub draw_passes: u32,
    #[serde(default)]
    pub undo: Vec<Snapshot>,
    #[serde(default)]
    pub redo: Vec<Snapshot>,
}

impl Game {
    pub fn new(seed: u64, rules: Rules, is_daily: bool) -> Self {
        let mut deck = Vec::with_capacity(52);
        let mut id = 0u8;
        for suit in Suit::ALL {
            for rank in [
                Rank::Ace,
                Rank::Two,
                Rank::Three,
                Rank::Four,
                Rank::Five,
                Rank::Six,
                Rank::Seven,
                Rank::Eight,
                Rank::Nine,
                Rank::Ten,
                Rank::Jack,
                Rank::Queen,
                Rank::King,
            ] {
                deck.push(Card::new(id, suit, rank, false));
                id += 1;
            }
        }

        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        deck.shuffle(&mut rng);

        let mut tableau: [Vec<Card>; 7] = Default::default();
        let mut next = 0usize;
        for pile in 0..7 {
            for row in 0..=pile {
                let mut card = deck[next];
                card.face_up = row == pile;
                tableau[pile].push(card);
                next += 1;
            }
        }

        let mut stock = deck[next..].to_vec();
        for card in &mut stock {
            card.face_up = false;
        }

        Self {
            seed,
            deal_number: (seed % 1_000_000) as u32,
            is_daily,
            rules,
            stock,
            waste: Vec::new(),
            foundations: Default::default(),
            tableau,
            score: if rules.scoring == Scoring::Vegas {
                -52
            } else {
                0
            },
            moves: 0,
            elapsed: 0,
            paused: false,
            won: false,
            draw_passes: 0,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        let mut ids = HashSet::new();
        let all = self
            .stock
            .iter()
            .chain(self.waste.iter())
            .chain(self.foundations.iter().flatten())
            .chain(self.tableau.iter().flatten());

        let mut total = 0usize;
        for card in all {
            total += 1;
            let canonical_id = card.suit.index() * 13 + card.rank.value() as usize - 1;
            if !ids.insert(card.id) || card.id as usize != canonical_id {
                return false;
            }
        }
        total == 52
            && self.stock.iter().all(|card| !card.face_up)
            && self.waste.iter().all(|card| card.face_up)
            && self.foundations.iter().enumerate().all(|(suit, pile)| {
                pile.iter().enumerate().all(|(rank, card)| {
                    card.face_up
                        && card.suit.index() == suit
                        && card.rank.value() as usize == rank + 1
                })
            })
            && self.tableau.iter().all(|pile| {
                let mut found_face_up = false;
                pile.iter().all(|card| {
                    if card.face_up {
                        found_face_up = true;
                        true
                    } else {
                        !found_face_up
                    }
                })
            })
            && self.undo.len() <= 300
            && self.redo.len() <= 300
            && self.undo.iter().chain(&self.redo).all(Snapshot::is_valid)
    }

    pub fn cards_at(&self, source: PileRef, index: usize) -> Option<&[Card]> {
        match source {
            PileRef::Waste => {
                if index + 1 == self.waste.len()
                    && self.waste.get(index).is_some_and(|card| card.face_up)
                {
                    Some(&self.waste[index..])
                } else {
                    None
                }
            }
            PileRef::Foundation(pile) => {
                if pile >= 4 || index + 1 != self.foundations[pile].len() {
                    return None;
                }
                Some(&self.foundations[pile][index..])
            }
            PileRef::Tableau(pile) => {
                if pile >= 7 {
                    return None;
                }
                let cards = &self.tableau[pile];
                if index >= cards.len() || cards[index..].iter().any(|card| !card.face_up) {
                    None
                } else {
                    Some(&cards[index..])
                }
            }
        }
    }

    pub fn can_move(&self, source: PileRef, index: usize, dest: PileRef) -> bool {
        if source == dest {
            return false;
        }
        let Some(cards) = self.cards_at(source, index) else {
            return false;
        };
        let Some(first) = cards.first().copied() else {
            return false;
        };
        if !first.face_up {
            return false;
        }

        match dest {
            PileRef::Waste => false,
            PileRef::Foundation(pile) => {
                if pile >= 4 || cards.len() != 1 || first.suit.index() != pile {
                    return false;
                }
                let foundation = &self.foundations[pile];
                first.rank.value() == foundation.len() as u8 + 1
                    && foundation.iter().all(|card| card.suit == first.suit)
            }
            PileRef::Tableau(pile) => {
                if pile >= 7 {
                    return false;
                }
                let target = &self.tableau[pile];
                match target.last() {
                    None => first.rank == Rank::King,
                    Some(top) => {
                        top.face_up
                            && top.suit.color() != first.suit.color()
                            && top.rank.value() == first.rank.value() + 1
                    }
                }
            }
        }
    }

    pub fn foundation_destination(&self, source: PileRef, index: usize) -> Option<PileRef> {
        let cards = self.cards_at(source, index)?;
        let first = *cards.first()?;
        let dest = PileRef::Foundation(first.suit.index());
        self.can_move(source, index, dest).then_some(dest)
    }

    /// Choose a sensible one-click destination: foundation first, then the
    /// deepest legal occupied tableau column, then an empty column for a King.
    pub fn smart_move_destination(&self, source: PileRef, index: usize) -> Option<PileRef> {
        if let Some(dest) = self.foundation_destination(source, index) {
            return Some(dest);
        }

        (0..7)
            .filter(|&pile| !self.tableau[pile].is_empty())
            .filter(|&pile| self.can_move(source, index, PileRef::Tableau(pile)))
            .max_by_key(|&pile| self.tableau[pile].len())
            .map(PileRef::Tableau)
            .or_else(|| {
                (0..7)
                    .find(|&pile| {
                        self.tableau[pile].is_empty()
                            && self.can_move(source, index, PileRef::Tableau(pile))
                    })
                    .map(PileRef::Tableau)
            })
    }

    pub fn draw(&mut self) -> bool {
        let before = self.snapshot();
        if self.draw_internal() {
            self.record(before);
            true
        } else {
            false
        }
    }

    fn draw_internal(&mut self) -> bool {
        if self.stock.is_empty() {
            if self.waste.is_empty() {
                return false;
            }
            let mut recycled: Vec<Card> = self.waste.drain(..).rev().collect();
            for card in &mut recycled {
                card.face_up = false;
            }
            self.stock = recycled;
            self.draw_passes += 1;
            if self.rules.scoring == Scoring::Standard {
                let penalty = if self.rules.draw_three { -20 } else { -100 };
                self.adjust_score(penalty);
            }
        }

        let count = if self.rules.draw_three { 3 } else { 1 };
        let mut drawn = 0;
        for _ in 0..count {
            if let Some(mut card) = self.stock.pop() {
                card.face_up = true;
                self.waste.push(card);
                drawn += 1;
            }
        }

        if drawn == 0 {
            return false;
        }
        self.moves += 1;
        true
    }

    pub fn try_move(
        &mut self,
        source: PileRef,
        index: usize,
        dest: PileRef,
        auto_play: bool,
    ) -> bool {
        if !self.can_move(source, index, dest) {
            return false;
        }
        let before = self.snapshot();
        if !self.apply_move(source, index, dest) {
            return false;
        }
        if auto_play {
            while self.auto_move_one_internal() {}
        }
        self.record(before);
        true
    }

    fn apply_move(&mut self, source: PileRef, index: usize, dest: PileRef) -> bool {
        if !self.can_move(source, index, dest) {
            return false;
        }
        let cards = self.take_cards(source, index);

        match dest {
            PileRef::Foundation(_) => self.adjust_score(match self.rules.scoring {
                Scoring::Standard => 10,
                Scoring::Vegas => 5,
            }),
            PileRef::Tableau(_) => {
                if matches!(source, PileRef::Waste) && self.rules.scoring == Scoring::Standard {
                    self.adjust_score(5);
                }
                if matches!(source, PileRef::Foundation(_)) {
                    self.adjust_score(match self.rules.scoring {
                        Scoring::Standard => -15,
                        Scoring::Vegas => -5,
                    });
                }
            }
            PileRef::Waste => unreachable!(),
        }

        match dest {
            PileRef::Foundation(pile) => self.foundations[pile].extend(cards),
            PileRef::Tableau(pile) => self.tableau[pile].extend(cards),
            PileRef::Waste => unreachable!(),
        }

        if let PileRef::Tableau(pile) = source
            && let Some(top) = self.tableau[pile].last_mut()
            && !top.face_up
        {
            top.face_up = true;
            if self.rules.scoring == Scoring::Standard {
                self.adjust_score(5);
            }
        }

        self.moves += 1;
        self.update_win();
        true
    }

    fn take_cards(&mut self, source: PileRef, index: usize) -> Vec<Card> {
        match source {
            PileRef::Waste => self.waste.pop().into_iter().collect(),
            PileRef::Foundation(pile) => self.foundations[pile].pop().into_iter().collect(),
            PileRef::Tableau(pile) => self.tableau[pile].split_off(index),
        }
    }

    pub fn undo(&mut self) -> bool {
        if self.won {
            return false;
        }
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.redo.push(current);
        self.restore(snapshot);
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.won {
            return false;
        }
        let Some(snapshot) = self.redo.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.undo.push(current);
        self.restore(snapshot);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.won && !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.won && !self.redo.is_empty()
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            stock: self.stock.clone(),
            waste: self.waste.clone(),
            foundations: self.foundations.clone(),
            tableau: self.tableau.clone(),
            score: self.score,
            moves: self.moves,
            draw_passes: self.draw_passes,
            won: self.won,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.stock = snapshot.stock;
        self.waste = snapshot.waste;
        self.foundations = snapshot.foundations;
        self.tableau = snapshot.tableau;
        self.score = snapshot.score;
        self.moves = snapshot.moves;
        self.draw_passes = snapshot.draw_passes;
        self.won = snapshot.won;
    }

    fn record(&mut self, before: Snapshot) {
        self.undo.push(before);
        if self.undo.len() > 300 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn adjust_score(&mut self, delta: i64) {
        self.score += delta;
        if self.rules.scoring == Scoring::Standard && self.score < 0 {
            self.score = 0;
        }
    }

    fn update_win(&mut self) {
        if !self.won && self.foundations.iter().all(|pile| pile.len() == 13) {
            self.won = true;
            if self.rules.scoring == Scoring::Standard && self.rules.timed && self.elapsed >= 30 {
                self.score += 700_000 / self.elapsed.max(1) as i64;
            }
        }
    }

    pub fn tick(&mut self) -> bool {
        if self.paused || self.won {
            return false;
        }
        self.elapsed += 1;
        if self.rules.scoring == Scoring::Standard
            && self.rules.timed
            && self.elapsed > 30
            && self.elapsed % 10 == 0
        {
            self.adjust_score(-2);
        }
        true
    }

    pub fn toggle_pause(&mut self) {
        if !self.won {
            self.paused = !self.paused;
        }
    }

    pub fn total_home(&self) -> usize {
        self.foundations.iter().map(Vec::len).sum()
    }

    pub fn can_draw(&self) -> bool {
        !self.stock.is_empty() || !self.waste.is_empty()
    }

    pub fn can_auto_finish(&self) -> bool {
        !self.won
            && self
                .tableau
                .iter()
                .all(|pile| pile.iter().all(|card| card.face_up))
    }

    pub fn begin_auto_finish(&mut self) -> bool {
        if !self.can_auto_finish() {
            return false;
        }
        if self.find_foundation_move().is_none() && self.stock.is_empty() && self.waste.is_empty() {
            return false;
        }
        let before = self.snapshot();
        self.record(before);
        true
    }

    /// Advance an auto-finish by one foundation move. When no foundation move
    /// is currently available, draw through the stock/waste until one appears.
    /// Returns false only when no progress can be made.
    pub fn auto_finish_one(&mut self) -> bool {
        if let Some((source, index, dest)) = self.find_foundation_move() {
            return self.apply_move(source, index, dest);
        }

        // Drawing is cheap, so search a generous number of draws for a card
        // that can go home. The cap keeps a genuinely stuck position from
        // spinning forever.
        for _ in 0..512 {
            if self.stock.is_empty() && self.waste.is_empty() {
                break;
            }
            if !self.draw_internal() {
                break;
            }
            if let Some((source, index, dest)) = self.find_foundation_move() {
                return self.apply_move(source, index, dest);
            }
        }
        false
    }

    fn auto_move_one_internal(&mut self) -> bool {
        let Some((source, index, dest)) = self.find_safe_foundation_move() else {
            return false;
        };
        self.apply_move(source, index, dest)
    }

    /// Pick a foundation move that does not get more than one rank ahead of
    /// either suit in the opposite color. Aces and twos are always safe.
    fn find_safe_foundation_move(&self) -> Option<(PileRef, usize, PileRef)> {
        let candidates = std::iter::once(PileRef::Waste).chain((0..7).map(PileRef::Tableau));
        for source in candidates {
            let Some(index) = (match source {
                PileRef::Waste => self.waste.len().checked_sub(1),
                PileRef::Tableau(pile) => self.tableau[pile].len().checked_sub(1),
                PileRef::Foundation(_) => None,
            }) else {
                continue;
            };
            let Some(card) = self.cards_at(source, index).and_then(|cards| cards.first()) else {
                continue;
            };
            let Some(dest) = self.foundation_destination(source, index) else {
                continue;
            };
            if card.rank.value() <= 2 || self.opposite_foundations_reached(card) {
                return Some((source, index, dest));
            }
        }
        None
    }

    fn opposite_foundations_reached(&self, card: &Card) -> bool {
        let opposite_suits = match card.suit.color() {
            CardColor::Red => [Suit::Spades, Suit::Clubs],
            CardColor::Black => [Suit::Hearts, Suit::Diamonds],
        };
        let required_rank = card.rank.value().saturating_sub(1) as usize;
        opposite_suits
            .iter()
            .all(|suit| self.foundations[suit.index()].len() >= required_rank)
    }

    pub fn find_foundation_move(&self) -> Option<(PileRef, usize, PileRef)> {
        self.find_safe_foundation_move()
    }

    pub fn find_hint(&self) -> Option<Hint> {
        if self.won {
            return None;
        }

        if let Some(top) = self.waste.len().checked_sub(1)
            && let Some(dest) = self.find_safe_foundation_for(PileRef::Waste, top)
        {
            return Some(Hint::Move {
                source: PileRef::Waste,
                index: top,
                dest,
            });
        }

        for pile in 0..7 {
            if let Some(top) = self.tableau[pile].len().checked_sub(1)
                && let Some(dest) = self.find_safe_foundation_for(PileRef::Tableau(pile), top)
            {
                return Some(Hint::Move {
                    source: PileRef::Tableau(pile),
                    index: top,
                    dest,
                });
            }
        }

        let mut best: Option<(i32, Hint)> = None;

        if let Some(top) = self.waste.len().checked_sub(1) {
            for dest in 0..7 {
                if self.can_move(PileRef::Waste, top, PileRef::Tableau(dest)) {
                    let score = if self.tableau[dest].is_empty() { 2 } else { 7 };
                    if best
                        .as_ref()
                        .is_none_or(|(best_score, _)| score > *best_score)
                    {
                        best = Some((
                            score,
                            Hint::Move {
                                source: PileRef::Waste,
                                index: top,
                                dest: PileRef::Tableau(dest),
                            },
                        ));
                    }
                }
            }
        }

        for source in 0..7 {
            let pile = &self.tableau[source];
            for index in 0..pile.len() {
                if !pile[index].face_up {
                    continue;
                }
                let reveals = index > 0 && !pile[index - 1].face_up;
                for dest in 0..7 {
                    if source == dest
                        || !self.can_move(PileRef::Tableau(source), index, PileRef::Tableau(dest))
                    {
                        continue;
                    }
                    // Lateral shuffles do not make progress and can ping-pong
                    // forever between equivalent columns. Keep tableau hints
                    // focused on moves that reveal a hidden card.
                    if !reveals {
                        continue;
                    }
                    let score = 12;
                    if best
                        .as_ref()
                        .is_none_or(|(best_score, _)| score > *best_score)
                    {
                        best = Some((
                            score,
                            Hint::Move {
                                source: PileRef::Tableau(source),
                                index,
                                dest: PileRef::Tableau(dest),
                            },
                        ));
                    }
                }
            }
        }

        if let Some((_, hint)) = best {
            return Some(hint);
        }

        if !self.stock.is_empty() {
            Some(Hint::Draw)
        } else if !self.waste.is_empty() {
            Some(Hint::Recycle)
        } else {
            None
        }
    }

    fn find_safe_foundation_for(&self, source: PileRef, index: usize) -> Option<PileRef> {
        let card = self.cards_at(source, index)?.first()?;
        let dest = self.foundation_destination(source, index)?;
        (card.rank.value() <= 2 || self.opposite_foundations_reached(card)).then_some(dest)
    }

    pub fn progress(&self) -> f32 {
        self.total_home() as f32 / 52.0
    }
}

impl Snapshot {
    fn is_valid(&self) -> bool {
        let mut ids = HashSet::new();
        let all = self
            .stock
            .iter()
            .chain(self.waste.iter())
            .chain(self.foundations.iter().flatten())
            .chain(self.tableau.iter().flatten());
        let mut total = 0usize;
        for card in all {
            total += 1;
            let canonical_id = card.suit.index() * 13 + card.rank.value() as usize - 1;
            if card.id as usize != canonical_id || !ids.insert(card.id) {
                return false;
            }
        }
        total == 52
            && self.stock.iter().all(|card| !card.face_up)
            && self.waste.iter().all(|card| card.face_up)
            && self.foundations.iter().enumerate().all(|(suit, pile)| {
                pile.iter().enumerate().all(|(rank, card)| {
                    card.face_up
                        && card.suit.index() == suit
                        && card.rank.value() as usize == rank + 1
                })
            })
            && self.tableau.iter().all(|pile| {
                let mut found_face_up = false;
                pile.iter().all(|card| {
                    if card.face_up {
                        found_face_up = true;
                        true
                    } else {
                        !found_face_up
                    }
                })
            })
    }
}

pub fn format_time(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes:02}:{secs:02}")
    }
}

pub fn format_score(score: i64, scoring: Scoring) -> String {
    match scoring {
        Scoring::Standard => score.to_string(),
        Scoring::Vegas => {
            if score < 0 {
                format!("-${}", score.unsigned_abs())
            } else {
                format!("${score}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game::new(42, Rules::default(), false)
    }

    #[test]
    fn deals_every_card_exactly_once() {
        let game = game();
        assert!(game.is_valid());
        assert_eq!(game.stock.len() + game.waste.len(), 24);
        assert_eq!(game.tableau.iter().map(Vec::len).sum::<usize>(), 28);
        assert_eq!(
            game.tableau
                .iter()
                .filter(|p| p.last().unwrap().face_up)
                .count(),
            7
        );
    }

    #[test]
    fn draw_modes_draw_the_right_number_of_cards() {
        let mut one = Game::new(7, Rules::default(), false);
        assert!(one.draw());
        assert_eq!(one.waste.len(), 1);

        let mut three = Game::new(
            7,
            Rules {
                draw_three: true,
                ..Rules::default()
            },
            false,
        );
        assert!(three.draw());
        assert_eq!(three.waste.len(), 3);
    }

    #[test]
    fn recycling_preserves_draw_order() {
        let mut game = Game::new(
            123,
            Rules {
                draw_three: true,
                ..Rules::default()
            },
            false,
        );
        for _ in 0..8 {
            game.draw();
        }
        assert!(game.stock.is_empty());
        let before: Vec<u8> = game.waste.iter().map(|card| card.id).collect();
        assert!(game.draw());
        let after: Vec<u8> = game.waste.iter().map(|card| card.id).collect();
        assert_eq!(&before[..3], &after[..]);
        assert_eq!(game.draw_passes, 1);
    }

    #[test]
    fn foundation_needs_ace_and_matching_suit() {
        let mut game = game();
        let ace = Card::new(0, Suit::Spades, Rank::Ace, true);
        let two = Card::new(1, Suit::Spades, Rank::Two, true);
        game.tableau[0] = vec![two, ace];
        assert!(game.can_move(
            PileRef::Tableau(0),
            1,
            PileRef::Foundation(Suit::Spades.index())
        ));
        assert!(!game.can_move(
            PileRef::Tableau(0),
            0,
            PileRef::Foundation(Suit::Spades.index())
        ));
        assert!(!game.can_move(
            PileRef::Tableau(0),
            1,
            PileRef::Foundation(Suit::Hearts.index())
        ));
    }

    #[test]
    fn tableau_builds_down_in_alternating_colors() {
        let mut game = game();
        game.tableau[0] = vec![Card::new(0, Suit::Hearts, Rank::Eight, true)];
        game.tableau[1] = vec![Card::new(1, Suit::Spades, Rank::Seven, true)];
        game.tableau[2] = vec![Card::new(2, Suit::Diamonds, Rank::Seven, true)];
        assert!(game.can_move(PileRef::Tableau(1), 0, PileRef::Tableau(0)));
        assert!(!game.can_move(PileRef::Tableau(2), 0, PileRef::Tableau(0)));
    }

    #[test]
    fn multi_card_tableau_run_cannot_move_to_a_foundation() {
        let mut game = game();
        game.tableau[0] = vec![
            Card::new(12, Suit::Spades, Rank::King, true),
            Card::new(24, Suit::Hearts, Rank::Queen, true),
        ];
        assert!(!game.can_move(
            PileRef::Tableau(0),
            0,
            PileRef::Foundation(Suit::Spades.index())
        ));
    }

    #[test]
    fn invalid_saved_games_reject_duplicate_or_mismatched_cards() {
        let mut valid_game = game();
        assert!(valid_game.is_valid());

        let duplicate = valid_game.stock[0];
        valid_game.stock[1] = duplicate;
        assert!(!valid_game.is_valid());

        let mut mismatched_rank = game();
        mismatched_rank.stock[0].rank = Rank::King;
        assert!(!mismatched_rank.is_valid());

        let mut invalid_history = game();
        assert!(invalid_history.draw());
        let duplicate = invalid_history.undo[0].stock[0];
        invalid_history.undo[0].stock[1] = duplicate;
        assert!(!invalid_history.is_valid());
    }

    #[test]
    fn automatic_foundation_moves_wait_for_opposite_color_support() {
        let mut game = game();
        game.stock.clear();
        game.waste = vec![Card::new(28, Suit::Diamonds, Rank::Three, true)];
        game.foundations[Suit::Diamonds.index()] = vec![
            Card::new(26, Suit::Diamonds, Rank::Ace, true),
            Card::new(27, Suit::Diamonds, Rank::Two, true),
        ];
        assert!(game.find_foundation_move().is_none());

        for suit in [Suit::Spades, Suit::Clubs] {
            let base = suit.index() as u8 * 13;
            game.foundations[suit.index()] = vec![
                Card::new(base, suit, Rank::Ace, true),
                Card::new(base + 1, suit, Rank::Two, true),
            ];
        }
        assert_eq!(
            game.find_foundation_move(),
            Some((
                PileRef::Waste,
                0,
                PileRef::Foundation(Suit::Diamonds.index())
            ))
        );
    }

    #[test]
    fn hints_do_not_recommend_lateral_ping_pong_moves() {
        let mut game = game();
        game.tableau[0] = vec![Card::new(7, Suit::Spades, Rank::Eight, true)];
        game.tableau[1] = vec![Card::new(19, Suit::Hearts, Rank::Seven, true)];
        let hint = game.find_hint();
        assert!(!matches!(
            hint,
            Some(Hint::Move {
                source: PileRef::Tableau(_),
                dest: PileRef::Tableau(_),
                ..
            })
        ));
    }

    #[test]
    fn undo_and_redo_restore_the_board() {
        let mut game = game();
        let stock = game.stock.len();
        let moves = game.moves;
        assert!(game.draw());
        assert_ne!(game.stock.len(), stock);
        assert!(game.undo());
        assert_eq!(game.stock.len(), stock);
        assert_eq!(game.moves, moves);
        assert!(game.redo());
        assert_ne!(game.stock.len(), stock);
    }

    #[test]
    fn undo_restores_a_card_revealed_by_the_move() {
        let mut game = game();
        game.tableau[0] = vec![
            Card::new(21, Suit::Hearts, Rank::Nine, false),
            Card::new(7, Suit::Spades, Rank::Eight, true),
        ];
        game.tableau[1] = vec![Card::new(19, Suit::Hearts, Rank::Seven, true)];
        assert!(game.try_move(PileRef::Tableau(1), 0, PileRef::Tableau(0), false));
        assert!(game.tableau[0][0].face_up);
        assert!(game.undo());
        assert!(!game.tableau[0][0].face_up);
        assert!(game.tableau[1][0].face_up);
    }

    #[test]
    fn foundation_top_can_be_pulled_back_to_tableau() {
        let mut game = game();
        game.foundations[Suit::Spades.index()] = vec![Card::new(0, Suit::Spades, Rank::Ace, true)];
        game.tableau[0] = vec![Card::new(19, Suit::Hearts, Rank::Seven, true)];
        assert!(game.can_move(
            PileRef::Foundation(Suit::Spades.index()),
            0,
            PileRef::Tableau(0)
        ));
    }

    #[test]
    fn smart_move_prefers_foundation_before_tableau() {
        let mut game = game();
        game.waste = vec![Card::new(0, Suit::Spades, Rank::Ace, true)];
        game.tableau[0] = vec![Card::new(1, Suit::Hearts, Rank::Two, true)];
        assert_eq!(
            game.smart_move_destination(PileRef::Waste, 0),
            Some(PileRef::Foundation(Suit::Spades.index()))
        );
    }

    #[test]
    fn moving_from_foundation_costs_points() {
        let mut game = game();
        game.tableau[0] = vec![Card::new(0, Suit::Hearts, Rank::Three, true)];
        game.foundations[0] = vec![
            Card::new(1, Suit::Spades, Rank::Ace, true),
            Card::new(2, Suit::Spades, Rank::Two, true),
        ];
        game.score = 100;
        assert!(game.try_move(PileRef::Foundation(0), 1, PileRef::Tableau(0), false));
        assert_eq!(game.score, 85);
    }

    #[test]
    fn auto_finish_draws_through_the_stock() {
        let mut game = game();
        game.stock.clear();
        game.waste.clear();
        game.tableau = Default::default();
        game.foundations = Default::default();

        let mut id = 0u8;
        for suit in Suit::ALL {
            for rank in 1..=12 {
                let rank = match rank {
                    1 => Rank::Ace,
                    2 => Rank::Two,
                    3 => Rank::Three,
                    4 => Rank::Four,
                    5 => Rank::Five,
                    6 => Rank::Six,
                    7 => Rank::Seven,
                    8 => Rank::Eight,
                    9 => Rank::Nine,
                    10 => Rank::Ten,
                    11 => Rank::Jack,
                    _ => Rank::Queen,
                };
                game.foundations[suit.index()].push(Card::new(id, suit, rank, true));
                id += 1;
            }
        }

        game.tableau[0] = vec![Card::new(id, Suit::Spades, Rank::King, true)];
        game.tableau[1] = vec![Card::new(id + 1, Suit::Hearts, Rank::King, true)];
        game.tableau[2] = vec![Card::new(id + 2, Suit::Diamonds, Rank::King, true)];
        game.stock = vec![Card::new(id + 3, Suit::Clubs, Rank::King, false)];

        assert!(game.can_auto_finish());
        assert!(game.begin_auto_finish());
        for _ in 0..10 {
            if !game.auto_finish_one() {
                break;
            }
        }
        assert!(game.won);
        assert!(game.foundations.iter().all(|pile| pile.len() == 13));
    }

    #[test]
    fn completing_the_foundations_wins() {
        let mut game = game();
        game.stock.clear();
        game.waste.clear();
        game.tableau = Default::default();
        for suit in Suit::ALL {
            game.foundations[suit.index()] = (1..=13)
                .map(|rank| {
                    let rank = match rank {
                        1 => Rank::Ace,
                        2 => Rank::Two,
                        3 => Rank::Three,
                        4 => Rank::Four,
                        5 => Rank::Five,
                        6 => Rank::Six,
                        7 => Rank::Seven,
                        8 => Rank::Eight,
                        9 => Rank::Nine,
                        10 => Rank::Ten,
                        11 => Rank::Jack,
                        12 => Rank::Queen,
                        _ => Rank::King,
                    };
                    Card::new(0, suit, rank, true)
                })
                .collect();
        }
        game.foundations[0].pop();
        game.tableau[0] = vec![Card::new(51, Suit::Spades, Rank::King, true)];
        assert!(game.try_move(PileRef::Tableau(0), 0, PileRef::Foundation(0), false));
        assert!(game.won);
        assert!(game.score > 0);
    }
}
