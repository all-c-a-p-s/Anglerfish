pub mod game;
pub mod rng;
pub mod search;

use crate::game::*;
use crate::search::*;

#[allow(unused)]
fn main() {
    let cs = ChipState { pot: 3, sb_stack: 99, bb_stack: 98, sb_this_street: 1, bb_this_street: 2, max_bet: 2 };

    let aa = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::Ace, Suit::Hearts));
    let aks = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Spades));
    let ako = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Hearts));
    let tt = (Card::new(Rank::Ten, Suit::Spades), Card::new(Rank::Ten, Suit::Hearts));
    let dd = (Card::new(Rank::Two, Suit::Spades), Card::new(Rank::Two, Suit::Hearts));
    let sdo = (Card::new(Rank::Seven, Suit::Spades), Card::new(Rank::Two, Suit::Hearts));
    let t9s = (Card::new(Rank::Ten, Suit::Spades), Card::new(Rank::Nine, Suit::Spades));

    let hand = tt;
    dbg!(hand.0, hand.1);

    let mut gs = GameState {
        chip_state: cs,
        board: CardSet::BLANK,
        turn: Position::SmallBlind,
        hero_hand: hand,
        sb_range: Range::BLANK,
        bb_range: Range::BLANK,
        seen: 0,
        board_len: 0,
    };

    gs.set_hero_hand(hand);

    let n = gs.do_runouts();

    let Some(actions) = n.actions else { unreachable!() };

    match &actions {
        Actions::Even(root_actions) => {
            dbg!(root_actions.probs);
            dbg!(root_actions.visits);
            dbg!(root_actions.total_ev);

            let after_jam = root_actions.children.as_ref().unwrap()[4].as_ref();

            let Actions::Behind(response_actions) = after_jam.actions.as_ref().unwrap() else {
                unreachable!();
            };

            dbg!(response_actions.probs);
            dbg!(response_actions.total_ev);
            dbg!(response_actions.visits);

            let fold_probability = response_actions.probs[0];
            let call_probability = response_actions.probs[1];

            dbg!(fold_probability);
            dbg!(call_probability);
        }

        Actions::Behind(root_actions) => {
            dbg!(root_actions.probs);
            dbg!(root_actions.visits);
            dbg!(root_actions.total_ev);

            let after_jam = root_actions.children.as_ref().unwrap()[3].as_ref();

            let Actions::Behind(response_actions) = after_jam.actions.as_ref().unwrap() else {
                unreachable!();
            };

            dbg!(response_actions.probs);
            dbg!(response_actions.total_ev);
            dbg!(response_actions.visits);

            let fold_probability = response_actions.probs[0];
            let call_probability = response_actions.probs[1];

            dbg!(fold_probability);
            dbg!(call_probability);
        }
    }
}
