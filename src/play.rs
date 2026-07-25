use crate::game::GameState;
use crate::rng::XorShiftU64;
use crate::search::{Actions, Node};

use std::time::Instant;

const EVEN_NAMES: [&'static str; 5] = ["Check", "Quarter", "Half", "Pot", "All-in"];
const BEHIND_NAMES: [&str; 4] = ["Fold", "Call", "2.5x", "All-in"];

fn print_results(root: &Node) {
    println!("\nROOT ACTIONS");

    match root.actions.as_ref().unwrap() {
        Actions::Even(actions) => {
            println!("{:<10} {:>7} {:>12} {:>8} {:>12}", "Action", "Legal", "Probability", "Visits", "Mean EV");
            println!("{}", "-".repeat(57));

            for i in 0..5 {
                let mean_ev = if actions.visits[i] == 0 { 0.0 } else { actions.total_ev[i] / actions.visits[i] as f64 };

                println!(
                    "{:<10} {:>7} {:>11.2}% {:>8} {:>12.3}",
                    EVEN_NAMES[i],
                    actions.legal[i],
                    100.0 * actions.probs[i],
                    actions.visits[i],
                    mean_ev,
                );
            }
        }

        Actions::Behind(actions) => {
            println!("{:<10} {:>7} {:>12} {:>8} {:>12}", "Action", "Legal", "Probability", "Visits", "Mean EV");
            println!("{}", "-".repeat(57));

            for i in 0..4 {
                let mean_ev = if actions.visits[i] == 0 { 0.0 } else { actions.total_ev[i] / actions.visits[i] as f64 };

                println!(
                    "{:<10} {:>7} {:>11.2}% {:>8} {:>12.3}",
                    BEHIND_NAMES[i],
                    actions.legal[i],
                    100.0 * actions.probs[i],
                    actions.visits[i],
                    mean_ev,
                );
            }
        }
    }

    println!();
}

pub fn play_from(gs: &mut GameState) {
    let start = Instant::now();

    println!("{gs}");

    let root = gs.do_runouts();

    println!("INFO analysis took: {:?}", start.elapsed());
    print_results(&root);

    let mut rng = XorShiftU64::new();

    match root.actions {
        Some(Actions::Even(a)) => {
            let (choice, p) = rng.choose_action(&a.probs);
            println!("DECISION chose action {} with probability {:.3}", EVEN_NAMES[choice], p);
        }
        Some(Actions::Behind(a)) => {
            let (choice, p) = rng.choose_action(&a.probs);
            println!("DECISION chose action {} with probability {:.3}", BEHIND_NAMES[choice], p);
        }
        _ => unreachable!(),
    };
}
