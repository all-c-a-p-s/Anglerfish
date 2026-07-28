use crate::game::{
    Bet, CARD_MASKS, CARDS, Card, CardSet, ChipState, GameState, Hand, Outcome, Position, Rank, Suit, cfor,
};
use crate::hash::EquivalenceHash;
use crate::rng::XorShiftU64;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering::Relaxed};

/// Big search file!!!
/// Generally, there are two parts:
///
/// (1) MCTS-ish search of game tree:
/// There are several Monte-Carlo playouts. In each one:
/// - each side chooses a random set of actions through the game tree before seeing any of the
///   future cards.
/// - we generate a runout of cards, and use ranges (see below) to adjudicate showdown
/// - we update action probabilities based on the outcome
/// The idea is that over several playouts, the actions probabilities will approach the optimal
/// probabilities over the average runout.
///
/// This essentially gives us a function mapping a hand/board situation to a probability
/// distribution of actions (call this F). Specifically
///
/// F: (hand, board, ranges) -> distribution
///
/// (2) Ranging:
/// But how can we actually estimate which hands our opponent is likely to have in a given showdown
/// situation, given the actions they performed? Bayes theorem to the rescue!
///
/// H_i := have hand i, A := these actions
/// P(H_i | A) = P(H_i n A) / P(A)
///
/// Note that we can get P(A | H_i) from F, P(H_i) is the prior probability of having this hand
/// do P(A) = SUM over all H_j[ P(A | H_j) * P(H_j) ]
/// P(H_i n A) = P(A | H_i) * P(H_i)
///
/// so we can now update the probability of a player holding a hand based on their actions.
///
/// Since these ranges actually depend on each other (e.g. if your opponents range to call your bet
/// is strong, your betting range needs to be strong/polar as well), we run several ranging passes.
/// Each pass runs F on the range estimates from the previous pass.

/// One node in the future game tree
#[derive(Debug, Clone)]
pub struct Node {
    pub terminal: bool,
    pub position: Position,
    pub node_type: NodeType,
    pub chip_state: ChipState,
    pub actions: Option<Actions>,
    pub outcome: Option<Outcome>,
    pub sb_range: Rc<Range>,
    pub bb_range: Rc<Range>,
    pub streets_remaining: u8,
    pub actions_this_street: u8,
    pub value: NodeValue,
}

/// Similar to the concept of a "range" that humans use when playing poker.
/// We assign a probability to each hand a player might hold.
#[derive(Debug, Clone, Copy)]
pub struct Range {
    pub probs: [[f64; Card::NUM]; Card::NUM],
}

/// Cache storing all showdown information used for constructing equity table.
pub struct ScoreCache {
    pub scores: [[Option<i32>; Card::NUM]; Card::NUM],
    sorted_hands: Vec<Hand>,

    /// For each card, sorted_hands indices of the 51 hands containing it.
    hands_by_card: [Vec<usize>; Card::NUM],

    /// Map an exact hand to its index in sorted_hands.
    sorted_index: [[u16; Card::NUM]; Card::NUM],

    /// Score-group boundaries for each sorted hand.
    first_equal: [u16; HAND_COUNT],
    first_greater: [u16; HAND_COUNT],

    /// For each card and hero-hand index:
    /// how many hands containing that card occur before the boundary.
    blocked_before_equal: [Vec<u8>; Card::NUM],
    blocked_before_greater: [Vec<u8>; Card::NUM],
}

/// Single runout will be used for a whole batch of cards,
/// so they can share ScoreCache.
struct CachedRunout {
    public_seen: u64,
    cache: ScoreCache,
}

impl ScoreCache {
    pub fn from_board(board: CardSet) -> Self {
        let mut scores = [[None; Card::NUM]; Card::NUM];

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

                scores[c1][c2] = Some(score);
                scores[c2][c1] = Some(score);

                sorted_hands.push((c1, c2));
            }
        }

        sorted_hands.sort_unstable_by_key(|&(c1, c2)| scores[c1][c2].unwrap());

        let mut sorted_index = [[0; Card::NUM]; Card::NUM];

        let mut hands_by_card = std::array::from_fn(|_| Vec::with_capacity(Card::NUM - 1));

        for (index, &(c1, c2)) in sorted_hands.iter().enumerate() {
            sorted_index[c1][c2] = index as u16;
            sorted_index[c2][c1] = index as u16;

            hands_by_card[c1].push(index);
            hands_by_card[c2].push(index);
        }

        let mut first_equal = [0; HAND_COUNT];
        let mut first_greater = [0; HAND_COUNT];

        let mut start = 0;

        while start < HAND_COUNT {
            let (c1, c2) = sorted_hands[start];
            let score = scores[c1][c2].unwrap();

            let mut end = start + 1;

            while end < HAND_COUNT {
                let (c1, c2) = sorted_hands[end];

                if scores[c1][c2].unwrap() != score {
                    break;
                }

                end += 1;
            }

            for index in start..end {
                first_equal[index] = start as u16;
                first_greater[index] = end as u16;
            }

            start = end;
        }

        let mut blocked_before_equal = std::array::from_fn(|_| vec![0; HAND_COUNT]);
        let mut blocked_before_greater = std::array::from_fn(|_| vec![0; HAND_COUNT]);

        for card in CARDS {
            let blocked_indices = &hands_by_card[card];

            for hand_index in 0..HAND_COUNT {
                let equal_boundary = first_equal[hand_index] as usize;
                let greater_boundary = first_greater[hand_index] as usize;

                blocked_before_equal[card][hand_index] =
                    blocked_indices.partition_point(|&index| index < equal_boundary) as u8;
                blocked_before_greater[card][hand_index] =
                    blocked_indices.partition_point(|&index| index < greater_boundary) as u8;
            }
        }

        Self {
            scores,
            sorted_hands,
            hands_by_card,
            sorted_index,
            first_equal,
            first_greater,
            blocked_before_equal,
            blocked_before_greater,
        }
    }
}

impl Range {
    const fn no_information() -> Self {
        let cnt = 26.0 * 51.0;
        let mut probs = [[1.0 / cnt; Card::NUM]; Card::NUM];

        cfor!(let mut i = 0; i < Card::NUM; i += 1;{
            probs[i][i] = 0.0;
        });

        Self { probs }
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

    /// Gets equity of each possible hero hand against this range.
    fn equity_table(&self, cache: &ScoreCache) -> [[f64; Card::NUM]; Card::NUM] {
        let mut equities = [[0.0; Card::NUM]; Card::NUM];

        let mut pref = [0.0; HAND_COUNT + 1];

        for (i, &(c1, c2)) in cache.sorted_hands.iter().enumerate() {
            pref[i + 1] = pref[i] + self.probs[c1][c2];
        }

        let total_p = pref[HAND_COUNT];

        let blocker_pref = std::array::from_fn(|card_index| {
            let indices = &cache.hands_by_card[card_index];

            let mut pref = Vec::with_capacity(indices.len() + 1);

            pref.push(0.0);

            let mut total = 0.0;

            for &hand_index in indices {
                let (c1, c2) = cache.sorted_hands[hand_index];

                total += self.probs[c1][c2];
                pref.push(total);
            }

            pref
        });

        for our_c1 in CARDS {
            for our_c2 in CARDS {
                if our_c1 <= our_c2 {
                    continue;
                }

                let hand_index = cache.sorted_index[our_c1][our_c2] as usize;
                let first_equal = cache.first_equal[hand_index] as usize;
                let first_greater = cache.first_greater[hand_index] as usize;

                let c1_before_equal = cache.blocked_before_equal[our_c1][hand_index] as usize;
                let c2_before_equal = cache.blocked_before_equal[our_c2][hand_index] as usize;
                let c1_before_greater = cache.blocked_before_greater[our_c1][hand_index] as usize;
                let c2_before_greater = cache.blocked_before_greater[our_c2][hand_index] as usize;

                let self_p = self.probs[our_c1][our_c2];

                let blocked_total = blocker_pref[our_c1].last().copied().unwrap()
                    + blocker_pref[our_c2].last().copied().unwrap()
                    - self_p;

                let blocked_win = blocker_pref[our_c1][c1_before_equal] + blocker_pref[our_c2][c2_before_equal];

                let blocked_win_or_tie =
                    blocker_pref[our_c1][c1_before_greater] + blocker_pref[our_c2][c2_before_greater] - self_p;

                let blocked_tie = blocked_win_or_tie - blocked_win;

                let valid_p = total_p - blocked_total;
                let win_p = pref[first_equal] - blocked_win;
                let tie_p = (pref[first_greater] - pref[first_equal]) - blocked_tie;

                // won't matter if it's zero anyway, just don't wanna crash
                let equity = if valid_p == 0.0 { 0.5 } else { (win_p + 0.5 * tie_p) / valid_p };

                equities[our_c1][our_c2] = equity;
                equities[our_c2][our_c1] = equity;
            }
        }

        equities
    }

    /// Gets equity of another range against this one.
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
    pub legal: [bool; 5],
    pub children: Option<[Box<Node>; 5]>,
}

impl EvenActions {
    pub const BLANK: Self = Self { probs: [0.2; 5], legal: [true; 5], children: None };
}

/// (2) Opponent's betting lead:
/// - fold
/// - call
/// - 2.5x re-raise
/// - all in
#[derive(Debug, Clone)]
pub struct BehindActions {
    pub probs: [f64; 4],
    pub legal: [bool; 4],
    pub children: Option<[Box<Node>; 4]>,
}

impl BehindActions {
    pub const BLANK: Self = Self { probs: [0.25; 4], legal: [true; 4], children: None };
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

#[derive(Debug, Clone, Copy, Default)]
pub struct NodeValue {
    pub hero_ev: f64,
    pub villain_ev: f64,
    pub visits: u32,
}

impl NodeValue {
    fn sample(hero_ev: f64, villain_ev: f64) -> Self {
        Self { hero_ev, villain_ev, visits: 1 }
    }

    fn initialized(self) -> bool {
        self.visits > 0
    }

    fn observe(&mut self, hero_ev: f64, villain_ev: f64) {
        self.visits += 1;
        let n = self.visits as f64;

        self.hero_ev += (hero_ev - self.hero_ev) / n;
        self.villain_ev += (villain_ev - self.villain_ev) / n;
    }

    fn for_player(self, node_position: Position, root_position: Position) -> f64 {
        if node_position == root_position { self.hero_ev } else { self.villain_ev }
    }
}

fn weighted_child_value<const N: usize>(
    probs: &[f64; N],
    legal: &[bool; N],
    mut child_value: impl FnMut(usize) -> NodeValue,
) -> NodeValue {
    let mut value = NodeValue::default();

    for i in 0..N {
        if !legal[i] {
            continue;
        }

        let child = child_value(i);
        value.hero_ev += probs[i] * child.hero_ev;
        value.villain_ev += probs[i] * child.villain_ev;
    }

    value
}

static POLICY_TEMPERATURE: AtomicI32 = AtomicI32::new(10_000);

static RANGE_TEMPERATURE: AtomicI32 = AtomicI32::new(1_000);

const TEMP_SCALE: f64 = 1_000.0;

pub fn set_policy_temperature(k: f64) {
    let scaled = (k * TEMP_SCALE).round() as i32;
    POLICY_TEMPERATURE.store(scaled, Relaxed);
}

pub fn policy_temperature() -> f64 {
    POLICY_TEMPERATURE.load(Relaxed) as f64 / TEMP_SCALE
}

pub fn set_range_temperature(k: f64) {
    let scaled = (k * TEMP_SCALE).round() as i32;
    RANGE_TEMPERATURE.store(scaled, Relaxed);
}

pub fn range_temperature() -> f64 {
    RANGE_TEMPERATURE.load(Relaxed) as f64 / TEMP_SCALE
}

fn update_probs<const N: usize, const RANGING: bool>(probs: &mut [f64; N], legal: &[bool; N], child_evs: &[f64; N]) {
    let temperature = if RANGING { range_temperature() } else { policy_temperature() };

    let max_ev = (0..N).filter(|&i| legal[i]).map(|i| child_evs[i]).fold(f64::NEG_INFINITY, f64::max);

    let mut total = 0.0;

    for i in 0..N {
        if legal[i] {
            probs[i] = ((child_evs[i] - max_ev) / temperature).exp();
            total += probs[i];
        } else {
            probs[i] = 0.0;
        }
    }

    for p in probs {
        *p /= total;
    }
}

pub enum HandPolicies {
    Even([[[f64; 5]; Card::NUM]; Card::NUM]),
    Behind([[[f64; 4]; Card::NUM]; Card::NUM]),
}

/// Stores the private policy and cached value of one node for one possible hand.
#[derive(Clone, Copy)]
enum NodePolicyStats {
    Even { probs: [f64; 5], value: NodeValue },
    Behind { probs: [f64; 4], value: NodeValue },
    Terminal { value: NodeValue },
}

impl NodePolicyStats {
    fn value(self) -> NodeValue {
        match self {
            Self::Even { value, .. } | Self::Behind { value, .. } | Self::Terminal { value } => value,
        }
    }
}

/// For any hand, caches NodePolicyState for each node in the game tree.
#[derive(Default)]
struct HandPolicyState {
    stats: HashMap<usize, NodePolicyStats>,
}

impl HandPolicyState {
    fn even_probs(&self, node: &Node) -> [f64; 5] {
        match self.stats.get(&Node::policy_key(node)) {
            Some(NodePolicyStats::Even { probs, .. }) => *probs,
            Some(NodePolicyStats::Behind { .. } | NodePolicyStats::Terminal { .. }) => unreachable!(),
            None => match node.actions.as_ref().unwrap() {
                Actions::Even(actions) => actions.probs,
                Actions::Behind(_) => unreachable!(),
            },
        }
    }

    fn behind_probs(&self, node: &Node) -> [f64; 4] {
        match self.stats.get(&Node::policy_key(node)) {
            Some(NodePolicyStats::Behind { probs, .. }) => *probs,
            Some(NodePolicyStats::Even { .. } | NodePolicyStats::Terminal { .. }) => unreachable!(),
            None => match node.actions.as_ref().unwrap() {
                Actions::Behind(actions) => actions.probs,
                Actions::Even(_) => unreachable!(),
            },
        }
    }

    fn value(&self, node: &Node) -> Option<NodeValue> {
        self.stats.get(&Node::policy_key(node)).copied().map(NodePolicyStats::value)
    }
}

const RANGE_UPDATE_DEPTH: usize = 2;

const NUM_PLAYOUTS: usize = 1024;
const HAND_COUNT: usize = Card::NUM * (Card::NUM - 1) / 2;
const HAND_BATCH_SIZE: usize = HAND_COUNT;
const NUM_RANGE_PASSES: usize = 4;

pub fn inspect_range_summary(name: &str, state: &GameState, range: &Range) {
    const NUM_SHOWN: usize = 10;

    let uniform_p = 1.0 / HAND_COUNT as f64;

    let aa = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::Ace, Suit::Hearts));
    let aks = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Spades));
    let ako = (Card::new(Rank::Ace, Suit::Spades), Card::new(Rank::King, Suit::Hearts));
    let tt = (Card::new(Rank::Ten, Suit::Spades), Card::new(Rank::Ten, Suit::Hearts));
    let dd = (Card::new(Rank::Two, Suit::Spades), Card::new(Rank::Two, Suit::Hearts));
    let sdo = (Card::new(Rank::Seven, Suit::Spades), Card::new(Rank::Two, Suit::Hearts));
    let t9s = (Card::new(Rank::Ten, Suit::Spades), Card::new(Rank::Nine, Suit::Spades));

    let interesting_hands =
        [("AA", aa), ("AKs", aks), ("AKo", ako), ("TT", tt), ("22", dd), ("72o", sdo), ("T9s", t9s)];

    let mut combos = vec![];
    let mut entropy = 0.0;
    let mut total_p = 0.0;
    let mut nonzero = 0;

    for c1 in CARDS {
        for c2 in CARDS {
            if c1 <= c2 {
                continue;
            }

            let p = range.probs[c1][c2];

            if p > 0.0 {
                nonzero += 1;
                total_p += p;
                entropy -= p * p.ln();

                combos.push(((c1, c2), p));
            }
        }
    }

    combos.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mass_in_top = |n: usize| -> f64 { combos.iter().take(n).map(|(_, p)| p).sum() };

    let equity_vs_random = state.range_equity_vs_random(*range, 1024);

    println!("\n========== {name} ==========");
    println!("Total p:       {total_p:.10}");
    println!("Nonzero combos:    {nonzero}");
    println!("Effective combos:  {:.2}", entropy.exp());
    println!("Equity vs random hand:    {:.2}%", 100.0 * equity_vs_random,);
    println!("Top 10 mass:              {:.2}%", 100.0 * mass_in_top(10),);
    println!("Top 50 mass:              {:.2}%", 100.0 * mass_in_top(50),);
    println!("Top 100 mass:             {:.2}%", 100.0 * mass_in_top(100),);

    println!("\nSelected hands:");

    for &(label, hand) in &interesting_hands {
        let (c1, c2) = if hand.0 > hand.1 { (hand.0, hand.1) } else { (hand.1, hand.0) };

        let p = range.probs[c1][c2];

        println!("{label:>4}: {p:.10} ({:>6.3}x uniform)", p / uniform_p,);
    }

    println!("\nMost common hands:");

    for (rank, &((c1, c2), p)) in combos.iter().take(NUM_SHOWN).enumerate() {
        println!("{:>2}. {:?} {:?}: {:.10} ({:.3}x uniform)", rank + 1, c1, c2, p, p / uniform_p,);
    }

    println!("\nLeast common nonzero hands:");

    for (rank, &((c1, c2), p)) in combos.iter().rev().take(NUM_SHOWN).enumerate() {
        println!("{:>2}. {:?} {:?}: {:.10} ({:.3}x uniform)", rank + 1, c1, c2, p, p / uniform_p,);
    }
}

impl GameState {
    fn actions_this_street(&self) -> u8 {
        if self.board_len == 0
            && self.turn == Position::SmallBlind
            && self.chip_state.sb_this_street == 1
            && self.chip_state.bb_this_street == 2
        {
            0
        } else if self.chip_state.sb_this_street != self.chip_state.bb_this_street {
            1
        } else {
            match (self.board_len, self.turn) {
                (0, Position::BigBlind) => 1,
                (0, Position::SmallBlind) => 0,
                (_, Position::SmallBlind) => 1,
                (_, Position::BigBlind) => 0,
            }
        }
    }

    /// Pre-generates CachedRunouts to use for evaluating hands.
    fn cached_runouts(&self, count: usize) -> Vec<CachedRunout> {
        let mut public_state = *self;
        public_state.remove_hero_hand();

        (0..count)
            .map(|_| {
                let runout = public_state.gen_runout();

                CachedRunout { public_seen: runout.seen, cache: ScoreCache::from_board(runout.board) }
            })
            .collect()
    }

    /// Builds tree of ranges after various actions.
    pub fn build_ranged_tree(&self) -> Node {
        let node_type = if self.chip_state.sb_this_street == self.chip_state.bb_this_street {
            NodeType::EvenNode
        } else {
            NodeType::BehindNode
        };

        let mut root = Node::from(
            self.chip_state,
            self.turn,
            node_type,
            false,
            None,
            Rc::new(self.sb_range),
            Rc::new(self.bb_range),
            self.streets_remaining(),
            self.actions_this_street(),
        );

        root.gen_subtree();

        println!("INFO generated game tree");
        println!("INFO node count {}", root.node_count());

        let cached_runouts = self.cached_runouts(NUM_PLAYOUTS);

        for pass_idx in 0..NUM_RANGE_PASSES {
            root.update_subtree_ranges(
                self.board,
                self.seen,
                self.hero_hand,
                self.board_len,
                RANGE_UPDATE_DEPTH,
                &cached_runouts,
            );

            println!("INFO completed range pass {}/{}", pass_idx + 1, NUM_RANGE_PASSES,);
        }

        root
    }

    /// Does runouts with the actual hero hand, using all the ranging that has been done
    /// previously.
    pub fn do_runouts(&self) -> Node {
        let mut root = self.build_ranged_tree();
        let mut rng = XorShiftU64::new();

        for _ in 0..NUM_PLAYOUTS * 10 {
            let runout = self.gen_runout();

            let hero_mask = CARD_MASKS[self.hero_hand.0] | CARD_MASKS[self.hero_hand.1];

            let hero_seen = runout.seen;
            let public_seen = runout.seen & !hero_mask;

            root.playout_from_root_with_runout(&mut rng, self.hero_hand, runout.board, public_seen, hero_seen);
        }

        root
    }

    /// Generates policy for each possible hand
    fn hand_policies_for<const N: usize>(
        &self,
        base_root: &Node,
        cached_runouts: &[CachedRunout],
        get_probs: impl Fn(&Node, &HandPolicyState) -> [f64; N],
    ) -> [[[f64; N]; Card::NUM]; Card::NUM] {
        let mut rng = XorShiftU64::new();

        let acting_range = match self.turn {
            Position::SmallBlind => self.sb_range,
            Position::BigBlind => self.bb_range,
        };

        let mut policies = [[[0.0; N]; Card::NUM]; Card::NUM];

        let mut public_state = *self;
        public_state.remove_hero_hand();

        // Each class contains:
        // - one rep hand to solve
        // - all exact hands which will receive that policy
        let mut class_indices = EquivalenceHash::<usize>::new();
        let mut classes: Vec<(Hand, Vec<Hand>)> = vec![];

        for c1 in CARDS {
            for c2 in CARDS {
                if c1 <= c2
                    || acting_range.probs[c1][c2] == 0.0
                    || public_state.seen & CARD_MASKS[c1] != 0
                    || public_state.seen & CARD_MASKS[c2] != 0
                {
                    continue;
                }

                let hand = (c1, c2);

                if let Some(class_idx) = class_indices.lookup_hand(&self.board, hand).copied() {
                    classes[class_idx].1.push(hand);
                } else {
                    let class_idx = classes.len();

                    class_indices.insert_hand(&self.board, hand, class_idx);

                    classes.push((hand, vec![hand]));
                }
            }
        }

        for batch in classes.chunks(HAND_BATCH_SIZE) {
            let mut states = batch.iter().map(|(rep, _)| (*rep, HandPolicyState::default())).collect::<Vec<_>>();

            for runout in cached_runouts {
                let public_seen = runout.public_seen;

                let mut terminal_equities: HashMap<
                    (*const Range, *const Range),
                    (Box<[[f64; Card::NUM]; Card::NUM]>, f64),
                > = HashMap::new();

                for (hand, state) in &mut states {
                    let hand_mask = CARD_MASKS[hand.0] | CARD_MASKS[hand.1];

                    if public_seen & hand_mask != 0 {
                        continue;
                    }

                    let (choices, result, outcome) = base_root.playout_with_hand_state(&mut rng, state);

                    let (hero_equity, range_equity) = if matches!(outcome, Outcome::Showdown) {
                        let terminal = base_root.node_at_path(&choices);

                        let key = (Rc::as_ptr(&terminal.sb_range), Rc::as_ptr(&terminal.bb_range));

                        if !terminal_equities.contains_key(&key) {
                            let (equities, range_equity) =
                                terminal.equity_table_for_runout(self.turn, public_seen, &runout.cache);

                            terminal_equities.insert(key, (Box::new(equities), range_equity));
                        }

                        let (equities, range_equity) = terminal_equities.get(&key).unwrap();

                        (equities[hand.0][hand.1], *range_equity)
                    } else {
                        (0.0, 0.0)
                    };

                    base_root.apply_playout_result_to_hand_state(
                        &choices,
                        state,
                        result,
                        outcome,
                        hero_equity,
                        range_equity,
                    );
                }
            }

            for ((_, members), (_, state)) in batch.iter().zip(states) {
                let probs = get_probs(base_root, &state);

                for &hand in members {
                    policies[hand.0][hand.1] = probs;
                    policies[hand.1][hand.0] = probs;
                }
            }
        }

        policies
    }

    fn hand_policies_from_root(&self, root: &Node, cached_runouts: &[CachedRunout]) -> HandPolicies {
        match root.node_type {
            NodeType::EvenNode => {
                HandPolicies::Even(
                    self.hand_policies_for::<5>(root, cached_runouts, |node, state| state.even_probs(node)),
                )
            }

            NodeType::BehindNode => {
                HandPolicies::Behind(
                    self.hand_policies_for::<4>(root, cached_runouts, |node, state| state.behind_probs(node)),
                )
            }

            NodeType::AheadNode => unreachable!(),
        }
    }

    pub fn hand_policies(&self) -> HandPolicies {
        let node_type = if self.chip_state.sb_this_street == self.chip_state.bb_this_street {
            NodeType::EvenNode
        } else {
            NodeType::BehindNode
        };

        let mut root = Node::from(
            self.chip_state,
            self.turn,
            node_type,
            false,
            None,
            Rc::new(self.sb_range),
            Rc::new(self.bb_range),
            self.streets_remaining(),
            self.actions_this_street(),
        );

        root.gen_subtree();

        let cached_runouts = self.cached_runouts(NUM_PLAYOUTS);
        self.hand_policies_from_root(&root, &cached_runouts)
    }

    /// Update ranges based on decision, when we don't have policies.
    pub fn update_ranges_with_decision(&mut self, decision_idx: usize) {
        let policies = self.hand_policies();
        self.update_ranges_with_policies(decision_idx, &policies);
    }

    /// Update ranges based on decision, when we already have policies.
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

    /// Estimate equity of a hand vs any two cards (ATC).
    /// Useful diagnostic to see how strong a range is.
    pub fn range_equity_vs_random(&self, range: Range, num_runouts: usize) -> f64 {
        let mut public_state = *self;
        public_state.remove_hero_hand();

        let mut equity_sum = 0.0;

        for _ in 0..num_runouts {
            let runout = public_state.gen_runout();
            let public_seen = runout.seen;

            let mut our_range = range;
            let mut random_range = Range::BLANK;

            our_range.update_from_seen(public_seen);
            random_range.update_from_seen(public_seen);

            let cache = ScoreCache::from_board(runout.board);

            let equities = random_range.equity_table(&cache);

            let range_equity = random_range.equity_against_with_range(our_range, &equities);

            equity_sum += range_equity;
        }

        equity_sum / num_runouts as f64
    }
}

impl Node {
    /// Check whether two actions lead to an idential outcome (e.g. calling all in or trying to
    /// "raise" a bet that already puts you all in).
    fn same_successor(a: &Node, b: &Node) -> bool {
        a.terminal == b.terminal
            && a.outcome == b.outcome
            && a.chip_state.pot == b.chip_state.pot
            && a.chip_state.sb_stack == b.chip_state.sb_stack
            && a.chip_state.bb_stack == b.chip_state.bb_stack
            && a.chip_state.sb_this_street == b.chip_state.sb_this_street
            && a.chip_state.bb_this_street == b.chip_state.bb_this_street
            && a.chip_state.max_bet == b.chip_state.max_bet
            && (a.terminal
                || (a.position == b.position
                    && a.node_type == b.node_type
                    && a.streets_remaining == b.streets_remaining
                    && a.actions_this_street == b.actions_this_street))
    }

    pub fn node_count(&self) -> usize {
        let children = match self.actions.as_ref() {
            None => 0,

            Some(Actions::Even(actions)) => actions
                .children
                .as_ref()
                .unwrap()
                .iter()
                .zip(actions.legal)
                .filter(|(_, legal)| *legal)
                .map(|(child, _)| child.node_count())
                .sum(),

            Some(Actions::Behind(actions)) => actions
                .children
                .as_ref()
                .unwrap()
                .iter()
                .zip(actions.legal)
                .filter(|(_, legal)| *legal)
                .map(|(child, _)| child.node_count())
                .sum(),
        };

        1 + children
    }

    fn legal_actions<const N: usize>(&self, children: &[Box<Node>; N]) -> [bool; N] {
        let mut legal = [true; N];

        let (us, them) = if self.position == Position::SmallBlind {
            (self.chip_state.sb_stack, self.chip_state.bb_stack)
        } else {
            (self.chip_state.bb_stack, self.chip_state.sb_stack)
        };

        let behind = N == 4;

        for i in 1..N {
            for j in 0..i {
                if legal[j] && Self::same_successor(children[i].as_ref(), children[j].as_ref()) {
                    legal[i] = false;
                    break;
                }
            }

            // can't fold after going all in
            if behind && i == 0 && us == 0 {
                legal[i] = false;
            }

            // can't raise when opponent is all in
            if behind && them == 0 && i >= 2 {
                legal[i] = false;
            }
        }

        legal
    }

    /// Computes hero hand equity vs villain range, hero range (villain's perception or hero range)
    /// equity vs villain range.
    fn equities_for_runout(
        &self,
        root_position: Position,
        hero_hand: Hand,
        public_seen: u64,
        hero_seen: u64,
        cache: &ScoreCache,
    ) -> (f64, f64) {
        let mut sb_range = *self.sb_range;
        let mut bb_range = *self.bb_range;

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

    /// Same as the above but computes for every possible hero hand.
    fn equity_table_for_runout(
        &self,
        root_position: Position,
        public_seen: u64,
        cache: &ScoreCache,
    ) -> ([[f64; Card::NUM]; Card::NUM], f64) {
        let mut sb_range = *self.sb_range;
        let mut bb_range = *self.bb_range;

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

    /// One playout, sampling from policy probabilities.
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

    fn calc_evs(&self, result: ChipState, outcome: Outcome, hero_equity: f64, range_equity: f64) -> (f64, f64) {
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

        (hero_pov_ev, villain_pov_ev)
    }

    /// Propagates a playout result from the terminal node back to the root.
    fn apply_playout_result(
        &mut self,
        choices: Vec<usize>,
        result: ChipState,
        outcome: Outcome,
        hero_equity: f64,
        range_equity: f64,
    ) {
        let root_position = self.position;
        let (hero_ev, villain_ev) = self.calc_evs(result, outcome, hero_equity, range_equity);
        let sample = NodeValue::sample(hero_ev, villain_ev);

        self.backup_playout_result(&choices, root_position, sample);
    }

    /// Update the terminal average, then recompute cached node values while unwinding.
    fn backup_playout_result(&mut self, choices: &[usize], root_position: Position, sample: NodeValue) -> NodeValue {
        if choices.is_empty() {
            self.value.observe(sample.hero_ev, sample.villain_ev);
            return self.value;
        }

        let choice = choices[0];
        let node_position = self.position;
        let previous_value = self.value;
        let fallback = if previous_value.initialized() { previous_value } else { sample };

        let mut new_value = match self.actions.as_mut().unwrap() {
            Actions::Even(actions) => {
                let children = actions.children.as_mut().unwrap();

                children[choice].backup_playout_result(&choices[1..], root_position, sample);

                let child_evs = std::array::from_fn(|i| {
                    let value = children[i].value;

                    let value = if value.initialized() { value } else { fallback };

                    value.for_player(node_position, root_position)
                });

                update_probs::<_, false>(&mut actions.probs, &actions.legal, &child_evs);

                weighted_child_value(&actions.probs, &actions.legal, |i| {
                    let value = children[i].value;

                    if value.initialized() { value } else { fallback }
                })
            }

            Actions::Behind(actions) => {
                let children = actions.children.as_mut().unwrap();

                children[choice].backup_playout_result(&choices[1..], root_position, sample);

                let child_evs = std::array::from_fn(|i| {
                    let value = children[i].value;

                    let value = if value.initialized() { value } else { fallback };

                    value.for_player(node_position, root_position)
                });

                update_probs::<_, false>(&mut actions.probs, &actions.legal, &child_evs);

                weighted_child_value(&actions.probs, &actions.legal, |i| {
                    let value = children[i].value;

                    if value.initialized() { value } else { fallback }
                })
            }
        };

        new_value.visits = previous_value.visits + 1;
        self.value = new_value;
        new_value
    }

    /// Recursively updates ranges. Basically stuff like:
    /// - if I jam now, what will they think my jamming range is?
    /// - given that, what range will they call me with?
    fn update_subtree_ranges(
        &mut self,
        board: CardSet,
        seen: u64,
        hero_hand: Hand,
        board_len: u8,
        depth: usize,
        cached_runouts: &[CachedRunout],
    ) {
        if self.terminal || depth == 0 {
            return;
        }

        let state = GameState {
            chip_state: self.chip_state,
            turn: self.position,
            board,
            hero_hand,
            sb_range: *self.sb_range,
            bb_range: *self.bb_range,
            seen,
            board_len,
        };

        let policies = state.hand_policies_from_root(self, cached_runouts);

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
                    child.sb_range = Rc::new(child_state.sb_range);
                    child.bb_range = Rc::new(child_state.bb_range);

                    child.update_subtree_ranges(board, seen, hero_hand, board_len, depth - 1, cached_runouts);
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
                    child.sb_range = Rc::new(child_state.sb_range);
                    child.bb_range = Rc::new(child_state.bb_range);

                    child.update_subtree_ranges(board, seen, hero_hand, board_len, depth - 1, cached_runouts);
                }
            }
        }
    }

    /// Finds node from sequence of choices in game tree.
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

    fn policy_key(node: &Node) -> usize {
        node as *const Node as usize
    }

    /// Playout with cached info about hand policy at various visited nodes.
    fn playout_with_hand_state(
        &self,
        rng: &mut XorShiftU64,
        state: &HandPolicyState,
    ) -> (Vec<usize>, ChipState, Outcome) {
        let mut choices = vec![];
        let mut node = self;

        loop {
            if node.terminal {
                return (choices, node.chip_state, node.outcome.unwrap());
            }

            let choice = match node.actions.as_ref().unwrap() {
                Actions::Even(actions) => {
                    let probs = state.even_probs(node);
                    rng.explore_action(&probs, &actions.legal)
                }
                Actions::Behind(actions) => {
                    let probs = state.behind_probs(node);
                    rng.explore_action(&probs, &actions.legal)
                }
            };

            choices.push(choice);

            node = match node.actions.as_ref().unwrap() {
                Actions::Even(actions) => actions.children.as_ref().unwrap()[choice].as_ref(),
                Actions::Behind(actions) => actions.children.as_ref().unwrap()[choice].as_ref(),
            };
        }
    }

    /// Update HandPolicyState based on a playout outcome.
    fn apply_playout_result_to_hand_state(
        &self,
        choices: &[usize],
        state: &mut HandPolicyState,
        result: ChipState,
        outcome: Outcome,
        hero_equity: f64,
        range_equity: f64,
    ) {
        let root_position = self.position;
        let (hero_ev, villain_ev) = self.calc_evs(result, outcome, hero_equity, range_equity);
        let sample = NodeValue::sample(hero_ev, villain_ev);

        self.backup_playout_result_to_hand_state(choices, state, root_position, sample);
    }

    fn backup_playout_result_to_hand_state(
        &self,
        choices: &[usize],
        state: &mut HandPolicyState,
        root_position: Position,
        sample: NodeValue,
    ) {
        let key = Self::policy_key(self);

        if choices.is_empty() {
            let mut value = match state.stats.get(&key).copied() {
                Some(NodePolicyStats::Terminal { value }) => value,
                Some(NodePolicyStats::Even { .. } | NodePolicyStats::Behind { .. }) => unreachable!(),
                None => NodeValue::default(),
            };

            value.observe(sample.hero_ev, sample.villain_ev);
            state.stats.insert(key, NodePolicyStats::Terminal { value });

            return;
        }

        let choice = choices[0];
        let node_position = self.position;

        let chosen_child = match self.actions.as_ref().unwrap() {
            Actions::Even(actions) => actions.children.as_ref().unwrap()[choice].as_ref(),
            Actions::Behind(actions) => actions.children.as_ref().unwrap()[choice].as_ref(),
        };

        chosen_child.backup_playout_result_to_hand_state(&choices[1..], state, root_position, sample);

        let existing = state.stats.get(&key).copied().unwrap_or_else(|| match self.actions.as_ref().unwrap() {
            Actions::Even(actions) => NodePolicyStats::Even { probs: actions.probs, value: NodeValue::default() },

            Actions::Behind(actions) => NodePolicyStats::Behind { probs: actions.probs, value: NodeValue::default() },
        });

        let previous_value = existing.value();

        let fallback = if previous_value.initialized() { previous_value } else { sample };

        match (self.actions.as_ref().unwrap(), existing) {
            (Actions::Even(actions), NodePolicyStats::Even { mut probs, value: _ }) => {
                let children = actions.children.as_ref().unwrap();

                let child_values = std::array::from_fn(|i| state.value(children[i].as_ref()).unwrap_or(fallback));

                let child_evs = child_values.map(|value| value.for_player(node_position, root_position));

                update_probs::<_, true>(&mut probs, &actions.legal, &child_evs);

                let mut value = weighted_child_value(&probs, &actions.legal, |i| child_values[i]);

                value.visits = previous_value.visits + 1;

                state.stats.insert(key, NodePolicyStats::Even { probs, value });
            }

            (Actions::Behind(actions), NodePolicyStats::Behind { mut probs, value: _ }) => {
                let children = actions.children.as_ref().unwrap();

                let child_values = std::array::from_fn(|i| state.value(children[i].as_ref()).unwrap_or(fallback));

                let child_evs = child_values.map(|value| value.for_player(node_position, root_position));

                update_probs::<_, true>(&mut probs, &actions.legal, &child_evs);

                let mut value = weighted_child_value(&probs, &actions.legal, |i| child_values[i]);

                value.visits = previous_value.visits + 1;

                state.stats.insert(key, NodePolicyStats::Behind { probs, value });
            }

            _ => unreachable!(),
        }
    }

    /// Playout from node, sampling action probabilities.
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
        sb_range: Rc<Range>,
        bb_range: Rc<Range>,
        streets_remaining: u8,
        actions_this_street: u8,
    ) -> Self {
        let actions = match (terminal, node_type) {
            (true, _) => None,
            (_, NodeType::EvenNode) => Some(Actions::Even(EvenActions::BLANK)),
            (_, NodeType::BehindNode) => Some(Actions::Behind(BehindActions::BLANK)),
            (_, NodeType::AheadNode) => unreachable!(),
            // AheadNode can only occur after opponent folded (terminal)
        };

        Self {
            terminal,
            position,
            node_type,
            chip_state,
            actions,
            outcome,
            sb_range,
            bb_range,
            streets_remaining,
            actions_this_street,
            value: NodeValue::default(),
        }
    }

    fn close_betting_round(&self, mut chip_state: ChipState) -> Self {
        if chip_state.sb_stack == 0 || chip_state.bb_stack == 0 || self.streets_remaining == 1 {
            chip_state.refund_uncalled();

            Node::from(
                chip_state,
                self.position.next(),
                NodeType::EvenNode,
                true,
                Some(Outcome::Showdown),
                Rc::clone(&self.sb_range),
                Rc::clone(&self.bb_range),
                self.streets_remaining,
                self.actions_this_street,
            )
        } else {
            chip_state.sb_this_street = 0;
            chip_state.bb_this_street = 0;
            chip_state.max_bet = 0;

            Node::from(
                chip_state,
                Position::BigBlind,
                NodeType::EvenNode,
                false,
                None,
                Rc::clone(&self.sb_range),
                Rc::clone(&self.bb_range),
                self.streets_remaining - 1,
                0,
            )
        }
    }

    fn call_successor(&self, chip_state: ChipState) -> Self {
        if self.streets_remaining == 4 && self.position == Position::SmallBlind && self.actions_this_street == 0 {
            // SB calling BB preflop
            Node::from(
                chip_state,
                Position::BigBlind,
                NodeType::EvenNode,
                false,
                None,
                Rc::clone(&self.sb_range),
                Rc::clone(&self.bb_range),
                self.streets_remaining,
                1,
            )
        } else {
            self.close_betting_round(chip_state)
        }
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
                let r = if self.actions_this_street > 0 {
                    self.close_betting_round(self.chip_state)
                } else {
                    Node::from(
                        self.chip_state,
                        self.position.next(),
                        NodeType::EvenNode,
                        false,
                        None,
                        Rc::clone(&self.sb_range),
                        Rc::clone(&self.bb_range),
                        self.streets_remaining,
                        1,
                    )
                };

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

            let r = Node::from(
                ns,
                self.position.next(),
                NodeType::BehindNode,
                false,
                None,
                Rc::clone(&self.sb_range),
                Rc::clone(&self.bb_range),
                self.streets_remaining,
                self.actions_this_street + 1,
            );

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

                    let r = Node::from(
                        self.chip_state,
                        self.position.next(),
                        NodeType::AheadNode,
                        true,
                        Some(outcome),
                        Rc::clone(&self.sb_range),
                        Rc::clone(&self.bb_range),
                        self.streets_remaining,
                        self.actions_this_street,
                    );

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

                    let r = self.call_successor(ns);

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

                    let r = if amount <= amount_behind {
                        self.call_successor(ns)
                    } else {
                        Node::from(
                            ns,
                            self.position.next(),
                            NodeType::BehindNode,
                            false,
                            None,
                            Rc::clone(&self.sb_range),
                            Rc::clone(&self.bb_range),
                            self.streets_remaining,
                            self.actions_this_street + 1,
                        )
                    };

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

                    let r = if amount <= amount_behind {
                        self.call_successor(ns)
                    } else {
                        Node::from(
                            ns,
                            self.position.next(),
                            NodeType::BehindNode,
                            false,
                            None,
                            Rc::clone(&self.sb_range),
                            Rc::clone(&self.bb_range),
                            self.streets_remaining,
                            self.actions_this_street + 1,
                        )
                    };

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
                let mut children: [Box<Node>; 4] =
                    (0..4).map(|i| Box::new(self.successor(i).unwrap())).collect::<Vec<_>>().try_into().unwrap();

                let legal = self.legal_actions(&children);

                for i in 0..4 {
                    if legal[i] {
                        children[i].gen_subtree();
                    }
                }

                let mut actions = BehindActions::BLANK;
                actions.legal = legal;
                actions.probs = [0.25; 4];
                actions.children = Some(children);

                self.actions = Some(Actions::Behind(actions));
            }

            NodeType::EvenNode => {
                let mut children: [Box<Node>; 5] =
                    (0..5).map(|i| Box::new(self.successor(i).unwrap())).collect::<Vec<_>>().try_into().unwrap();

                let legal = self.legal_actions(&children);

                for i in 0..5 {
                    if legal[i] {
                        children[i].gen_subtree();
                    }
                }

                let mut actions = EvenActions::BLANK;
                actions.legal = legal;
                actions.probs = [0.2; 5];
                actions.children = Some(children);

                self.actions = Some(Actions::Even(actions));
            }

            _ => unreachable!(),
        }
    }
}
