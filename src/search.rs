use crate::game::{CARD_MASKS, CARDS, Card, CardSet, ChipState, GameState, cfor};
use crate::rng::XorShiftU64;
use std::cmp::Ordering;

#[derive(Debug, Clone)]
pub struct Node {
    pub terminal: bool,
    pub position: Position,
    pub node_type: NodeType,
    pub chip_state: ChipState,
    pub actions: Option<Actions>,
    pub outcome: Option<Outcome>,
    pub sb_range: Range,
    pub bb_range: Range,
}

#[derive(Debug, Clone, Copy)]
pub struct Range {
    pub probs: [[f64; Card::NUM]; Card::NUM],
    pub equity_against_with_hand: f64,
    pub equity_against_with_range: f64,
}

pub struct ScoreCache {
    pub scores: [[Option<i32>; Card::NUM]; Card::NUM],
}

impl ScoreCache {
    pub const BLANK: Self = Self { scores: [[None; Card::NUM]; Card::NUM] };

    pub fn from_board(board: CardSet) -> Self {
        let mut cache = Self::BLANK;

        for c1 in CARDS {
            for c2 in CARDS {
                if c1 <= c2 {
                    continue;
                }

                let mut cardset = board;
                cardset.update_with(c1);
                cardset.update_with(c2);

                let score = cardset.score();

                cache.scores[c1][c2] = Some(score);
                cache.scores[c2][c1] = Some(score);
            }
        }

        cache
    }
}

impl Range {
    const fn no_information() -> Self {
        let cnt = 26.0 * 51.0;
        let mut probs = [[1.0 / cnt; Card::NUM]; Card::NUM];

        cfor!(let mut i = 0; i < Card::NUM; i += 1;{
            probs[i][i] = 0.0;
        });

        Self { probs, equity_against_with_hand: 0.0, equity_against_with_range: 0.0 }
    }

    pub const BLANK: Self = Self::no_information();

    pub fn update_from_seen(&mut self, seen: u64) {
        let mut t = 0.0;
        for c1 in CARDS {
            for c2 in CARDS {
                if c1 <= c2 {
                    continue;
                }

                if (seen & CARD_MASKS[c1]) > 0 || (seen & CARD_MASKS[c2]) > 0 {
                    self.probs[c1][c2] = 0.0;
                } else {
                    t += self.probs[c1][c2];
                }
            }
        }

        for c1 in CARDS {
            for c2 in CARDS {
                if c1 <= c2 {
                    continue;
                }

                self.probs[c1][c2] /= t;
            }
        }
    }

    fn equity_table(&self, cache: &ScoreCache) -> [[f64; Card::NUM]; Card::NUM] {
        let mut equities = [[0.0; Card::NUM]; Card::NUM];

        for our_c1 in CARDS {
            for our_c2 in CARDS {
                if our_c1 <= our_c2 {
                    continue;
                }

                let hand_mask = CARD_MASKS[our_c1] | CARD_MASKS[our_c2];
                let our_score = cache.scores[our_c1][our_c2].unwrap();

                let mut equity = 0.0;
                let mut total = 0.0;

                for c1 in CARDS {
                    if hand_mask & CARD_MASKS[c1] > 0 {
                        continue;
                    }

                    for c2 in CARDS {
                        if c1 <= c2 || self.probs[c1][c2] == 0.0 || hand_mask & CARD_MASKS[c2] > 0 {
                            continue;
                        }

                        let their_score = cache.scores[c1][c2].unwrap();

                        let result = match our_score.cmp(&their_score) {
                            Ordering::Greater => 1.0,
                            Ordering::Equal => 0.5,
                            Ordering::Less => 0.0,
                        };

                        total += self.probs[c1][c2];
                        equity += result * self.probs[c1][c2];
                    }
                }

                let equity = equity / total;

                equities[our_c1][our_c2] = equity;
                equities[our_c2][our_c1] = equity;
            }
        }

        equities
    }

    fn equity_against_with_range(&self, range: Range, equities: &[[f64; Card::NUM]; Card::NUM]) -> f64 {
        let mut equity = 0.0;

        for c1 in CARDS {
            for c2 in CARDS {
                if c1 <= c2 || range.probs[c1][c2] == 0.0 {
                    continue;
                }

                equity += equities[c1][c2] * range.probs[c1][c2];
            }
        }

        equity
    }
}

/// (1) Even betting actions:
/// - check
/// - bet 1/4 pot
/// - bet 1/2 pot
/// - bet full pot
/// - all in
#[derive(Debug, Clone)]
pub struct EvenActions {
    pub probs: [f64; 5],
    children: Option<[Box<Node>; 5]>,
}

impl EvenActions {
    pub const BLANK: Self = Self { probs: [0.2; 5], children: None };
}

/// (2) Opponent's betting lead:
/// - fold
/// - call
/// - 2.5x re-raise
/// - all in
#[derive(Debug, Clone)]
pub struct BehindActions {
    pub probs: [f64; 4],
    children: Option<[Box<Node>; 4]>,
}

impl BehindActions {
    pub const BLANK: Self = Self { probs: [0.25; 4], children: None };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NodeType {
    EvenNode,
    BehindNode,
    AheadNode,
}

#[derive(Debug, Clone)]
pub enum Actions {
    Even(EvenActions),
    Behind(BehindActions),
}

#[derive(Debug, Clone, Copy)]
pub enum Outcome {
    Showdown, // goes to showdown -> resolve with equities
    BBFolded, // BB folded -> SB gets all chips
    SBFolded, // SB folded -> BB gets all chips
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Position {
    SmallBlind,
    BigBlind,
}

impl Position {
    fn next(&self) -> Self {
        match self {
            Position::SmallBlind => Position::BigBlind,
            Position::BigBlind => Position::SmallBlind,
        }
    }
}

pub enum Bet {
    SBBet(i32),
    BBBet(i32),
}

impl ChipState {
    fn update_with(&mut self, bet: Bet) {
        match bet {
            Bet::SBBet(k) => {
                self.sb_stack -= k;
                self.sb_this_street += k;
                self.pot += k;
                self.max_bet = self.max_bet.max(self.sb_this_street);
            }

            Bet::BBBet(k) => {
                self.bb_stack -= k;
                self.bb_this_street += k;
                self.pot += k;
                self.max_bet = self.max_bet.max(self.bb_this_street);
            }
        }
    }
}

fn update_probs<const N: usize>(probs: &mut [f64; N], choice: usize, player_ev: f64) {
    probs[choice] *= (LR * player_ev).exp();

    let total = probs.iter().sum::<f64>();

    for probability in probs {
        *probability /= total;
    }
}

const NUM_RUNOUTS: usize = 1024;
const NUM_PLAYOUTS: usize = 1024;
const LR: f64 = 1.0 / NUM_PLAYOUTS as f64;

const RANGE_CALC_RUNOUTS: usize = 100;

impl GameState {
    pub fn do_runouts(&self) -> Node {
        let nt = if self.chip_state.sb_this_street == self.chip_state.bb_this_street {
            NodeType::EvenNode
        } else {
            NodeType::BehindNode
        };

        let mut rng = XorShiftU64::new();

        let mut base_root = Node::from(self.chip_state, self.turn, nt, false, None, self.sb_range, self.bb_range);

        base_root.gen_subtree();

        let mut averaged_root = base_root.policy_zeroed();

        for _ in 0..NUM_RUNOUTS {
            let mut runout_root = base_root.clone();
            let mut runout = self.gen_runout();

            let hero_mask = CARD_MASKS[self.hero_hand.0] | CARD_MASKS[self.hero_hand.1];

            let hero_seen = runout.seen;
            let public_seen = runout.seen & !hero_mask;

            let cache = ScoreCache::from_board(runout.board);

            match self.turn {
                Position::SmallBlind => {
                    runout.sb_range.update_from_seen(public_seen);
                    runout.bb_range.update_from_seen(hero_seen);

                    runout_root.sb_range = runout.sb_range;
                    runout_root.bb_range = runout.bb_range;

                    let equities = runout_root.bb_range.equity_table(&cache);

                    runout_root.bb_range.equity_against_with_hand = equities[self.hero_hand.0][self.hero_hand.1];

                    runout_root.bb_range.equity_against_with_range =
                        runout_root.bb_range.equity_against_with_range(runout_root.sb_range, &equities);
                }

                Position::BigBlind => {
                    runout.sb_range.update_from_seen(hero_seen);
                    runout.bb_range.update_from_seen(public_seen);

                    runout_root.sb_range = runout.sb_range;
                    runout_root.bb_range = runout.bb_range;

                    let equities = runout_root.sb_range.equity_table(&cache);

                    runout_root.sb_range.equity_against_with_hand = equities[self.hero_hand.0][self.hero_hand.1];

                    runout_root.sb_range.equity_against_with_range =
                        runout_root.sb_range.equity_against_with_range(runout_root.bb_range, &equities);
                }
            }

            for _ in 0..NUM_PLAYOUTS {
                runout_root.playout_from_root(&mut rng);
            }

            averaged_root.add_policy_from(&runout_root);
        }

        averaged_root.scale_policy(1.0 / NUM_RUNOUTS as f64);
        averaged_root
    }

    pub fn update_ranges_with_decision(&mut self, decision_idx: usize) {
        let nt = if self.chip_state.sb_this_street == self.chip_state.bb_this_street {
            NodeType::EvenNode
        } else {
            NodeType::BehindNode
        };

        let mut rng = XorShiftU64::new();

        let mut base_root = Node::from(self.chip_state, self.turn, nt, false, None, self.sb_range, self.bb_range);
        base_root.gen_subtree();

        let acting_range = match self.turn {
            Position::SmallBlind => self.sb_range,
            Position::BigBlind => self.bb_range,
        };

        let mut likelihood_sums = [[0.0; Card::NUM]; Card::NUM];
        let mut runout_counts = [[0; Card::NUM]; Card::NUM];

        let mut public_state = self.clone();
        public_state.remove_hero_hand();

        for _ in 0..RANGE_CALC_RUNOUTS {
            let mut runout = public_state.gen_runout();
            let public_seen = runout.seen;

            runout.sb_range.update_from_seen(public_seen);
            runout.bb_range.update_from_seen(public_seen);

            let cache = ScoreCache::from_board(runout.board);

            let (equities, range_equity) = match self.turn {
                Position::SmallBlind => {
                    let equities = runout.bb_range.equity_table(&cache);
                    let range_equity = runout.bb_range.equity_against_with_range(runout.sb_range, &equities);

                    (equities, range_equity)
                }

                Position::BigBlind => {
                    let equities = runout.sb_range.equity_table(&cache);
                    let range_equity = runout.sb_range.equity_against_with_range(runout.bb_range, &equities);

                    (equities, range_equity)
                }
            };

            for c1 in CARDS {
                for c2 in CARDS {
                    if c1 <= c2
                        || acting_range.probs[c1][c2] == 0.0
                        || public_seen & CARD_MASKS[c1] != 0
                        || public_seen & CARD_MASKS[c2] != 0
                    {
                        continue;
                    }

                    let mut runout_root = base_root.clone();
                    runout_root.sb_range = runout.sb_range;
                    runout_root.bb_range = runout.bb_range;

                    match self.turn {
                        Position::SmallBlind => {
                            runout_root.bb_range.equity_against_with_hand = equities[c1][c2];
                            runout_root.bb_range.equity_against_with_range = range_equity;
                        }

                        Position::BigBlind => {
                            runout_root.sb_range.equity_against_with_hand = equities[c1][c2];
                            runout_root.sb_range.equity_against_with_range = range_equity;
                        }
                    }

                    for _ in 0..NUM_PLAYOUTS {
                        runout_root.playout_from_root(&mut rng);
                    }

                    let likelihood = match runout_root.actions.as_ref().unwrap() {
                        Actions::Even(actions) => actions.probs[decision_idx],
                        Actions::Behind(actions) => actions.probs[decision_idx],
                    };

                    likelihood_sums[c1][c2] += likelihood;
                    runout_counts[c1][c2] += 1;
                }
            }
        }

        let range = match self.turn {
            Position::SmallBlind => &mut self.sb_range,
            Position::BigBlind => &mut self.bb_range,
        };

        let mut total = 0.0;

        for c1 in CARDS {
            for c2 in CARDS {
                if c1 <= c2 || range.probs[c1][c2] == 0.0 {
                    continue;
                }

                let count = runout_counts[c1][c2];

                if count == 0 {
                    range.probs[c1][c2] = 0.0;
                    continue;
                }

                let likelihood = likelihood_sums[c1][c2] / count as f64;

                range.probs[c1][c2] *= likelihood;
                total += range.probs[c1][c2];
            }
        }

        assert!(total > 0.0);

        for c1 in CARDS {
            for c2 in CARDS {
                if c1 <= c2 {
                    continue;
                }

                range.probs[c1][c2] /= total;
            }
        }
    }
}

impl Node {
    fn policy_zeroed(&self) -> Self {
        let mut result = self.clone();
        result.zero_policy();
        result
    }

    fn zero_policy(&mut self) {
        let Some(actions) = self.actions.as_mut() else {
            return;
        };

        match actions {
            Actions::Even(a) => {
                a.probs.fill(0.0);

                if let Some(children) = a.children.as_mut() {
                    for child in children {
                        child.zero_policy();
                    }
                }
            }

            Actions::Behind(a) => {
                a.probs.fill(0.0);

                if let Some(children) = a.children.as_mut() {
                    for child in children {
                        child.zero_policy();
                    }
                }
            }
        }
    }

    fn add_policy_from(&mut self, other: &Node) {
        match (self.actions.as_mut(), other.actions.as_ref()) {
            (None, None) => {}

            (Some(Actions::Even(this)), Some(Actions::Even(other))) => {
                for (dst, src) in this.probs.iter_mut().zip(other.probs) {
                    *dst += src;
                }

                match (this.children.as_mut(), other.children.as_ref()) {
                    (Some(this_children), Some(other_children)) => {
                        for (dst, src) in this_children.iter_mut().zip(other_children) {
                            dst.add_policy_from(src);
                        }
                    }

                    (None, None) => {}

                    _ => unreachable!(),
                }
            }

            (Some(Actions::Behind(this)), Some(Actions::Behind(other))) => {
                for (dst, src) in this.probs.iter_mut().zip(other.probs) {
                    *dst += src;
                }

                match (this.children.as_mut(), other.children.as_ref()) {
                    (Some(this_children), Some(other_children)) => {
                        for (dst, src) in this_children.iter_mut().zip(other_children) {
                            dst.add_policy_from(src);
                        }
                    }

                    (None, None) => {}

                    _ => unreachable!(),
                }
            }

            _ => unreachable!(),
        }
    }

    fn scale_policy(&mut self, factor: f64) {
        let Some(actions) = self.actions.as_mut() else {
            return;
        };

        match actions {
            Actions::Even(actions) => {
                for probability in &mut actions.probs {
                    *probability *= factor;
                }

                if let Some(children) = actions.children.as_mut() {
                    for child in children {
                        child.scale_policy(factor);
                    }
                }
            }

            Actions::Behind(actions) => {
                for probability in &mut actions.probs {
                    *probability *= factor;
                }

                if let Some(children) = actions.children.as_mut() {
                    for child in children {
                        child.scale_policy(factor);
                    }
                }
            }
        }
    }

    fn playout_from_root(&mut self, rng: &mut XorShiftU64) {
        let root_position = self.position;
        let starting_stack = match root_position {
            Position::SmallBlind => self.chip_state.sb_stack as f64,
            Position::BigBlind => self.chip_state.bb_stack as f64,
        };

        let (choices, result, outcome) = self.playout(rng);

        let final_stack_hero_pov = match (root_position, outcome) {
            // hero is SB and reaches showdown
            (Position::SmallBlind, Outcome::Showdown) => {
                result.sb_stack as f64 + result.pot as f64 * self.bb_range.equity_against_with_hand
            }

            // hero is BB and reaches showdown
            (Position::BigBlind, Outcome::Showdown) => {
                result.bb_stack as f64 + result.pot as f64 * self.sb_range.equity_against_with_hand
            }

            // BB folded -> hero in SB wins pot
            (Position::SmallBlind, Outcome::BBFolded) => result.sb_stack as f64 + result.pot as f64,

            // hero in BB folded -> SB wins pot
            (Position::BigBlind, Outcome::BBFolded) => result.bb_stack as f64,

            // hero in SB folded -> BB wins pot
            (Position::SmallBlind, Outcome::SBFolded) => result.sb_stack as f64,

            // SB folded -> hero in BB wins pot
            (Position::BigBlind, Outcome::SBFolded) => result.bb_stack as f64 + result.pot as f64,
        };

        let hero_pov_ev = final_stack_hero_pov - starting_stack;

        let villain_starting_stack = match root_position {
            Position::SmallBlind => self.chip_state.bb_stack as f64,
            Position::BigBlind => self.chip_state.sb_stack as f64,
        };

        let villain_final_stack = match (root_position, outcome) {
            (Position::SmallBlind, Outcome::Showdown) => {
                result.bb_stack as f64 + result.pot as f64 * (1.0 - self.bb_range.equity_against_with_range)
            }

            (Position::BigBlind, Outcome::Showdown) => {
                result.sb_stack as f64 + result.pot as f64 * (1.0 - self.sb_range.equity_against_with_range)
            }

            (Position::SmallBlind, Outcome::BBFolded) => result.bb_stack as f64,

            (Position::SmallBlind, Outcome::SBFolded) => result.bb_stack as f64 + result.pot as f64,

            (Position::BigBlind, Outcome::BBFolded) => result.sb_stack as f64 + result.pot as f64,

            (Position::BigBlind, Outcome::SBFolded) => result.sb_stack as f64,
        };

        let villain_pov_ev = villain_final_stack - villain_starting_stack;

        let mut node = self;

        for choice in choices {
            let player_ev = if node.position == root_position { hero_pov_ev } else { villain_pov_ev };

            let next_node = match node.actions.as_mut().unwrap() {
                Actions::Even(actions) => {
                    update_probs(&mut actions.probs, choice, player_ev);

                    actions.children.as_mut().unwrap()[choice].as_mut()
                }

                Actions::Behind(actions) => {
                    update_probs(&mut actions.probs, choice, player_ev);

                    actions.children.as_mut().unwrap()[choice].as_mut()
                }
            };

            node = next_node;
        }
    }

    fn playout(&self, rng: &mut XorShiftU64) -> (Vec<usize>, ChipState, Outcome) {
        if self.terminal {
            return (vec![], self.chip_state, self.outcome.unwrap());
        }

        let Some(actions) = self.actions.as_ref() else {
            unreachable!();
        };

        match actions {
            Actions::Even(a) => {
                let choice = rng.explore_action(&a.probs);

                let (choices, chip_state, outcome) = a.children.as_ref().unwrap()[choice].playout(rng);

                let mut path = Vec::with_capacity(choices.len() + 1);
                path.push(choice);
                path.extend(choices);

                (path, chip_state, outcome)
            }

            Actions::Behind(a) => {
                let choice = rng.explore_action(&a.probs);

                let (choices, chip_state, outcome) = a.children.as_ref().unwrap()[choice].playout(rng);

                let mut path = Vec::with_capacity(choices.len() + 1);
                path.push(choice);
                path.extend(choices);

                (path, chip_state, outcome)
            }
        }
    }

    fn from(
        chip_state: ChipState,
        position: Position,
        node_type: NodeType,
        terminal: bool,
        outcome: Option<Outcome>,
        sb_range: Range,
        bb_range: Range,
    ) -> Self {
        let actions = match (terminal, node_type) {
            (true, _) => None,
            (_, NodeType::EvenNode) => Some(Actions::Even(EvenActions::BLANK)),
            (_, NodeType::BehindNode) => Some(Actions::Behind(BehindActions::BLANK)),
            (_, NodeType::AheadNode) => unreachable!(),
            // AheadNode can only occur after opponent folded (terminal)
        };

        Self { terminal, position, node_type, chip_state, actions, outcome, sb_range, bb_range }
    }

    fn successor(&self, action_idx: usize) -> Option<Self> {
        if self.terminal {
            return None;
        }

        let our_stack = match self.position {
            Position::SmallBlind => self.chip_state.sb_stack,
            Position::BigBlind => self.chip_state.bb_stack,
        };

        if self.node_type == NodeType::EvenNode {
            if action_idx == 0 {
                //check
                let mut r = match self.position {
                    // opponent just checked
                    Position::SmallBlind => Node::from(
                        self.chip_state,
                        Position::BigBlind,
                        NodeType::EvenNode,
                        true,
                        Some(Outcome::Showdown),
                        self.sb_range,
                        self.bb_range,
                    ),

                    Position::BigBlind => Node::from(
                        self.chip_state,
                        Position::SmallBlind,
                        NodeType::EvenNode,
                        false,
                        None,
                        self.sb_range,
                        self.bb_range,
                    ),
                };

                r.gen_subtree();
                return Some(r);
            }

            let mut amount = match action_idx {
                1 => self.chip_state.pot / 4,
                2 => self.chip_state.pot / 2,
                3 => self.chip_state.pot,
                4 => our_stack,
                _ => unreachable!(),
            };

            amount = amount.max(self.chip_state.max_bet).min(our_stack);
            // can't just clamp as it's possible our stack < max bet

            let bet = match self.position {
                Position::SmallBlind => Bet::SBBet(amount),
                Position::BigBlind => Bet::BBBet(amount),
            };

            let mut ns = self.chip_state;
            ns.update_with(bet);

            let mut r =
                Node::from(ns, self.position.next(), NodeType::BehindNode, false, None, self.sb_range, self.bb_range);

            r.gen_subtree();
            return Some(r);
        } else {
            let amount_behind = match self.position {
                Position::SmallBlind => self.chip_state.bb_this_street - self.chip_state.sb_this_street,
                Position::BigBlind => self.chip_state.sb_this_street - self.chip_state.bb_this_street,
            };

            match action_idx {
                0 => {
                    let outcome = match self.position {
                        Position::SmallBlind => Outcome::SBFolded,
                        Position::BigBlind => Outcome::BBFolded,
                    };

                    let mut r = Node::from(
                        self.chip_state,
                        self.position.next(),
                        NodeType::AheadNode,
                        true,
                        Some(outcome),
                        self.sb_range,
                        self.bb_range,
                    );

                    r.gen_subtree();
                    return Some(r);
                }

                1 => {
                    let amount = amount_behind.min(our_stack);

                    let bet = match self.position {
                        Position::SmallBlind => Bet::SBBet(amount),
                        Position::BigBlind => Bet::BBBet(amount),
                    };

                    let mut ns = self.chip_state;
                    ns.update_with(bet);

                    let mut r = Node::from(
                        ns,
                        self.position.next(),
                        NodeType::EvenNode,
                        true,
                        Some(Outcome::Showdown),
                        self.sb_range,
                        self.bb_range,
                    );

                    r.gen_subtree();
                    return Some(r);
                }

                2 => {
                    let opponent_total = match self.position {
                        Position::SmallBlind => self.chip_state.bb_this_street,
                        Position::BigBlind => self.chip_state.sb_this_street,
                    };

                    let our_total = match self.position {
                        Position::SmallBlind => self.chip_state.sb_this_street,
                        Position::BigBlind => self.chip_state.bb_this_street,
                    };

                    let target_total = (opponent_total * 5) / 2;
                    let amount = (target_total - our_total).min(our_stack);

                    let bet = match self.position {
                        Position::SmallBlind => Bet::SBBet(amount),
                        Position::BigBlind => Bet::BBBet(amount),
                    };

                    let mut ns = self.chip_state;
                    ns.update_with(bet);

                    let (terminal, outcome) =
                        if amount <= amount_behind { (true, Some(Outcome::Showdown)) } else { (false, None) };

                    let mut r = Node::from(
                        ns,
                        self.position.next(),
                        NodeType::BehindNode,
                        terminal,
                        outcome,
                        self.sb_range,
                        self.bb_range,
                    );

                    r.gen_subtree();
                    return Some(r);
                }
                3 => {
                    let amount = our_stack;

                    let bet = match self.position {
                        Position::SmallBlind => Bet::SBBet(amount),
                        Position::BigBlind => Bet::BBBet(amount),
                    };

                    let mut ns = self.chip_state;
                    ns.update_with(bet);

                    let (terminal, outcome) =
                        if amount <= amount_behind { (true, Some(Outcome::Showdown)) } else { (false, None) };

                    let mut r = Node::from(
                        ns,
                        self.position.next(),
                        NodeType::BehindNode,
                        terminal,
                        outcome,
                        self.sb_range,
                        self.bb_range,
                    );

                    r.gen_subtree();
                    return Some(r);
                }

                _ => unreachable!(),
            }
        }
    }

    pub fn gen_subtree(&mut self) {
        if self.terminal {
            return;
        }

        match self.node_type {
            NodeType::BehindNode => {
                let children = (0..4).map(|i| Box::new(self.successor(i).unwrap())).collect::<Vec<_>>();

                let mut actions = BehindActions::BLANK;
                actions.children = Some(children.try_into().unwrap());

                self.actions = Some(Actions::Behind(actions));
            }
            NodeType::EvenNode => {
                let children = (0..5).map(|i| Box::new(self.successor(i).unwrap())).collect::<Vec<_>>();
                let mut actions = EvenActions::BLANK;
                actions.children = Some(children.try_into().unwrap());

                self.actions = Some(Actions::Even(actions))
            }
            _ => unreachable!(),
        }
    }
}
