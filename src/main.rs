pub mod game;
pub mod rng;
pub mod search;

use crate::game::*;
use crate::search::*;

#[allow(unused)]
fn main() {
    let chip_state = ChipState { pot: 3, sb_stack: 99, bb_stack: 98, sb_this_street: 1, bb_this_street: 2, max_bet: 2 };

    let aa = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::Ace, Suit::Hearts));
    let aks = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Spades));
    let ako = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Hearts));
    let tt = (Card::new(Rank::Ten, Suit::Spades), Card::new(Rank::Ten, Suit::Hearts));
    let dd = (Card::new(Rank::Two, Suit::Spades), Card::new(Rank::Two, Suit::Hearts));
    let sdo = (Card::new(Rank::Seven, Suit::Spades), Card::new(Rank::Two, Suit::Hearts));
    let t9s = (Card::new(Rank::Ten, Suit::Spades), Card::new(Rank::Nine, Suit::Spades));

    let hand = aa;
    dbg!(hand.0, hand.1);

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

    let root = game_state.do_runouts();

    match root.actions.as_ref().unwrap() {
        Actions::Even(actions) => {
            println!("\nROOT ACTIONS:");
            dbg!(actions.probs);
            dbg!(actions.visits);
            dbg!(actions.total_ev);
            dbg!(actions.legal);
        }

        Actions::Behind(actions) => {
            println!("\nROOT ACTIONS:");
            dbg!(actions.probs);
            dbg!(actions.visits);
            dbg!(actions.total_ev);
            dbg!(actions.legal);
        }
    }
}
