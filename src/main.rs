pub mod game;
pub mod rng;
pub mod search;

use crate::game::*;
use crate::search::*;

fn main() {
    let cs = ChipState {
        pot: 3,
        sb_stack: 99,
        bb_stack: 98,
        sb_this_street: 1,
        bb_this_street: 2,
        max_bet: 2,
    };

    let first_card = Card::new(Rank::Nine, Suit::Diamonds);
    let second_card = Card::new(Rank::Ten, Suit::Diamonds);

    let mut seen_mask = 0;
    seen_mask |= CARD_MASKS[first_card];
    seen_mask |= CARD_MASKS[second_card];

    let gs = GameState {
        chip_state: cs,
        board: CardSet::BLANK,
        turn: Position::SmallBlind,
        hero_hand: (first_card, second_card),
        sb_range: Range::BLANK,
        bb_range: Range::BLANK,
        seen: seen_mask,
    };

    let n = gs.do_runouts();

    let Some(actions) = n.actions else {
        unreachable!()
    };

    match actions {
        Actions::Even(a) => {
            dbg!(a.probs);
        }
        Actions::Behind(a) => {
            dbg!(a.probs);
        }
    }
}
