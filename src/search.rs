use crate::game::{CARD_MASKS, CARDS, Card, CardSet, ChipState, GameState, Hand, cfor};
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
    sorted_hands: Vec<Hand>,
}

impl ScoreCache {
    pub const BLANK: Self = Self { scores: [[None; Card::NUM]; Card::NUM], sorted_hands: Vec::new() };

    pub fn from_board(board: CardSet) -> Self {
        let mut cache = Self::BLANK;
        let mut sorted_hands = Vec::with_capacity(HAND_COUNT);

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

                sorted_hands.push((c1, c2));
            }
        }

        sorted_hands.sort_unstable_by_key(|&(c1, c2)| cache.scores[c1][c2].unwrap());
        cache.sorted_hands = sorted_hands;

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
        let mut prefix = [0.0; HAND_COUNT + 1];

        for (i, &(c1, c2)) in cache.sorted_hands.iter().enumerate() {
            prefix[i + 1] = prefix[i] + self.probs[c1][c2];
        }

        let total_probability = prefix[HAND_COUNT];

        for our_c1 in CARDS {
            for our_c2 in CARDS {
                if our_c1 <= our_c2 {
                    continue;
                }

                let our_score = cache.scores[our_c1][our_c2].unwrap();

                let first_equal =
                    cache.sorted_hands.partition_point(|&(c1, c2)| cache.scores[c1][c2].unwrap() < our_score);

                let first_greater =
                    cache.sorted_hands.partition_point(|&(c1, c2)| cache.scores[c1][c2].unwrap() <= our_score);

                let mut win_probability = prefix[first_equal];
                let mut tie_probability = prefix[first_greater] - prefix[first_equal];
                let mut valid_probability = total_probability;

                let mut remove_blocked = |c1: Card, c2: Card| {
                    let probability = if c1 > c2 { self.probs[c1][c2] } else { self.probs[c2][c1] };

                    if probability == 0.0 {
                        return;
                    }

                    valid_probability -= probability;

                    match cache.scores[c1][c2].unwrap().cmp(&our_score) {
                        Ordering::Less => win_probability -= probability,
                        Ordering::Equal => tie_probability -= probability,
                        Ordering::Greater => {}
                    }
                };

                for c in CARDS {
                    if c != our_c1 {
                        remove_blocked(our_c1, c);
                    }
                }

                for c in CARDS {
                    if c != our_c1 && c != our_c2 {
                        remove_blocked(our_c2, c);
                    }
                }

                let equity = (win_probability + 0.5 * tie_probability) / valid_probability;

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
    pub total_ev: [f64; 5],
    pub visits: [u32; 5],
    pub legal: [bool; 5],
    pub children: Option<[Box<Node>; 5]>,
}

impl EvenActions {
    pub const BLANK: Self =
        Self { probs: [0.2; 5], total_ev: [0.0; 5], visits: [0; 5], legal: [true; 5], children: None };
}

/// (2) Opponent's betting lead:
/// - fold
/// - call
/// - 2.5x re-raise
/// - all in
#[derive(Debug, Clone)]
pub struct BehindActions {
    pub probs: [f64; 4],
    pub total_ev: [f64; 4],
    pub visits: [u32; 4],
    pub legal: [bool; 4],
    pub children: Option<[Box<Node>; 4]>,
}

impl BehindActions {
    pub const BLANK: Self =
        Self { probs: [0.25; 4], total_ev: [0.0; 4], visits: [0; 4], legal: [true; 4], children: None };
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

const TEMPERATURE: f64 = 25.0;

fn softmax<const N: usize>(values: &[f64; N], legal: &[bool; N]) -> [f64; N] {
    let max_value = (0..N).filter(|&i| legal[i]).map(|i| values[i]).fold(f64::NEG_INFINITY, f64::max);

    let mut probs = [0.0; N];
    let mut total = 0.0;

    for i in 0..N {
        if legal[i] {
            probs[i] = ((values[i] - max_value) / TEMPERATURE).exp();
            total += probs[i];
        }
    }

    for probability in &mut probs {
        *probability /= total;
    }

    probs
}

fn update_probs<const N: usize>(
    probs: &mut [f64; N],
    total_ev: &mut [f64; N],
    visits: &mut [u32; N],
    legal: &[bool; N],
    choice: usize,
    player_ev: f64,
) {
    assert!(legal[choice]);

    total_ev[choice] += player_ev;
    visits[choice] += 1;

    let mut mean_ev = [0.0; N];

    for i in 0..N {
        if visits[i] > 0 {
            mean_ev[i] = total_ev[i] / visits[i] as f64;
        }
    }

    *probs = softmax(&mean_ev, legal);
}

pub enum HandPolicies {
    Even([[[f64; 5]; Card::NUM]; Card::NUM]),
    Behind([[[f64; 4]; Card::NUM]; Card::NUM]),
}

const NUM_PLAYOUTS: usize = 100;
const HAND_COUNT: usize = Card::NUM * (Card::NUM - 1) / 2;
const HAND_BATCH_SIZE: usize = HAND_COUNT;
const NUM_RANGE_PASSES: usize = 5;

impl GameState {
    pub fn do_runouts(&self) -> Node {
        let nt = if self.chip_state.sb_this_street == self.chip_state.bb_this_street {
            NodeType::EvenNode
        } else {
            NodeType::BehindNode
        };

        let mut rng = XorShiftU64::new();

        let mut root = Node::from(self.chip_state, self.turn, nt, false, None, self.sb_range, self.bb_range);

        root.gen_subtree();

        for pass_idx in 0..NUM_RANGE_PASSES {
            root.update_subtree_ranges(self.board, self.seen, self.hero_hand, self.board_len);
            println!("INFO completed range pass {}/{}", pass_idx + 1, NUM_RANGE_PASSES);
        }

        for _ in 0..NUM_PLAYOUTS {
            let runout = self.gen_runout();

            let hero_mask = CARD_MASKS[self.hero_hand.0] | CARD_MASKS[self.hero_hand.1];

            let hero_seen = runout.seen;
            let public_seen = runout.seen & !hero_mask;

            root.playout_from_root_with_runout(&mut rng, self.hero_hand, runout.board, public_seen, hero_seen);
        }

        root
    }

    fn hand_policies_for<const N: usize>(
        &self,
        base_root: &Node,
        get_probs: impl Fn(&Actions) -> [f64; N],
    ) -> [[[f64; N]; Card::NUM]; Card::NUM] {
        let mut rng = XorShiftU64::new();

        let acting_range = match self.turn {
            Position::SmallBlind => self.sb_range,
            Position::BigBlind => self.bb_range,
        };

        let mut policies = [[[0.0; N]; Card::NUM]; Card::NUM];

        let mut public_state = *self;
        public_state.remove_hero_hand();

        let mut hands = Vec::new();

        for c1 in CARDS {
            for c2 in CARDS {
                if c1 <= c2
                    || acting_range.probs[c1][c2] == 0.0
                    || public_state.seen & CARD_MASKS[c1] != 0
                    || public_state.seen & CARD_MASKS[c2] != 0
                {
                    continue;
                }

                hands.push((c1, c2));
            }
        }

        for batch in hands.chunks(HAND_BATCH_SIZE) {
            let mut roots = batch.iter().copied().map(|hand| (hand, base_root.clone(), 0usize)).collect::<Vec<_>>();

            while roots.iter().any(|(_, _, count)| *count < NUM_PLAYOUTS) {
                let mut runout = public_state.gen_runout();
                let public_seen = runout.seen;

                runout.sb_range.update_from_seen(public_seen);
                runout.bb_range.update_from_seen(public_seen);

                let cache = ScoreCache::from_board(runout.board);
                let mut terminal_equities = Vec::new();

                for (hand, root, count) in &mut roots {
                    if *count >= NUM_PLAYOUTS {
                        continue;
                    }

                    let hand_mask = CARD_MASKS[hand.0] | CARD_MASKS[hand.1];

                    if public_seen & hand_mask != 0 {
                        continue;
                    }

                    let (choices, result, outcome) = root.playout(&mut rng);

                    let (hero_equity, range_equity) = if matches!(outcome, Outcome::Showdown) {
                        let cache_idx = terminal_equities.iter().position(|(path, _, _)| path == &choices);

                        let cache_idx = match cache_idx {
                            Some(i) => i,

                            None => {
                                let terminal = base_root.node_at_path(&choices);

                                let (equities, range_equity) =
                                    terminal.equity_table_for_runout(self.turn, public_seen, &cache);

                                terminal_equities.push((choices.clone(), equities, range_equity));
                                terminal_equities.len() - 1
                            }
                        };

                        let (_, equities, range_equity) = &terminal_equities[cache_idx];

                        (equities[hand.0][hand.1], *range_equity)
                    } else {
                        (0.0, 0.0)
                    };

                    root.apply_playout_result(choices, result, outcome, hero_equity, range_equity);

                    *count += 1;
                }
            }

            for (hand, root, _) in roots {
                policies[hand.0][hand.1] = get_probs(root.actions.as_ref().unwrap());
            }
        }

        policies
    }

    fn hand_policies_from_root(&self, root: &Node) -> HandPolicies {
        match root.node_type {
            NodeType::EvenNode => HandPolicies::Even(self.hand_policies_for::<5>(root, |actions| match actions {
                Actions::Even(actions) => actions.probs,
                Actions::Behind(_) => unreachable!(),
            })),

            NodeType::BehindNode => HandPolicies::Behind(self.hand_policies_for::<4>(root, |actions| match actions {
                Actions::Behind(actions) => actions.probs,
                Actions::Even(_) => unreachable!(),
            })),

            NodeType::AheadNode => unreachable!(),
        }
    }

    fn hand_policies(&self) -> HandPolicies {
        let node_type = if self.chip_state.sb_this_street == self.chip_state.bb_this_street {
            NodeType::EvenNode
        } else {
            NodeType::BehindNode
        };

        let mut root = Node::from(self.chip_state, self.turn, node_type, false, None, self.sb_range, self.bb_range);

        root.gen_subtree();

        self.hand_policies_from_root(&root)
    }

    pub fn update_ranges_with_decision(&mut self, decision_idx: usize) {
        let policies = self.hand_policies();
        self.update_ranges_with_policies(decision_idx, &policies);
    }

    pub fn update_ranges_with_policies(&mut self, decision_idx: usize, policies: &HandPolicies) {
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

                let likelihood = match &policies {
                    HandPolicies::Even(policies) => policies[c1][c2][decision_idx],
                    HandPolicies::Behind(policies) => policies[c1][c2][decision_idx],
                };

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
    fn same_successor(a: &Node, b: &Node) -> bool {
        a.terminal == b.terminal
            && a.outcome == b.outcome
            && a.chip_state.pot == b.chip_state.pot
            && a.chip_state.sb_stack == b.chip_state.sb_stack
            && a.chip_state.bb_stack == b.chip_state.bb_stack
            && a.chip_state.sb_this_street == b.chip_state.sb_this_street
            && a.chip_state.bb_this_street == b.chip_state.bb_this_street
            && a.chip_state.max_bet == b.chip_state.max_bet
            && (a.terminal || (a.position == b.position && a.node_type == b.node_type))
    }

    fn distinct_actions<const N: usize>(children: &[Box<Node>; N]) -> [bool; N] {
        let mut legal = [true; N];

        for i in 1..N {
            for j in 0..i {
                if legal[j] && Self::same_successor(children[i].as_ref(), children[j].as_ref()) {
                    legal[i] = false;
                    break;
                }
            }
        }

        legal
    }

    fn equities_for_runout(
        &self,
        root_position: Position,
        hero_hand: Hand,
        public_seen: u64,
        hero_seen: u64,
        cache: &ScoreCache,
    ) -> (f64, f64) {
        let mut sb_range = self.sb_range;
        let mut bb_range = self.bb_range;

        match root_position {
            Position::SmallBlind => {
                sb_range.update_from_seen(public_seen);
                bb_range.update_from_seen(hero_seen);

                let equities = bb_range.equity_table(cache);

                (equities[hero_hand.0][hero_hand.1], bb_range.equity_against_with_range(sb_range, &equities))
            }

            Position::BigBlind => {
                sb_range.update_from_seen(hero_seen);
                bb_range.update_from_seen(public_seen);

                let equities = sb_range.equity_table(cache);

                (equities[hero_hand.0][hero_hand.1], sb_range.equity_against_with_range(bb_range, &equities))
            }
        }
    }

    fn equity_table_for_runout(
        &self,
        root_position: Position,
        public_seen: u64,
        cache: &ScoreCache,
    ) -> ([[f64; Card::NUM]; Card::NUM], f64) {
        let mut sb_range = self.sb_range;
        let mut bb_range = self.bb_range;

        sb_range.update_from_seen(public_seen);
        bb_range.update_from_seen(public_seen);

        match root_position {
            Position::SmallBlind => {
                let equities = bb_range.equity_table(cache);
                let range_equity = bb_range.equity_against_with_range(sb_range, &equities);

                (equities, range_equity)
            }

            Position::BigBlind => {
                let equities = sb_range.equity_table(cache);
                let range_equity = sb_range.equity_against_with_range(bb_range, &equities);

                (equities, range_equity)
            }
        }
    }

    fn playout_from_root_with_runout(
        &mut self,
        rng: &mut XorShiftU64,
        hero_hand: Hand,
        board: CardSet,
        public_seen: u64,
        hero_seen: u64,
    ) {
        let (choices, result, outcome) = self.playout(rng);

        let (hero_equity, range_equity) = if matches!(outcome, Outcome::Showdown) {
            let cache = ScoreCache::from_board(board);
            let terminal = self.node_at_path(&choices);

            terminal.equities_for_runout(self.position, hero_hand, public_seen, hero_seen, &cache)
        } else {
            (0.0, 0.0)
        };

        self.apply_playout_result(choices, result, outcome, hero_equity, range_equity);
    }

    fn apply_playout_result(
        &mut self,
        choices: Vec<usize>,
        result: ChipState,
        outcome: Outcome,
        hero_equity: f64,
        range_equity: f64,
    ) {
        let root_position = self.position;

        let starting_stack = match root_position {
            Position::SmallBlind => self.chip_state.sb_stack as f64,
            Position::BigBlind => self.chip_state.bb_stack as f64,
        };

        let final_stack_hero_pov = match (root_position, outcome) {
            (Position::SmallBlind, Outcome::Showdown) => result.sb_stack as f64 + result.pot as f64 * hero_equity,
            (Position::BigBlind, Outcome::Showdown) => result.bb_stack as f64 + result.pot as f64 * hero_equity,
            (Position::SmallBlind, Outcome::BBFolded) => result.sb_stack as f64 + result.pot as f64,
            (Position::BigBlind, Outcome::BBFolded) => result.bb_stack as f64,
            (Position::SmallBlind, Outcome::SBFolded) => result.sb_stack as f64,
            (Position::BigBlind, Outcome::SBFolded) => result.bb_stack as f64 + result.pot as f64,
        };

        let hero_pov_ev = final_stack_hero_pov - starting_stack;

        let villain_starting_stack = match root_position {
            Position::SmallBlind => self.chip_state.bb_stack as f64,
            Position::BigBlind => self.chip_state.sb_stack as f64,
        };

        let villain_final_stack = match (root_position, outcome) {
            (Position::SmallBlind, Outcome::Showdown) => {
                result.bb_stack as f64 + result.pot as f64 * (1.0 - range_equity)
            }
            (Position::BigBlind, Outcome::Showdown) => {
                result.sb_stack as f64 + result.pot as f64 * (1.0 - range_equity)
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
                    update_probs(
                        &mut actions.probs,
                        &mut actions.total_ev,
                        &mut actions.visits,
                        &actions.legal,
                        choice,
                        player_ev,
                    );
                    actions.children.as_mut().unwrap()[choice].as_mut()
                }

                Actions::Behind(actions) => {
                    update_probs(
                        &mut actions.probs,
                        &mut actions.total_ev,
                        &mut actions.visits,
                        &actions.legal,
                        choice,
                        player_ev,
                    );
                    actions.children.as_mut().unwrap()[choice].as_mut()
                }
            };

            node = next_node;
        }
    }

    fn update_subtree_ranges(&mut self, board: CardSet, seen: u64, hero_hand: Hand, board_len: u8) {
        if self.terminal {
            return;
        }

        let state = GameState {
            chip_state: self.chip_state,
            turn: self.position,
            board,
            hero_hand,
            sb_range: self.sb_range,
            bb_range: self.bb_range,
            seen,
            board_len,
        };

        let policies = state.hand_policies_from_root(self);

        match self.actions.as_mut().unwrap() {
            Actions::Even(actions) => {
                let children = actions.children.as_mut().unwrap();

                for decision_idx in 0..5 {
                    if !actions.legal[decision_idx] {
                        continue;
                    }

                    let mut child_state = state;
                    child_state.update_ranges_with_policies(decision_idx, &policies);

                    let child = children[decision_idx].as_mut();
                    child.sb_range = child_state.sb_range;
                    child.bb_range = child_state.bb_range;

                    child.update_subtree_ranges(board, seen, hero_hand, board_len);
                }
            }

            Actions::Behind(actions) => {
                let children = actions.children.as_mut().unwrap();

                for decision_idx in 0..4 {
                    if !actions.legal[decision_idx] {
                        continue;
                    }

                    let mut child_state = state;
                    child_state.update_ranges_with_policies(decision_idx, &policies);

                    let child = children[decision_idx].as_mut();
                    child.sb_range = child_state.sb_range;
                    child.bb_range = child_state.bb_range;

                    child.update_subtree_ranges(board, seen, hero_hand, board_len);
                }
            }
        }
    }

    fn node_at_path(&self, choices: &[usize]) -> &Node {
        let mut node = self;

        for &choice in choices {
            node = match node.actions.as_ref().unwrap() {
                Actions::Even(actions) => actions.children.as_ref().unwrap()[choice].as_ref(),
                Actions::Behind(actions) => actions.children.as_ref().unwrap()[choice].as_ref(),
            };
        }

        node
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
                let choice = rng.explore_action(&a.probs, &a.legal);

                let (choices, chip_state, outcome) = a.children.as_ref().unwrap()[choice].playout(rng);

                let mut path = Vec::with_capacity(choices.len() + 1);
                path.push(choice);
                path.extend(choices);

                (path, chip_state, outcome)
            }

            Actions::Behind(a) => {
                let choice = rng.explore_action(&a.probs, &a.legal);

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
                let children =
                    (0..4).map(|i| Box::new(self.successor(i).unwrap())).collect::<Vec<_>>().try_into().unwrap();

                let mut actions = BehindActions::BLANK;
                actions.legal = Self::distinct_actions(&children);
                actions.probs = softmax(&actions.total_ev, &actions.legal);
                actions.children = Some(children);

                self.actions = Some(Actions::Behind(actions));
            }
            NodeType::EvenNode => {
                let children =
                    (0..5).map(|i| Box::new(self.successor(i).unwrap())).collect::<Vec<_>>().try_into().unwrap();

                let mut actions = EvenActions::BLANK;
                actions.legal = Self::distinct_actions(&children);
                actions.probs = softmax(&actions.total_ev, &actions.legal);
                actions.children = Some(children);

                self.actions = Some(Actions::Even(actions));
            }
            _ => unreachable!(),
        }
    }
}
