pub mod game;
pub mod hash;
pub mod play;
pub mod rng;
pub mod search;

use crate::game::*;
use crate::play::*;
use crate::search::*;

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
    let chip_state =
        ChipState { pot: 10, sb_stack: 95, bb_stack: 95, sb_this_street: 0, bb_this_street: 0, max_bet: 5 };

    let hand = hands::DD;

    let mut game_state = GameState {
        chip_state,
        board: CardSet::BLANK,
        turn: Position::SmallBlind,
        hero_hand: hand,
        sb_range: Range::BLANK,
        bb_range: Range::BLANK,
        seen: 0,
        board_len: 0,
    };

    game_state.set_hero_hand(hand);

    let flop =
        [Card::new(Rank::Two, Suit::Clubs), Card::new(Rank::Ace, Suit::Clubs), Card::new(Rank::Five, Suit::Hearts)];

    for card in flop {
        game_state.board.update_with(card);
        game_state.seen |= CARD_MASKS[card];
        game_state.board_len += 1;
    }

    play_from(&mut game_state);
}
