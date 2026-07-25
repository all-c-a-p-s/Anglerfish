pub mod game;
pub mod hash;
pub mod play;
pub mod rng;
pub mod search;

use crate::game::*;
use crate::play::*;

#[allow(unused)]
mod hands {
    use super::*;

    pub const AA: Hand = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::Ace, Suit::Hearts));
    pub const AKS: Hand = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Spades));
    pub const AKO: Hand = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Hearts));
    pub const DD: Hand = (Card::new(Rank::Two, Suit::Spades), Card::new(Rank::Two, Suit::Hearts));
    pub const TT: Hand = (Card::new(Rank::Ten, Suit::Spades), Card::new(Rank::Ten, Suit::Hearts));
    pub const SDO: Hand = (Card::new(Rank::Seven, Suit::Spades), Card::new(Rank::Two, Suit::Hearts));
    pub const T9S: Hand = (Card::new(Rank::Ten, Suit::Spades), Card::new(Rank::Nine, Suit::Spades));
}

fn main() {
    hand_loop();
}
