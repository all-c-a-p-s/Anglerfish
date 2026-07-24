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

use crate::{
    rng::XorShiftU64,
    search::{Position, Range},
};

pub type Hand = (Card, Card);

/// About the derive(Ord):
/// obviously all suits are equal; this is not used for comparing showdown value
#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
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

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
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

#[derive(PartialEq, Eq, Clone, Copy, PartialOrd, Ord)]
pub struct Card {
    rank: Rank,
    suit: Suit,
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
    const QUADS_SCORE: i32 = 1800_000_000;
    const FH_SCORE: i32 = 1600_000_000;
    const FLUSH_SCORE: i32 = 1400_000_000;
    const STRAIGHT_SCORE: i32 = 1200_000_000;
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

    const MASKS_TOTAL: usize = 1 << 13; // 8192

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
        0b_0001_1000_0000_0000,
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
}
