use crate::game::{CARD_MASKS, CARDS, Card};
use std::time::UNIX_EPOCH;

pub struct XorShiftU64 {
    pub state: u64,
}

const SEED: u128 = 0xF8D1C463A579BE02;

impl XorShiftU64 {
    const EXPLORATION: f64 = 0.1;

    pub fn new() -> Self {
        Self { state: (UNIX_EPOCH.elapsed().unwrap().as_nanos() % SEED) as u64 }
    }

    pub fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub fn gen_range(&mut self, l: i64, r: i64) -> i64 {
        let range = (r - l + 1) as u64;
        l + (self.next() % range) as i64
    }

    /// Fisher-Yates shuffle
    pub fn shuffle(&mut self) -> [Card; Card::NUM] {
        let mut p = CARDS;

        for i in (1..Card::NUM).rev() {
            let j = self.gen_range(0, i as i64) as usize;
            p.swap(i as usize, j);
        }

        p
    }

    pub fn next_card(&mut self, seen: &mut u64) -> Card {
        let mut idx;

        loop {
            idx = self.gen_range(0, 51) as usize;
            if *seen & CARD_MASKS[idx] == 0 {
                *seen |= CARD_MASKS[idx];
                break;
            }
        }

        CARDS[idx]
    }

    pub fn next_f64(&mut self) -> f64 {
        const SCALE: f64 = 1.0 / ((1_u64 << 53) as f64);

        ((self.next() >> 11) as f64) * SCALE
    }

    pub fn explore_action(&mut self, ps: &[f64], legal: &[bool]) -> usize {
        let n = ps.len();
        let legal_count = legal.iter().filter(|&&x| x).count();

        let mut pref = vec![0.0; n + 1];

        for i in 1..=n {
            let p = if legal[i - 1] {
                (1.0 - Self::EXPLORATION) * ps[i - 1] + Self::EXPLORATION / legal_count as f64
            } else {
                0.0
            };

            pref[i] = pref[i - 1] + p;
        }

        let r = self.next_f64() * pref[n];

        let (mut lo, mut hi) = (1, n);

        while lo < hi {
            let mid = lo + (hi - lo) / 2;

            if r < pref[mid] {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }

        lo - 1
    }

    pub fn choose_action(&mut self, ps: &[f64]) -> (usize, f64) {
        let n = ps.len();
        let mut pref = vec![0.0; n + 1];

        for i in 1..=n {
            pref[i] = pref[i - 1] + ps[i - 1];
        }

        let r = self.next_f64();

        let (mut lo, mut hi) = (1, n);

        while lo < hi {
            let mid = lo + (hi - lo) / 2;

            if pref[mid] >= r {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }

        (lo - 1, ps[lo - 1])
    }
}
