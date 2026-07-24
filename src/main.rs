pub mod game;
pub mod rng;
pub mod search;

use crate::game::*;
use crate::search::*;

fn inspect_range(name: &str, range: &Range, hands: &[(&str, Hand)]) {
    println!("\n{name}:");

    for &(label, hand) in hands {
        let (c1, c2) = if hand.0 > hand.1 { (hand.0, hand.1) } else { (hand.1, hand.0) };

        println!("{label:>3}: {:.10}", range.probs[c1][c2]);
    }
}

fn inspect_range_detailed(
    name: &str,
    range: &Range,
    interesting_hands: &[(&str, Hand)],
    top_n: usize,
    equity_vs_random: f64,
) {
    let uniform_probability = 1.0 / 1326.0;

    let mut combinations = Vec::new();
    let mut total_probability = 0.0;
    let mut entropy = 0.0;
    let mut nonzero = 0usize;

    for c1 in CARDS {
        for c2 in CARDS {
            if c1 <= c2 {
                continue;
            }

            let probability = range.probs[c1][c2];

            if probability > 0.0 {
                nonzero += 1;
                total_probability += probability;
                entropy -= probability * probability.ln();

                combinations.push(((c1, c2), probability));
            }
        }
    }

    combinations.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let effective_combinations = entropy.exp();

    let mass_in_top = |n: usize| -> f64 { combinations.iter().take(n).map(|(_, probability)| probability).sum() };

    println!("\n========== {name} ==========");
    println!("Total probability:       {total_probability:.10}");
    println!("Nonzero combinations:    {nonzero}");
    println!("Entropy:                  {entropy:.6}");
    println!("Effective combinations:  {effective_combinations:.2}");
    println!("Equity vs random hand:    {:.2}%", 100.0 * equity_vs_random,);
    println!("Top 10 mass:              {:.2}%", 100.0 * mass_in_top(10));
    println!("Top 50 mass:              {:.2}%", 100.0 * mass_in_top(50));
    println!("Top 100 mass:             {:.2}%", 100.0 * mass_in_top(100));

    println!("\nSelected hands:");

    for &(label, hand) in interesting_hands {
        let (c1, c2) = if hand.0 > hand.1 { (hand.0, hand.1) } else { (hand.1, hand.0) };

        let probability = range.probs[c1][c2];

        println!("{label:>4}: {probability:.10} ({:>6.3}x uniform)", probability / uniform_probability,);
    }

    println!("\nTop {} combinations:", top_n.min(combinations.len()));

    for (rank, &((c1, c2), probability)) in combinations.iter().take(top_n).enumerate() {
        println!(
            "{:>3}. {:?} {:?}: {:.10} ({:.3}x uniform)",
            rank + 1,
            c1,
            c2,
            probability,
            probability / uniform_probability,
        );
    }
}

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

    let interesting_hands =
        [("AA", aa), ("AKs", aks), ("AKo", ako), ("TT", tt), ("22", dd), ("72o", sdo), ("T9s", t9s)];

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

    let Some(actions) = n.actions else {
        unreachable!();
    };

    match &actions {
        Actions::Even(root_actions) => {
            println!("\nROOT ACTIONS:");
            dbg!(root_actions.probs);
            dbg!(root_actions.visits);
            dbg!(root_actions.total_ev);

            // Even actions:
            // 0 = check
            // 1 = quarter-pot bet
            // 2 = half-pot bet
            // 3 = pot-sized bet
            // 4 = all-in
            let after_jam = root_actions.children.as_ref().unwrap()[4].as_ref();

            inspect_range("SB range after jamming", after_jam.sb_range.as_ref(), &interesting_hands);

            let jam_equity = gs.range_equity_vs_random(*after_jam.sb_range, 1_000);

            inspect_range_detailed("SB jamming range", after_jam.sb_range.as_ref(), &interesting_hands, 10, jam_equity);

            let Actions::Behind(response_actions) = after_jam.actions.as_ref().unwrap() else {
                unreachable!("BB should be facing the SB jam");
            };

            println!("\nBB RESPONSE TO JAM:");
            dbg!(response_actions.probs);
            dbg!(response_actions.legal);
            dbg!(response_actions.total_ev);
            dbg!(response_actions.visits);

            let fold_probability = response_actions.probs[0];
            let call_probability = response_actions.probs[1];

            dbg!(fold_probability);
            dbg!(call_probability);

            // Behind actions:
            // 0 = fold
            // 1 = call
            // 2 = 2.5x raise
            // 3 = all-in
            let after_call = response_actions.children.as_ref().unwrap()[1].as_ref();

            inspect_range("BB range after calling the jam", after_call.bb_range.as_ref(), &interesting_hands);

            let call_equity = gs.range_equity_vs_random(*after_call.bb_range, 1_000);

            inspect_range_detailed(
                "BB call-after-jam range",
                after_call.bb_range.as_ref(),
                &interesting_hands,
                10,
                call_equity,
            );
        }

        Actions::Behind(root_actions) => {
            println!("\nROOT ACTIONS:");
            dbg!(root_actions.probs);
            dbg!(root_actions.visits);
            dbg!(root_actions.total_ev);

            // Behind actions:
            // 0 = fold
            // 1 = call
            // 2 = 2.5x raise
            // 3 = all-in
            let after_jam = root_actions.children.as_ref().unwrap()[3].as_ref();

            inspect_range("SB range after jamming", after_jam.sb_range.as_ref(), &interesting_hands);

            let jam_equity = gs.range_equity_vs_random(*after_jam.sb_range, 1_000);

            inspect_range_detailed("SB jamming range", after_jam.sb_range.as_ref(), &interesting_hands, 10, jam_equity);

            let Actions::Behind(response_actions) = after_jam.actions.as_ref().unwrap() else {
                unreachable!("BB should be facing the SB jam");
            };

            let after_call = response_actions.children.as_ref().unwrap()[1].as_ref();

            inspect_range("BB range after calling the jam", after_call.bb_range.as_ref(), &interesting_hands);
            let call_equity = gs.range_equity_vs_random(*after_call.bb_range, 1_000);

            inspect_range_detailed(
                "BB call-after-jam range",
                after_call.bb_range.as_ref(),
                &interesting_hands,
                10,
                call_equity,
            );
        }
    }
}
