pub mod game;
pub mod rng;
pub mod search;

use crate::game::*;
use crate::search::*;

// need to take into account strength of action inside game tree
// otherwise shoves WAY too much
// maybe:
// - calculate their calling range with pot odds
// - how to calculate our raising range?
// - maybe precalculate it

fn main() {
    let mut cs = ChipState { pot: 3, sb_stack: 99, bb_stack: 98, sb_this_street: 1, bb_this_street: 2, max_bet: 2 };

    let first_card = Card::new(Rank::Ace, Suit::Diamonds);
    let second_card = Card::new(Rank::Ace, Suit::Spades);

    let hand = (first_card, second_card);

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

    match actions {
        Actions::Even(a) => {
            dbg!(a.probs);
        }
        Actions::Behind(a) => {
            dbg!(a.probs);
        }
    }

    println!("BEFORE:");

    let aa = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::Ace, Suit::Hearts));
    let aks = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Spades));
    let ako = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Hearts));
    let sdo = (Card::new(Rank::Seven, Suit::Spades), Card::new(Rank::Two, Suit::Hearts));
    let t9s = (Card::new(Rank::Ten, Suit::Spades), Card::new(Rank::Nine, Suit::Spades));

    println!("AA:  {}", gs.sb_range.probs[aa.0][aa.1] + gs.sb_range.probs[aa.1][aa.0]);
    println!("AKs: {}", gs.sb_range.probs[aks.0][aks.1] + gs.sb_range.probs[aks.1][aks.0]);
    println!("AKo: {}", gs.sb_range.probs[ako.0][ako.1] + gs.sb_range.probs[ako.1][ako.0]);
    println!("72o: {}", gs.sb_range.probs[sdo.0][sdo.1] + gs.sb_range.probs[sdo.1][sdo.0]);
    println!("T9s: {}", gs.sb_range.probs[t9s.0][t9s.1] + gs.sb_range.probs[t9s.1][t9s.0]);

    gs.update_ranges_with_decision(3);

    println!("AFTER:");

    println!("AA:  {}", gs.sb_range.probs[aa.0][aa.1] + gs.sb_range.probs[aa.1][aa.0]);
    println!("AKs: {}", gs.sb_range.probs[aks.0][aks.1] + gs.sb_range.probs[aks.1][aks.0]);
    println!("AKo: {}", gs.sb_range.probs[ako.0][ako.1] + gs.sb_range.probs[ako.1][ako.0]);
    println!("72o: {}", gs.sb_range.probs[sdo.0][sdo.1] + gs.sb_range.probs[sdo.1][sdo.0]);
    println!("T9s: {}", gs.sb_range.probs[t9s.0][t9s.1] + gs.sb_range.probs[t9s.1][t9s.0]);

    cs = ChipState { pot: 102, sb_stack: 0, bb_stack: 98, sb_this_street: 100, bb_this_street: 2, max_bet: 100 };

    gs.chip_state = cs;
    gs.turn = Position::BigBlind;

    gs.set_hero_hand(aa);

    let n = gs.do_runouts();

    let Some(actions) = n.actions else { unreachable!() };

    match actions {
        Actions::Even(a) => {
            dbg!(a.probs);
        }
        Actions::Behind(a) => {
            dbg!(a.probs);
        }
    }
}
