use crate::game::{CardSet, Hand};

use std::collections::HashMap;

/// For two hands to be equivalent they must:
/// - have the same suitedness property
/// - have the same two ranks
/// - have the same board-rank mask for each hole-card suit
/// Preflop, a lot of hands will be equivalent (only 169 possible keys),
/// but postflop most hands will be distinct.
fn equivalence_key(board: &CardSet, hand: Hand) -> u64 {
    let suit_masks = board.suit_masks;

    let mut a = (hand.0.rank as u8, suit_masks[hand.0.suit]);

    let mut b = (hand.1.rank as u8, suit_masks[hand.1.suit]);

    if a > b {
        std::mem::swap(&mut a, &mut b);
    }

    let suited = u64::from(hand.0.suit == hand.1.suit);

    let mut key = 0;

    key |= a.0 as u64;
    key |= (a.1 as u64) << 4;

    key |= (b.0 as u64) << 17;
    key |= (b.1 as u64) << 21;

    key |= suited << 34;

    key
}

pub struct EquivalenceHash<T> {
    map: HashMap<u64, T>,
}

impl<T> EquivalenceHash<T> {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    pub fn lookup_hand_mut(&mut self, board: &CardSet, hand: Hand) -> Option<&mut T> {
        self.map.get_mut(&equivalence_key(board, hand))
    }

    pub fn lookup_hand(&self, board: &CardSet, hand: Hand) -> Option<&T> {
        self.map.get(&equivalence_key(board, hand))
    }

    pub fn insert_hand(&mut self, board: &CardSet, hand: Hand, value: T) {
        self.map.insert(equivalence_key(board, hand), value);
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl<T> Default for EquivalenceHash<T> {
    fn default() -> Self {
        Self::new()
    }
}
