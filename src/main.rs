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
    let cs = ChipState { pot: 4, sb_stack: 98, bb_stack: 98, sb_this_street: 2, bb_this_street: 2, max_bet: 2 };

    let first_card = Card::new(Rank::Ace, Suit::Diamonds);
    let second_card = Card::new(Rank::Ace, Suit::Spades);

    let mut seen_mask = 0;
    seen_mask |= CARD_MASKS[first_card];
    seen_mask |= CARD_MASKS[second_card];

    let hand = (first_card, second_card);

    let mut gs = GameState {
        chip_state: cs,
        board: CardSet::BLANK,
        turn: Position::BigBlind,
        hero_hand: hand,
        sb_range: Range::BLANK,
        bb_range: Range::BLANK,
        seen: seen_mask,
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
    let seven_two_off = (Card::new(Rank::Seven, Suit::Spades), Card::new(Rank::Two, Suit::Hearts));
    let t9s = (Card::new(Rank::Ten, Suit::Spades), Card::new(Rank::Nine, Suit::Spades));

    println!("AA:  {}", gs.bb_range.probs[aa.0][aa.1] + gs.bb_range.probs[aa.1][aa.0]);
    println!("AKs: {}", gs.bb_range.probs[aks.0][aks.1] + gs.bb_range.probs[aks.1][aks.0]);
    println!("AKo: {}", gs.bb_range.probs[ako.0][ako.1] + gs.bb_range.probs[ako.1][ako.0]);
    println!(
        "72o: {}",
        gs.bb_range.probs[seven_two_off.0][seven_two_off.1] + gs.bb_range.probs[seven_two_off.1][seven_two_off.0]
    );
    println!("T9s: {}", gs.bb_range.probs[t9s.0][t9s.1] + gs.bb_range.probs[t9s.1][t9s.0]);

    gs.update_ranges_with_decision(0);

    println!("AFTER:");

    let aa = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::Ace, Suit::Hearts));
    let aks = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Spades));
    let ako = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Hearts));
    let seven_two_off = (Card::new(Rank::Seven, Suit::Spades), Card::new(Rank::Two, Suit::Hearts));
    let t9s = (Card::new(Rank::Ten, Suit::Spades), Card::new(Rank::Nine, Suit::Spades));

    println!("AA:  {}", gs.bb_range.probs[aa.0][aa.1] + gs.bb_range.probs[aa.1][aa.0]);
    println!("AKs: {}", gs.bb_range.probs[aks.0][aks.1] + gs.bb_range.probs[aks.1][aks.0]);
    println!("AKo: {}", gs.bb_range.probs[ako.0][ako.1] + gs.bb_range.probs[ako.1][ako.0]);
    println!(
        "72o: {}",
        gs.bb_range.probs[seven_two_off.0][seven_two_off.1] + gs.bb_range.probs[seven_two_off.1][seven_two_off.0]
    );
    println!("T9s: {}", gs.bb_range.probs[t9s.0][t9s.1] + gs.bb_range.probs[t9s.1][t9s.0]);
}
