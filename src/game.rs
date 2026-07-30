use std::ops::{Index, IndexMut};

macro_rules! cfor {
    ($init: stmt; $cond: expr; $step: expr; $body: block) => {
        {
            $init
            #[allow(while_true)]
            while $cond {
                $body;

                $step;
            }
        }
    }
}

pub(crate) use cfor;

use crate::{rng::XorShiftU64, search::Range};

pub type Hand = (Card, Card);

/// About the derive(Ord):
/// obviously all suits are equal; this is not used for comparing showdown value
#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug)]
pub enum Suit {
    Hearts,
    Diamonds,
    Spades,
    Clubs,
}

impl Suit {
    const NUM: usize = 4;
}

impl<T> Index<Suit> for [T; 4] {
    type Output = T;

    fn index(&self, index: Suit) -> &Self::Output {
        unsafe { self.get_unchecked(index as usize) }
    }
}

impl<T> IndexMut<Suit> for [T; 4] {
    fn index_mut(&mut self, index: Suit) -> &mut Self::Output {
        unsafe { self.get_unchecked_mut(index as usize) }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub enum Rank {
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
    Ace,
}

impl Rank {
    const NUM: usize = 13;
}

impl<T> Index<Rank> for [T; 13] {
    type Output = T;

    fn index(&self, index: Rank) -> &Self::Output {
        unsafe { self.get_unchecked(index as usize) }
    }
}

impl<T> IndexMut<Rank> for [T; 13] {
    fn index_mut(&mut self, index: Rank) -> &mut Self::Output {
        unsafe { self.get_unchecked_mut(index as usize) }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Debug)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl<T> Index<Card> for [T; 52] {
    type Output = T;

    fn index(&self, index: Card) -> &Self::Output {
        let idx = index.rank as usize + Rank::NUM * index.suit as usize;
        unsafe { self.get_unchecked(idx) }
    }
}

impl<T> IndexMut<Card> for [T; 52] {
    fn index_mut(&mut self, index: Card) -> &mut Self::Output {
        let idx = index.rank as usize + Rank::NUM * index.suit as usize;
        unsafe { self.get_unchecked_mut(idx) }
    }
}

impl Card {
    pub const NUM: usize = Suit::NUM * Rank::NUM;

    pub const fn new(rank: Rank, suit: Suit) -> Self {
        Self { rank, suit }
    }
}

pub const CARDS: [Card; Card::NUM] = [
    Card::new(Rank::Two, Suit::Hearts),
    Card::new(Rank::Three, Suit::Hearts),
    Card::new(Rank::Four, Suit::Hearts),
    Card::new(Rank::Five, Suit::Hearts),
    Card::new(Rank::Six, Suit::Hearts),
    Card::new(Rank::Seven, Suit::Hearts),
    Card::new(Rank::Eight, Suit::Hearts),
    Card::new(Rank::Nine, Suit::Hearts),
    Card::new(Rank::Ten, Suit::Hearts),
    Card::new(Rank::Jack, Suit::Hearts),
    Card::new(Rank::Queen, Suit::Hearts),
    Card::new(Rank::King, Suit::Hearts),
    Card::new(Rank::Ace, Suit::Hearts),
    Card::new(Rank::Two, Suit::Diamonds),
    Card::new(Rank::Three, Suit::Diamonds),
    Card::new(Rank::Four, Suit::Diamonds),
    Card::new(Rank::Five, Suit::Diamonds),
    Card::new(Rank::Six, Suit::Diamonds),
    Card::new(Rank::Seven, Suit::Diamonds),
    Card::new(Rank::Eight, Suit::Diamonds),
    Card::new(Rank::Nine, Suit::Diamonds),
    Card::new(Rank::Ten, Suit::Diamonds),
    Card::new(Rank::Jack, Suit::Diamonds),
    Card::new(Rank::Queen, Suit::Diamonds),
    Card::new(Rank::King, Suit::Diamonds),
    Card::new(Rank::Ace, Suit::Diamonds),
    Card::new(Rank::Two, Suit::Spades),
    Card::new(Rank::Three, Suit::Spades),
    Card::new(Rank::Four, Suit::Spades),
    Card::new(Rank::Five, Suit::Spades),
    Card::new(Rank::Six, Suit::Spades),
    Card::new(Rank::Seven, Suit::Spades),
    Card::new(Rank::Eight, Suit::Spades),
    Card::new(Rank::Nine, Suit::Spades),
    Card::new(Rank::Ten, Suit::Spades),
    Card::new(Rank::Jack, Suit::Spades),
    Card::new(Rank::Queen, Suit::Spades),
    Card::new(Rank::King, Suit::Spades),
    Card::new(Rank::Ace, Suit::Spades),
    Card::new(Rank::Two, Suit::Clubs),
    Card::new(Rank::Three, Suit::Clubs),
    Card::new(Rank::Four, Suit::Clubs),
    Card::new(Rank::Five, Suit::Clubs),
    Card::new(Rank::Six, Suit::Clubs),
    Card::new(Rank::Seven, Suit::Clubs),
    Card::new(Rank::Eight, Suit::Clubs),
    Card::new(Rank::Nine, Suit::Clubs),
    Card::new(Rank::Ten, Suit::Clubs),
    Card::new(Rank::Jack, Suit::Clubs),
    Card::new(Rank::Queen, Suit::Clubs),
    Card::new(Rank::King, Suit::Clubs),
    Card::new(Rank::Ace, Suit::Clubs),
];

use std::fmt;

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rank = match self.rank {
            Rank::Two => '2',
            Rank::Three => '3',
            Rank::Four => '4',
            Rank::Five => '5',
            Rank::Six => '6',
            Rank::Seven => '7',
            Rank::Eight => '8',
            Rank::Nine => '9',
            Rank::Ten => 'T',
            Rank::Jack => 'J',
            Rank::Queen => 'Q',
            Rank::King => 'K',
            Rank::Ace => 'A',
        };

        let suit = match self.suit {
            Suit::Hearts => 'h',
            Suit::Diamonds => 'd',
            Suit::Spades => 's',
            Suit::Clubs => 'c',
        };

        write!(f, "{rank}{suit}")
    }
}

impl fmt::Display for CardSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cards = self.get_cards();

        let s = cards.iter().fold("".to_string(), |acc, x| acc + format!("{x}").as_str());

        write!(f, "{s}")
    }
}

impl fmt::Display for ChipState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "pot: {}", self.pot)?;
        writeln!(f, "bb stack: {}", self.bb_stack)?;
        write!(f, "sb stack: {}", self.sb_stack)?;

        Ok(())
    }
}

impl fmt::Display for GameState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", "=".repeat(20))?;
        write!(f, "\nBOARD:\n{}\n\n", self.board)?;
        write!(f, "CHIPSTATE:\n{}\n\n", self.chip_state)?;
        writeln!(f, "HERO HAND: {}{}", self.hero_hand.0, self.hero_hand.1)?;
        write!(f, "{}", "=".repeat(20))?;

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Showdown, // goes to showdown -> resolve with equities
    BBFolded, // BB folded -> SB gets all chips
    SBFolded, // SB folded -> BB gets all chips
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Position {
    SmallBlind,
    BigBlind,
}

impl Position {
    pub fn next(&self) -> Self {
        match self {
            Position::SmallBlind => Position::BigBlind,
            Position::BigBlind => Position::SmallBlind,
        }
    }
}

pub enum Bet {
    SBBet(i32),
    BBBet(i32),
}

impl ChipState {
    pub fn update_with(&mut self, bet: Bet) {
        match bet {
            Bet::SBBet(k) => {
                self.sb_stack -= k;
                self.sb_this_street += k;
                self.pot += k;
                self.max_bet = self.max_bet.max(self.sb_this_street);
            }

            Bet::BBBet(k) => {
                self.bb_stack -= k;
                self.bb_this_street += k;
                self.pot += k;
                self.max_bet = self.max_bet.max(self.bb_this_street);
            }
        }
    }

    pub fn refund_uncalled(&mut self) {
        let sb_bet = self.sb_this_street;
        let bb_bet = self.bb_this_street;

        if sb_bet > bb_bet {
            let excess = sb_bet - bb_bet;

            self.sb_stack += excess;
            self.sb_this_street -= excess;
            self.pot -= excess;
        } else if bb_bet > sb_bet {
            let excess = bb_bet - sb_bet;

            self.bb_stack += excess;
            self.bb_this_street -= excess;
            self.pot -= excess;
        }
    }
}

/// Cardset != 5-card hand.
/// This is just the union of our hole cards and whatever community cards there are.
/// To assess the strength of the hand, we only need suit masks and rank counts.
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct CardSet {
    /// Bitset of cards present of each suit.
    /// First 3 bits unused; more significant bit = more significant card.
    pub suit_masks: [u16; Suit::NUM],
    pub rank_counts: [u8; Rank::NUM],
}

const ACE_HIGH_STRAIGHT: u16 = 0b_0001_1111_0000_0000;
const WHEEL_MASK: u16 = 0b0001_0000_0000_1111;

impl CardSet {
    const SF_SCORE: i32 = 2_000_000_000;
    const QUADS_SCORE: i32 = 1_800_000_000;
    const FH_SCORE: i32 = 1_600_000_000;
    const FLUSH_SCORE: i32 = 1_400_000_000;
    const STRAIGHT_SCORE: i32 = 1_200_000_000;
    const THREE_OF_KIND_SCORE: i32 = 1_000_000_000;
    const TWO_PAIR_SCORE: i32 = 800_000_000;
    const PAIR_SCORE: i32 = 600_000_000;

    const FIRST_SCORE: i32 = 10_000_000;
    const SECOND_SCORE: i32 = 100_000;

    const fn straight_masks() -> [u16; 10] {
        let mut res = [0; 10];

        let mut m = ACE_HIGH_STRAIGHT;

        let mut idx = 0;

        while m.count_ones() >= 5 {
            res[idx] = m;
            m >>= 1;
            idx += 1;
        }

        res[9] = WHEEL_MASK;

        res
    }

    pub const BLANK: Self = Self { suit_masks: [0; 4], rank_counts: [0; 13] };

    const STRAIGHT_MASKS: [u16; 10] = Self::straight_masks();

    const fn top_n_ranks(mut mask: u16, n: u32) -> u16 {
        while mask.count_ones() > n {
            mask &= mask - 1;
        }

        mask
    }

    const fn straight_high(mask: u16) -> Option<i32> {
        cfor!(let mut idx = 0; idx < 10; idx += 1;{
            let s = Self::STRAIGHT_MASKS[idx];

            if mask & s == s {
                return Some(10 - idx as i32);
            }
        });

        None
    }

    pub fn get_cards(&self) -> Vec<Card> {
        let m = (self.suit_masks[3] as u64) << 39
            | (self.suit_masks[2] as u64) << 26
            | (self.suit_masks[1] as u64) << 13
            | self.suit_masks[0] as u64;

        let mut res = vec![];

        for c in CARDS {
            if m & CARD_MASKS[c] > 0 {
                res.push(c);
            }
        }

        res
    }

    const MASKS_TOTAL: usize = 1 << 13; // 8192

    #[allow(clippy::type_complexity)]
    const TABLES: (
        [Option<i32>; Self::MASKS_TOTAL],
        [u16; Self::MASKS_TOTAL],
        [u16; Self::MASKS_TOTAL],
        [u16; Self::MASKS_TOTAL],
        [u16; Self::MASKS_TOTAL],
    ) = {
        let mut straight_high = [None; Self::MASKS_TOTAL];
        let mut top_5 = [0; Self::MASKS_TOTAL];
        let mut top_3 = [0; Self::MASKS_TOTAL];
        let mut top_2 = [0; Self::MASKS_TOTAL];
        let mut top_1 = [0; Self::MASKS_TOTAL];

        let mut mask = 0;

        while mask < Self::MASKS_TOTAL {
            let m = mask as u16;

            straight_high[mask] = Self::straight_high(m);
            top_5[mask] = Self::top_n_ranks(m, 5);
            top_3[mask] = Self::top_n_ranks(m, 3);
            top_2[mask] = Self::top_n_ranks(m, 2);
            top_1[mask] = Self::top_n_ranks(m, 1);

            mask += 1;
        }

        (straight_high, top_5, top_3, top_2, top_1)
    };

    const STRAIGHT_HIGH: [Option<i32>; Self::MASKS_TOTAL] = Self::TABLES.0;
    const TOP_5: [u16; Self::MASKS_TOTAL] = Self::TABLES.1;
    const TOP_3: [u16; Self::MASKS_TOTAL] = Self::TABLES.2;
    const TOP_2: [u16; Self::MASKS_TOTAL] = Self::TABLES.3;
    const TOP: [u16; Self::MASKS_TOTAL] = Self::TABLES.4;

    pub fn score(&self) -> i32 {
        let mut flush_score = None;

        let all = self.suit_masks[Suit::Hearts]
            | self.suit_masks[Suit::Diamonds]
            | self.suit_masks[Suit::Spades]
            | self.suit_masks[Suit::Clubs];

        for u in self.suit_masks {
            if u.count_ones() >= 5 {
                let flush_mask = Self::TOP_5[u as usize];
                flush_score = Some(Self::FLUSH_SCORE + flush_mask as i32);

                if let Some(s) = Self::STRAIGHT_HIGH[u as usize] {
                    return Self::SF_SCORE + s;
                }
            }
        }

        let (mut mx, mut mx_idx, mut smx, mut smx_idx) = (0, 0, 0, 0);

        for rank_idx in 0..Rank::NUM {
            if self.rank_counts[rank_idx] >= mx {
                smx = mx;
                smx_idx = mx_idx;

                mx = self.rank_counts[rank_idx];
                mx_idx = rank_idx;
            } else if self.rank_counts[rank_idx] >= smx {
                smx = self.rank_counts[rank_idx];
                smx_idx = rank_idx;
            }
        }

        if mx == 4 {
            // quads
            let rest = all & !(1 << mx_idx);
            let kicker = Self::TOP[rest as usize];

            Self::QUADS_SCORE + Self::FIRST_SCORE * mx_idx as i32 + kicker as i32
        } else if mx == 3 && smx >= 2 {
            // full house
            Self::FH_SCORE + Self::FIRST_SCORE * (mx_idx as i32) + Self::SECOND_SCORE * (smx_idx as i32)
        } else if let Some(s) = flush_score {
            // flush
            s
        } else if let Some(s) = Self::STRAIGHT_HIGH[all as usize] {
            // straight
            Self::STRAIGHT_SCORE + s
        } else if mx == 3 {
            // three of a kind
            let rest = all & !(1 << mx_idx);
            let kickers = Self::TOP_2[rest as usize];
            Self::THREE_OF_KIND_SCORE + Self::FIRST_SCORE * (mx_idx as i32) + kickers as i32
        } else if smx == 2 {
            // two pair
            let rest = all & !(1 << mx_idx | 1 << smx_idx);
            let kicker = Self::TOP[rest as usize];
            Self::TWO_PAIR_SCORE
                + Self::FIRST_SCORE * (mx_idx as i32)
                + Self::SECOND_SCORE * (smx_idx as i32)
                + kicker as i32
        } else if mx == 2 {
            // one pair
            let rest = all & !(1 << mx_idx);
            let kickers = Self::TOP_3[rest as usize];
            Self::PAIR_SCORE + Self::FIRST_SCORE * (mx_idx as i32) + kickers as i32
        } else {
            // high card
            let kickers = Self::TOP_5[all as usize];
            kickers as i32
        }
    }

    const RANK_MASKS: [u16; 13] = [
        0b_0000_0000_0000_0001,
        0b_0000_0000_0000_0010,
        0b_0000_0000_0000_0100,
        0b_0000_0000_0000_1000,
        0b_0000_0000_0001_0000,
        0b_0000_0000_0010_0000,
        0b_0000_0000_0100_0000,
        0b_0000_0000_1000_0000,
        0b_0000_0001_0000_0000,
        0b_0000_0010_0000_0000,
        0b_0000_0100_0000_0000,
        0b_0000_1000_0000_0000,
        0b_0001_0000_0000_0000,
    ];

    pub fn update_with(&mut self, c: Card) {
        self.suit_masks[c.suit] |= Self::RANK_MASKS[c.rank];
        self.rank_counts[c.rank] += 1;
    }
}

impl Ord for CardSet {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score().cmp(&other.score())
    }
}

impl PartialOrd for CardSet {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ChipState {
    pub pot: i32,
    pub sb_stack: i32,
    pub bb_stack: i32,
    pub sb_this_street: i32,
    pub bb_this_street: i32,
    pub max_bet: i32,
}

/// Note about hero_hand:
/// This isn't necessarily the cards we have - often we will have to set this to a particular hand
/// to figure out ranges.
#[derive(Clone, Copy)]
pub struct GameState {
    pub chip_state: ChipState,
    pub turn: Position,
    pub board: CardSet,
    pub hero_hand: Hand,
    pub sb_range: Range,
    pub bb_range: Range,
    pub seen: u64,
    pub board_len: u8,
}

pub const CARD_MASKS: [u64; Card::NUM] = {
    const fn card_masks() -> [u64; Card::NUM] {
        let mut res = [0; 52];

        cfor!(let mut i = 0;i < 52;i += 1; {
            res[i] = 1 << i;
        });

        res
    }

    card_masks()
};

impl GameState {
    pub fn set_hero_hand(&mut self, hand: Hand) {
        self.seen &= !CARD_MASKS[self.hero_hand.0];
        self.seen &= !CARD_MASKS[self.hero_hand.1];

        self.hero_hand = hand;

        self.seen |= CARD_MASKS[hand.0];
        self.seen |= CARD_MASKS[hand.1];
    }

    pub fn remove_hero_hand(&mut self) {
        self.seen &= !CARD_MASKS[self.hero_hand.0];
        self.seen &= !CARD_MASKS[self.hero_hand.1];
    }

    pub fn gen_runout(&self) -> GameState {
        let mut res = *self;
        let mut rng = XorShiftU64::new();

        while res.board_len < 5 {
            let next_card = rng.next_card(&mut res.seen);
            res.board.update_with(next_card);
            res.board_len += 1;
        }

        res
    }

    pub fn streets_remaining(&self) -> u8 {
        match self.board_len {
            0 => 4,
            3 => 3,
            4 => 2,
            5 => 1,
            _ => unreachable!(),
        }
    }

    pub fn add_card(&mut self, c: Card) {
        self.board.update_with(c);
        self.seen |= CARD_MASKS[c];
        self.board_len += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::play::Parseable;

    use super::*;

    macro_rules! hand {
        ($name:ident, $($card:expr),+ $(,)?) => {
            let mut $name = CardSet::BLANK;
            $(
                $name.update_with(Card::parse($card));
            )+
        };
    }

    #[test]
    fn test_hand_ranks() {
        hand!(royal, "As", "Ks", "Qs", "Js", "Ts");
        hand!(steel_wheel, "As", "2s", "3s", "4s", "5s");

        hand!(quad_aces, "As", "Ah", "Ad", "Ac", "Ts");
        hand!(worse_quad_aces, "As", "Ah", "Ad", "Ac", "3s");
        hand!(quad_deuces, "2s", "2h", "2d", "2c", "Ts");

        hand!(aces_full_of_deuces, "As", "Ah", "Ad", "2c", "2s");
        hand!(also_aces_full_of_deuces, "2c", "2s", "2d", "As", "Ad", "Ah");
        hand!(kings_full_of_queens, "Ks", "Kh", "Kd", "Qc", "Qs");

        hand!(ace_high_flush, "As", "2s", "Qs", "Js", "Ts");
        hand!(king_high_flush, "Ks", "2s", "Qs", "Js", "Ts");

        hand!(ace_high_straight, "As", "Kc", "Qs", "Js", "Ts");
        hand!(king_high_straight, "9s", "Kc", "Qs", "Js", "Ts");

        hand!(three_aces, "As", "Ah", "Ad", "Jc", "Ts");
        hand!(three_deuces, "2s", "2h", "2d", "5c", "Ts");

        hand!(aces_and_deuces, "As", "Ah", "2s", "2h", "Ks");
        hand!(kings_and_queens, "Ks", "Kh", "Qs", "Qh", "9s");
        hand!(three_pair, "Ks", "Kh", "Qs", "Qh", "9s", "9h");

        hand!(pair_aces, "As", "Ah", "Ks", "Qs", "Js");
        hand!(worse_pair_aces, "As", "Ah", "Qs", "Js", "Ts");
        hand!(pair_kings, "Ks", "Kh", "As", "Qs", "Js");

        hand!(ace_high, "As", "Ks", "Qs", "Js", "9d");
        hand!(different_suits, "Ad", "Kd", "Qd", "Jd", "9s");

        assert!(royal > steel_wheel);

        assert!(steel_wheel > quad_aces);
        assert!(quad_aces > worse_quad_aces);
        assert!(worse_quad_aces > quad_deuces);

        assert!(quad_deuces > aces_full_of_deuces);
        assert!(aces_full_of_deuces > kings_full_of_queens);
        assert_eq!(aces_full_of_deuces.cmp(&also_aces_full_of_deuces), Ordering::Equal);

        assert!(kings_full_of_queens > ace_high_flush);
        assert!(ace_high_flush > king_high_flush);

        assert!(king_high_flush > ace_high_straight);
        assert!(ace_high_straight > king_high_straight);

        assert!(king_high_straight > three_aces);
        assert!(three_aces > three_deuces);

        assert!(three_deuces > aces_and_deuces);
        assert!(aces_and_deuces > kings_and_queens);
        assert_eq!(kings_and_queens.cmp(&three_pair), Ordering::Equal);

        assert!(kings_and_queens > pair_aces);
        assert!(pair_aces > worse_pair_aces);
        assert!(worse_pair_aces > pair_kings);

        assert!(pair_kings > ace_high);
        assert_eq!(ace_high.cmp(&different_suits), Ordering::Equal);
    }

    #[test]
    fn test_sevens() {
        hand!(royal_5, "As", "Ks", "Qs", "Js", "Ts");
        hand!(royal_7, "As", "Ks", "Qs", "Js", "Ts", "2h", "3d",);

        hand!(quad_aces_5, "As", "Ah", "Ad", "Ac", "Ts");
        hand!(quad_aces_7, "As", "Ah", "Ad", "Ac", "Ts", "2h", "3d",);

        hand!(ace_high_flush_5, "As", "Qs", "Js", "Ts", "2s");
        hand!(ace_high_flush_7, "As", "Qs", "Js", "Ts", "2s", "3h", "7d",);

        hand!(three_aces_5, "As", "Ah", "Ad", "Jc", "Ts");
        hand!(three_aces_7, "As", "Ah", "Ad", "Jc", "Ts", "2h", "3d",);

        hand!(aces_and_deuces_5, "As", "Ah", "2s", "2h", "Ks");
        hand!(aces_and_deuces_7, "As", "Ah", "2s", "2h", "Ks", "3c", "4d",);

        hand!(pair_aces_5, "As", "Ah", "Ks", "Qs", "Js");
        hand!(pair_aces_7, "As", "Ah", "Ks", "Qs", "Js", "2c", "3d",);

        assert_eq!(royal_5.cmp(&royal_7), Ordering::Equal);
        assert_eq!(quad_aces_5.cmp(&quad_aces_7), Ordering::Equal);
        assert_eq!(ace_high_flush_5.cmp(&ace_high_flush_7), Ordering::Equal);
        assert_eq!(three_aces_5.cmp(&three_aces_7), Ordering::Equal);
        assert_eq!(aces_and_deuces_5.cmp(&aces_and_deuces_7), Ordering::Equal);
        assert_eq!(pair_aces_5.cmp(&pair_aces_7), Ordering::Equal);
    }
}
