use crate::game::{CARD_MASKS, Card, CardSet, ChipState, GameState, Hand, Rank, Suit};
use crate::rng::XorShiftU64;
use crate::search::{Actions, Node, Outcome, Position, Range, inspect_range_summary, set_range_temp};

use std::io;
use std::time::Instant;

const EVEN_NAMES: [&str; 5] = ["Check", "Quarter", "Half", "Pot", "All-in"];
const BEHIND_NAMES: [&str; 4] = ["Fold", "Call", "2.5x", "All-in"];

pub struct AppliedAction {
    pub outcome: Option<Outcome>,
    pub street_advanced: bool,
}

fn print_results(root: &Node) {
    println!("\nROOT ACTIONS");
    println!("{:<10} {:>7} {:>12} {:>8} {:>12}", "Action", "Legal", "Probability", "Visits", "Mean EV",);
    println!("{}", "-".repeat(57));

    match root.actions.as_ref().expect("root should not be terminal") {
        Actions::Even(actions) => {
            for (i, name) in EVEN_NAMES.iter().enumerate() {
                let mean_ev = if actions.visits[i] == 0 { 0.0 } else { actions.total_ev[i] / actions.visits[i] as f64 };

                println!(
                    "{:<10} {:>7} {:>11.2}% {:>8} {:>12.3}",
                    name,
                    actions.legal[i],
                    100.0 * actions.probs[i],
                    actions.visits[i],
                    mean_ev,
                );
            }
        }

        Actions::Behind(actions) => {
            for (i, name) in BEHIND_NAMES.iter().enumerate() {
                let mean_ev = if actions.visits[i] == 0 { 0.0 } else { actions.total_ev[i] / actions.visits[i] as f64 };

                println!(
                    "{:<10} {:>7} {:>11.2}% {:>8} {:>12.3}",
                    name,
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

fn solve(gs: &GameState) -> Node {
    let start = Instant::now();

    println!("{gs}");

    let root = gs.do_runouts();

    println!("INFO analysis took: {:?}", start.elapsed());
    print_results(&root);

    root
}

fn action_name(root: &Node, choice: usize) -> &'static str {
    match root.actions.as_ref().expect("root should not be terminal") {
        Actions::Even(_) => EVEN_NAMES[choice],
        Actions::Behind(_) => BEHIND_NAMES[choice],
    }
}

fn apply_action(gs: &mut GameState, root: &Node, choice: usize) -> AppliedAction {
    let child = match root.actions.as_ref().expect("cannot act from a terminal node") {
        Actions::Even(actions) => {
            assert!(choice < actions.legal.len(), "invalid even-action index");
            assert!(actions.legal[choice], "attempted to apply illegal action {}", EVEN_NAMES[choice],);

            actions.children.as_ref().expect("generated node should have children")[choice].as_ref()
        }

        Actions::Behind(actions) => {
            assert!(choice < actions.legal.len(), "invalid behind-action index");
            assert!(actions.legal[choice], "attempted to apply illegal action {}", BEHIND_NAMES[choice],);

            actions.children.as_ref().expect("generated node should have children")[choice].as_ref()
        }
    };

    let street_advanced = child.streets_remaining < root.streets_remaining;

    gs.chip_state = child.chip_state;
    gs.turn = child.position;
    gs.sb_range = *child.sb_range;
    gs.bb_range = *child.bb_range;

    AppliedAction { outcome: child.outcome, street_advanced }
}

pub fn play_from(gs: &mut GameState) -> AppliedAction {
    set_range_temp(1.0);
    let policy_root = solve(gs);

    let mut rng = XorShiftU64::new();

    let (choice, probability) = match policy_root.actions.as_ref().expect("root should not be terminal") {
        Actions::Even(actions) => rng.choose_action(&actions.probs),
        Actions::Behind(actions) => rng.choose_action(&actions.probs),
    };

    println!(
        "DECISION chose action {} with probability {:.2}%",
        action_name(&policy_root, choice),
        100.0 * probability,
    );

    // Since our opponent probably plays differently to us, we increase the temperature when
    // considering their range/their perception of our range.
    set_range_temp(10.0);
    let range_root = gs.build_ranged_tree();
    set_range_temp(1.0);

    apply_action(gs, &range_root, choice)
}

fn receive_action(root: &Node) -> usize {
    let mut input = String::new();

    io::stdin().read_line(&mut input).expect("failed to read action");

    let action = input.trim().to_ascii_lowercase();

    let choice = match root.actions.as_ref().expect("terminal node has no actions") {
        Actions::Even(_) => match action.as_str() {
            "check" => 0,
            "quarter" => 1,
            "half" => 2,
            "pot" => 3,
            "all-in" => 4,

            _ => panic!(
                "invalid action; expected \
                 'check', 'quarter', 'half', 'pot', or 'all-in'"
            ),
        },

        Actions::Behind(_) => match action.as_str() {
            "fold" => 0,
            "call" => 1,
            "2.5x" => 2,
            "allin" => 3,

            _ => panic!(
                "invalid action; expected \
                 'fold', 'call', '2.5x', or 'all-in'"
            ),
        },
    };

    match root.actions.as_ref().unwrap() {
        Actions::Even(actions) => {
            assert!(actions.legal[choice], "action '{}' is not legal here", EVEN_NAMES[choice],);
        }

        Actions::Behind(actions) => {
            assert!(actions.legal[choice], "action '{}' is not legal here", BEHIND_NAMES[choice],);
        }
    }

    choice
}

fn receive_opponent_action(gs: &mut GameState) -> AppliedAction {
    let start = Instant::now();

    println!("{gs}");

    // As above.
    set_range_temp(10.0);
    let root = gs.build_ranged_tree();
    set_range_temp(1.0);

    println!("INFO range analysis took: {:?}", start.elapsed(),);
    println!("INFO waiting to receive opponent action");

    let choice = receive_action(&root);

    println!("INFO opponent chose {}", action_name(&root, choice),);

    apply_action(gs, &root, choice)
}

fn receive_next_street(gs: &mut GameState) {
    let (street_name, cards_to_receive) = match gs.board_len {
        0 => ("flop", 3),
        3 => ("turn", 1),
        4 => ("river", 1),
        5 => return,

        _ => unreachable!("invalid board length: {}", gs.board_len,),
    };

    println!("INFO waiting to receive {street_name}");

    for _ in 0..cards_to_receive {
        let card = Card::receive();

        assert!(gs.seen & CARD_MASKS[card] == 0, "card {card} has already been seen",);

        gs.add_card(card);
    }

    println!("INFO board is now {}", gs.board);
}

pub trait Parseable {
    fn parse(s: &str) -> Self;

    fn receive() -> Self
    where
        Self: Sized,
    {
        let mut input = String::new();

        io::stdin().read_line(&mut input).expect("failed to read input");

        Self::parse(input.trim())
    }
}

impl Parseable for Card {
    fn parse(s: &str) -> Self {
        let error_msg = "invalid card; expected [rank][suit], \
             for example 'Ac', 'Th', or '2s'";

        let bytes = s.as_bytes();

        assert_eq!(bytes.len(), 2, "{error_msg}");

        let rank = match bytes[0].to_ascii_uppercase() {
            b'2' => Rank::Two,
            b'3' => Rank::Three,
            b'4' => Rank::Four,
            b'5' => Rank::Five,
            b'6' => Rank::Six,
            b'7' => Rank::Seven,
            b'8' => Rank::Eight,
            b'9' => Rank::Nine,
            b'T' => Rank::Ten,
            b'J' => Rank::Jack,
            b'Q' => Rank::Queen,
            b'K' => Rank::King,
            b'A' => Rank::Ace,
            _ => panic!("{error_msg}"),
        };

        let suit = match bytes[1].to_ascii_lowercase() {
            b'h' => Suit::Hearts,
            b'd' => Suit::Diamonds,
            b's' => Suit::Spades,
            b'c' => Suit::Clubs,
            _ => panic!("{error_msg}"),
        };

        Card::new(rank, suit)
    }
}

impl Parseable for Hand {
    fn parse(s: &str) -> Self {
        assert_eq!(s.len(), 4, "invalid hand. expected two cards e.g. 'AsAd'",);
        let (first_card, second_card) = s.split_at(2);
        let hand = (Card::parse(first_card), Card::parse(second_card));

        hand
    }
}

impl Parseable for Position {
    fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "sb" => Position::SmallBlind,
            "bb" => Position::BigBlind,

            _ => panic!("invalid position. expected 'sb' or 'bb'"),
        }
    }
}

impl Parseable for ChipState {
    /// Parses the initial state immediately after the blinds.
    fn parse(s: &str) -> Self {
        let error_msg = "failed to parse chip state; expected \
             'sb [stack] [bet] bb [stack] [bet]'";

        let parts = s.split_whitespace().collect::<Vec<_>>();

        assert!(parts.len() == 6 && parts[0] == "sb" && parts[3] == "bb", "{error_msg}",);

        let sb_stack = parts[1].parse::<i32>().expect(error_msg);
        let sb_bet = parts[2].parse::<i32>().expect(error_msg);
        let bb_stack = parts[4].parse::<i32>().expect(error_msg);
        let bb_bet = parts[5].parse::<i32>().expect(error_msg);

        Self {
            pot: sb_bet + bb_bet,
            sb_stack,
            bb_stack,
            sb_this_street: sb_bet,
            bb_this_street: bb_bet,
            max_bet: sb_bet.max(bb_bet),
        }
    }
}

const INSPECT_RANGES: bool = false;

pub fn hand_loop() {
    println!("INFO waiting to receive chip state after blinds (sb [stack] [bet] bb [stack] [bet])");
    let chip_state = ChipState::receive();

    println!("INFO waiting to receive my position (sb|bb)");
    let hero_position = Position::receive();

    let turn = Position::SmallBlind;

    println!("INFO waiting to receive my hand (e.g. AsAd)");
    let hand = Hand::receive();

    let mut gs = GameState {
        chip_state,
        turn,
        board: CardSet::BLANK,
        sb_range: Range::BLANK,
        bb_range: Range::BLANK,
        hero_hand: hand,
        seen: 0,
        board_len: 0,
    };

    gs.set_hero_hand(hand);

    loop {
        let applied = if gs.turn == hero_position { play_from(&mut gs) } else { receive_opponent_action(&mut gs) };

        if let Some(outcome) = applied.outcome {
            println!("INFO hand ended: {outcome:?}");
            break;
        }

        if applied.street_advanced {
            receive_next_street(&mut gs);
        }

        if INSPECT_RANGES {
            inspect_range_summary("SB range", &gs, &gs.sb_range);
            inspect_range_summary("BB range", &gs, &gs.bb_range);
        }
    }
}
