//! Fractional indexes for assigning stable, sortable positions to ordered values.
//!
//! A [`Fracindex`] can be generated before, after, or between existing indexes
//! without renumbering the rest of the sequence. Its byte representation preserves
//! the same ordering as the index itself.

use std::fmt::Debug;

use smallvec::{SmallVec, smallvec};

/// Controls how space is allocated when generating an index before or after
/// another index.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FracindexPolicy {
    /// Chooses a midpoint toward the available boundary.
    ///
    /// This policy leaves room for insertions whose future positions are not
    /// expected to follow a predictable direction.
    Random,
    /// Moves by a fixed gap when possible and falls back to a midpoint near a
    /// component boundary.
    ///
    /// This is the default policy and is suitable for repeated appends or
    /// prepends.
    #[default]
    Sequential,
}

/// A variable-length fractional index used to order values without renumbering
/// existing indexes.
///
/// Indexes implement [`Ord`], and their canonical byte encodings returned by
/// [`Fracindex::to_bytes`] have the same lexicographic ordering.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Fracindex {
    inner: SmallVec<[u32; 4]>,
}

const MAX: u32 = u32::MAX;
const MIN: u32 = u32::MIN;
const DEFAULT: u32 = MAX / 2;
const GAP: u32 = 256;
const BYTES_CNT: usize = 32 / 8;

impl Default for Fracindex {
    fn default() -> Self {
        Self {
            inner: smallvec![DEFAULT],
        }
    }
}

impl Debug for Fracindex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fracindex({})", self.to_hex())
    }
}

impl Fracindex {
    /// Decodes a fractional index from its big-endian byte representation.
    ///
    /// An incomplete final four-byte component is padded with zero bytes. The
    /// decoded value is normalized by removing trailing zero components.
    ///
    /// Returns [`None`] when `bytes` is empty or contains no non-zero component.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            // Empty bytes cannot represent a valid fracindex.
            return None;
        }

        let inner_len = bytes.len().div_ceil(BYTES_CNT);
        let mut inner = SmallVec::with_capacity(inner_len);
        for i in 0..inner_len {
            let start = i * BYTES_CNT;
            let mut inner_bytes = [0u8; BYTES_CNT];
            for j in 0..BYTES_CNT {
                if start + j < bytes.len() {
                    inner_bytes[j] = bytes[start + j];
                }
            }
            inner.push(u32::from_be_bytes(inner_bytes));
        }

        // Remove trailing zeros.
        while inner.last() == Some(&0) {
            inner.pop();
        }

        // Check if it is not empty.
        if inner.is_empty() {
            return None;
        }

        Some(Self { inner })
    }

    /// Creates an index that sorts after `other` using the default
    /// [`FracindexPolicy::Sequential`] policy.
    pub fn new_after(other: &Self) -> Self {
        let policy = FracindexPolicy::default();
        Self::new_after_with_policy(other, policy)
    }

    /// Creates an index that sorts after `other` using `policy`.
    pub fn new_after_with_policy(other: &Self, policy: FracindexPolicy) -> Self {
        let other_inner = &other.inner;
        let other_len = other_inner.len();
        // The final component must not be zero in a valid fracindex.
        debug_assert_ne!(other_inner[other_len - 1], MIN);

        for i in 0..other_inner.len() {
            use FracindexPolicy::*;

            let value = other_inner[i];
            let get_midpoint_to_max = || {
                let distance = MAX - value;
                let mut inner = SmallVec::from_slice(&other_inner[..=i]);
                inner[i] = value + distance / 2 + distance % 2;
                Self { inner }
            };
            match &policy {
                Sequential if value <= MAX - GAP * 2 => {
                    let mut inner = SmallVec::from_slice(&other_inner[..=i]);
                    inner[i] += GAP;
                    return Self { inner };
                }
                Random if value < MAX => {
                    return get_midpoint_to_max();
                }
                Sequential if value != MAX => {
                    return get_midpoint_to_max();
                }
                _ => {}
            }
        }

        let mut inner = SmallVec::with_capacity(other_len + 1);
        inner.extend_from_slice(other_inner);
        inner.push(match policy {
            FracindexPolicy::Random => DEFAULT,
            FracindexPolicy::Sequential => GAP,
        });
        Self { inner }
    }

    /// Creates an index that sorts before `other` using the default
    /// [`FracindexPolicy::Sequential`] policy.
    pub fn new_before(other: &Self) -> Self {
        let policy = FracindexPolicy::default();
        Self::new_before_with_policy(other, policy)
    }

    /// Creates an index that sorts before `other` using `policy`.
    pub fn new_before_with_policy(other: &Self, policy: FracindexPolicy) -> Self {
        let other_inner = &other.inner;
        let other_len = other_inner.len();
        // The final component must not be zero in a valid fracindex.
        debug_assert_ne!(other_inner[other_len - 1], MIN);

        for i in 0..other_inner.len() {
            use FracindexPolicy::*;

            let value = other_inner[i];
            let get_midpoint_to_min = || {
                let mut inner = SmallVec::from_slice(&other_inner[..=i]);
                inner[i] /= 2;
                Self { inner }
            };
            match &policy {
                Sequential if value >= GAP * 2 => {
                    let mut inner = SmallVec::from_slice(&other_inner[..=i]);
                    inner[i] -= GAP;
                    return Self { inner };
                }
                Random if value > 1 => {
                    return get_midpoint_to_min();
                }
                Sequential if value > 1 => {
                    return get_midpoint_to_min();
                }
                _ => {}
            }
        }

        let len = other_len + 1;
        let mut inner = SmallVec::with_capacity(len);
        inner.extend_from_slice(other_inner);
        inner[len - 2] = MIN;
        inner.push(match policy {
            FracindexPolicy::Random => DEFAULT,
            FracindexPolicy::Sequential => MAX,
        });
        Self { inner }
    }

    /// Creates an index that sorts strictly between `a` and `b`.
    ///
    /// Returns [`None`] when the bounds are equal, reversed, or do not admit a
    /// valid fractional index between them.
    pub fn new_between(a: &Self, b: &Self) -> Option<Self> {
        let a_len = a.inner.len();
        let b_len = b.inner.len();
        let min_len = a_len.min(b_len);

        for i in 0..min_len {
            // Handle a zero component in `b` separately to avoid underflow.
            if b.inner[i] == MIN {
                if a.inner[i] != MIN {
                    return None;
                }
                continue;
            }

            if a.inner[i] < b.inner[i] - 1 {
                let mut inner = SmallVec::from_slice(&a.inner[..=i]);
                inner[i] = a.inner[i] + (b.inner[i] - a.inner[i]) / 2;
                return Some(Self { inner });
            }

            if a.inner[i] == b.inner[i] - 1 {
                // A value with this prefix remains below `b`. Increase the
                // first non-MAX suffix component in `a`, or append a component.
                if let Some(ai) = (i + 1..a_len).find(|&ai| a.inner[ai] != MAX) {
                    let value = a.inner[ai];
                    let distance = MAX - value;
                    let mut inner = SmallVec::from_slice(&a.inner[..=ai]);
                    inner[ai] = value + distance / 2 + distance % 2;
                    return Some(Self { inner });
                }

                let mut inner = SmallVec::with_capacity(a_len + 1);
                inner.extend_from_slice(&a.inner);
                inner.push(DEFAULT);
                return Some(Self { inner });
            }

            if a.inner[i] > b.inner[i] {
                return None;
            }
        }

        // Equal values have no midpoint, and a longer `a` with the same prefix
        // is ordered after `b`.
        if a_len != min_len || b_len == min_len {
            return None;
        }

        // `a` is a strict prefix of `b`. Move the final component of `b`
        // towards MIN; if that would become zero, extend the index instead.
        let mut inner = SmallVec::with_capacity(b_len + 1);
        inner.extend_from_slice(&b.inner);
        if inner[b_len - 1] >= 2 {
            inner[b_len - 1] /= 2;
        } else {
            inner[b_len - 1] = MIN;
            inner.push(DEFAULT);
        }
        Some(Self { inner })
    }

    /// Encodes this index as canonical big-endian bytes.
    ///
    /// Trailing zero bytes are omitted. Comparing the returned byte vectors
    /// lexicographically produces the same ordering as comparing the indexes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.inner.len() * BYTES_CNT);
        for word in &self.inner {
            bytes.extend_from_slice(&word.to_be_bytes());
        }
        while bytes.last() == Some(&0) {
            bytes.pop();
        }
        bytes
    }

    /// Formats the canonical byte representation as lowercase hexadecimal
    /// without a prefix.
    pub fn to_hex(&self) -> String {
        const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

        let trailing_zero_bytes = self
            .inner
            .last()
            .expect("a fractional index must contain at least one component")
            .trailing_zeros() as usize
            / 8;
        let byte_count = self.inner.len() * BYTES_CNT - trailing_zero_bytes;
        let mut hex = String::with_capacity(byte_count * 2);

        for byte in self
            .inner
            .iter()
            .flat_map(|word| word.to_be_bytes())
            .take(byte_count)
        {
            hex.push(HEX_DIGITS[(byte >> 4) as usize] as char);
            hex.push(HEX_DIGITS[(byte & 0x0f) as usize] as char);
        }

        hex
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngExt;
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
        collections::BTreeSet,
        ops::Bound::{Excluded, Unbounded},
        rc::Rc,
    };

    struct CountingAllocator;

    thread_local! {
        static TRACK_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
        static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
    }

    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                record_allocation();
            }
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) };
        }
    }

    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    fn record_allocation() {
        let is_tracking = TRACK_ALLOCATIONS
            .try_with(|tracking| tracking.get())
            .unwrap_or(false);
        if is_tracking {
            let _ = ALLOCATION_COUNT.try_with(|count| count.set(count.get() + 1));
        }
    }

    fn count_allocations<T>(operation: impl FnOnce() -> T) -> (T, usize) {
        ALLOCATION_COUNT.with(|count| count.set(0));
        TRACK_ALLOCATIONS.with(|tracking| {
            assert!(
                !tracking.replace(true),
                "allocation tracking cannot be nested"
            );
        });

        let result = operation();

        TRACK_ALLOCATIONS.with(|tracking| tracking.set(false));
        let count = ALLOCATION_COUNT.with(|count| count.get());
        (result, count)
    }

    fn bytes(words: &[u32]) -> Vec<u8> {
        let mut result: Vec<u8> = words.iter().flat_map(|word| word.to_be_bytes()).collect();
        while result.last() == Some(&0) {
            result.pop();
        }
        result
    }

    fn fracindex(words: &[u32]) -> Fracindex {
        Fracindex::from_bytes(&bytes(words)).expect("words must encode a valid fracindex")
    }

    #[test]
    fn test_default() {
        let fracindex = Fracindex::default();
        assert_eq!(fracindex.to_bytes(), bytes(&[DEFAULT]));
    }

    #[test]
    fn test_bytes_round_trip() {
        let expected = bytes(&[DEFAULT, GAP, MAX]);
        let fracindex = Fracindex::from_bytes(&expected).unwrap();

        assert_eq!(fracindex.inner.as_slice(), &[DEFAULT, GAP, MAX]);
        assert_eq!(fracindex.to_bytes(), expected);
    }

    #[test]
    fn test_from_bytes_pads_an_incomplete_final_word() {
        let fracindex = Fracindex::from_bytes(&[0x12, 0x34]).unwrap();

        assert_eq!(fracindex.inner.as_slice(), &[0x1234_0000]);
        assert_eq!(fracindex.to_bytes(), vec![0x12, 0x34]);
    }

    #[test]
    fn test_from_bytes_rejects_empty_or_zero_and_removes_trailing_zero_words() {
        assert!(Fracindex::from_bytes(&[]).is_none());
        assert!(Fracindex::from_bytes(&[0; BYTES_CNT]).is_none());

        let encoded = bytes(&[GAP, MIN]);
        let fracindex = Fracindex::from_bytes(&encoded).unwrap();
        assert_eq!(fracindex.inner.as_slice(), &[GAP]);
        assert_eq!(fracindex.to_bytes(), bytes(&[GAP]));
    }

    #[test]
    fn test_to_hex_is_canonical_and_uses_one_allocation() {
        let test_cases: &[(&[u32], &str)] = &[
            (&[0x1200_0000], "12"),
            (&[0x1234_0000], "1234"),
            (&[0x1234_5600], "123456"),
            (&[0x1234_5678], "12345678"),
            (&[0x0000_0001, 0x1234_5600], "00000001123456"),
        ];

        for &(words, expected) in test_cases {
            let fracindex = fracindex(words);
            let (hex, allocation_count) = count_allocations(|| fracindex.to_hex());

            assert_eq!(hex, expected, "words: {words:?}");
            assert_eq!(allocation_count, 1, "words: {words:?}");
        }
    }

    #[test]
    fn test_serialized_order_matches_fracindex_order() {
        let indexes = [
            fracindex(&[MIN, GAP]),
            fracindex(&[1]),
            fracindex(&[DEFAULT]),
            fracindex(&[DEFAULT, GAP]),
            fracindex(&[MAX]),
        ];

        for pair in indexes.windows(2) {
            assert!(pair[0] < pair[1]);
            assert!(pair[0].to_bytes() < pair[1].to_bytes());
        }
    }

    #[track_caller]
    fn assert_words(actual: &Fracindex, expected: &[u32]) {
        assert_eq!(actual.inner.as_slice(), expected);
        assert_eq!(actual.to_bytes(), bytes(expected));
    }

    #[test]
    fn test_new_after_random() {
        let test_cases: &[(&[u32], &[u32])] = &[
            (&[DEFAULT], &[3_221_225_471]),
            (&[MIN, GAP], &[2_147_483_648]),
            (&[MAX - 1], &[MAX]),
            (&[MAX], &[MAX, DEFAULT]),
            (&[MAX, MAX - 1], &[MAX, MAX]),
            (&[MAX, MAX], &[MAX, MAX, DEFAULT]),
        ];

        for (input, expected) in test_cases {
            let current = fracindex(input);
            let next = Fracindex::new_after_with_policy(&current, FracindexPolicy::Random);
            assert_words(&next, expected);
            assert!(next > current, "input: {input:?}");
        }
    }

    #[test]
    fn test_new_after_sequential() {
        let test_cases: &[(&[u32], &[u32])] = &[
            (&[DEFAULT], &[DEFAULT + GAP]),
            (&[MAX - GAP * 2], &[MAX - GAP]),
            (&[MAX - GAP * 2 + 2], &[MAX - GAP + 1]),
            (&[MAX - GAP * 2 + 64], &[MAX - GAP + 32]),
            (&[MAX - GAP], &[MAX - GAP / 2]),
            (&[MAX - GAP + 1], &[MAX - GAP / 2 + 1]),
            (&[MAX], &[MAX, GAP]),
            (&[MAX, MAX - GAP], &[MAX, MAX - GAP / 2]),
            (&[DEFAULT, DEFAULT], &[DEFAULT + GAP]),
        ];

        for (input, expected) in test_cases {
            let current = fracindex(input);
            let next = Fracindex::new_after(&current);
            assert_words(&next, expected);
            assert!(next > current, "input: {input:?}");
        }
    }

    #[test]
    fn test_new_before_random() {
        let test_cases: &[(&[u32], &[u32])] = &[
            (&[DEFAULT], &[DEFAULT / 2]),
            (&[2], &[1]),
            (&[1], &[MIN, DEFAULT]),
            (&[MIN, 1], &[MIN, MIN, DEFAULT]),
            (&[MIN, MAX], &[MIN, MAX / 2]),
            (&[DEFAULT, DEFAULT], &[DEFAULT / 2]),
        ];

        for (input, expected) in test_cases {
            let current = fracindex(input);
            let previous = Fracindex::new_before_with_policy(&current, FracindexPolicy::Random);
            assert_words(&previous, expected);
            assert!(previous < current, "input: {input:?}");
        }
    }

    #[test]
    fn test_new_before_sequential() {
        let test_cases: &[(&[u32], &[u32])] = &[
            (&[DEFAULT], &[DEFAULT - GAP]),
            (&[GAP * 2], &[GAP]),
            (&[GAP * 2 - 1], &[GAP - 1]),
            (&[GAP * 2 - 2], &[GAP - 1]),
            (&[GAP * 2 - 64], &[GAP - 32]),
            (&[GAP + 1], &[GAP / 2]),
            (&[GAP], &[GAP / 2]),
            (&[GAP - 1], &[GAP / 2 - 1]),
            (&[1], &[MIN, MAX]),
            (&[MIN, MAX], &[MIN, MAX - GAP]),
            (&[MIN, MAX - GAP], &[MIN, MAX - 2 * GAP]),
            (&[MIN, GAP], &[MIN, GAP / 2]),
            (&[DEFAULT, DEFAULT], &[DEFAULT - GAP]),
        ];

        for (input, expected) in test_cases {
            let current = fracindex(input);
            let previous = Fracindex::new_before(&current);
            assert_words(&previous, expected);
            assert!(previous < current, "input: {input:?}");
        }
    }

    #[test]
    fn test_new_between() {
        struct TestCase {
            a: &'static [u32],
            b: &'static [u32],
            expected: Option<&'static [u32]>,
        }
        macro_rules! test_case {
            ($a:expr, $b:expr => $expected:expr) => {
                TestCase {
                    a: $a,
                    b: $b,
                    expected: $expected,
                }
            };
        }
        let test_cases = [
            test_case! { &[1], &[2] => Some(&[1, DEFAULT]) },
            test_case! { &[MIN, DEFAULT], &[DEFAULT] => Some(&[DEFAULT / 2]) },
            test_case! { &[62], &[64] => Some(&[63]) },
            test_case! { &[63], &[64] => Some(&[63, DEFAULT]) },
            test_case! { &[63], &[63, DEFAULT] => Some(&[63, DEFAULT / 2]) },
            test_case! { &[63], &[63] => None },
            test_case! { &[64], &[63] => None },
            test_case! { &[63], &[63, 1] => Some(&[63, MIN, DEFAULT]) },
            test_case! { &[63], &[63, 2] => Some(&[63, 1]) },
            test_case! { &[MIN, MIN, 1], &[MIN, MIN, 3] => Some(&[MIN, MIN, 2]) },
            test_case! { &[MAX, MAX, MAX - 2], &[MAX, MAX, MAX] => Some(&[MAX, MAX, MAX - 1]) },
            test_case! { &[64, 1], &[63, 3] => None },
            test_case! { &[1, MAX], &[2] => Some(&[1, MAX, DEFAULT]) },
            test_case! { &[1, MAX, MAX], &[2] => Some(&[1, MAX, MAX, DEFAULT]) },
            test_case! { &[1, MAX, MAX - 1], &[2] => Some(&[1, MAX, MAX]) },
            test_case! { &[DEFAULT], &[DEFAULT + GAP] => Some(&[DEFAULT + GAP / 2]) },
        ];

        for (i, test_case) in test_cases.iter().enumerate() {
            let a = fracindex(test_case.a);
            let b = fracindex(test_case.b);
            let result = Fracindex::new_between(&a, &b);

            match (result, test_case.expected) {
                (Some(result), Some(expected)) => {
                    assert_words(&result, expected);
                    assert!(a < result && result < b, "iteration {i}");
                }
                (None, None) => {}
                _ => panic!("unexpected result at iteration {i}"),
            }
        }
    }

    #[test]
    fn test_repeated_new_between_stays_strictly_ordered() {
        let left = Fracindex::default();
        let mut right = Fracindex::new_after(&left);

        for iteration in 0..64 {
            let next = Fracindex::new_between(&left, &right)
                .expect("ordered bounds must always have a midpoint");
            assert!(left < next, "iteration {iteration}");
            assert!(next < right, "iteration {iteration}");
            right = next;
        }
    }

    fn insert_index(
        ordered_indexes: &mut BTreeSet<Rc<Fracindex>>,
        random_indexes: &mut Vec<Rc<Fracindex>>,
        index: Fracindex,
        iteration: usize,
        action: &str,
    ) {
        let index = Rc::new(index);
        assert!(
            ordered_indexes.insert(Rc::clone(&index)),
            "iteration {iteration}, {action}"
        );
        random_indexes.push(index);
    }

    fn random_between(
        ordered_indexes: &BTreeSet<Rc<Fracindex>>,
        random_indexes: &[Rc<Fracindex>],
        rng: &mut impl rand::Rng,
        iteration: usize,
    ) -> Fracindex {
        let anchor = Rc::clone(&random_indexes[rng.random_range(0..random_indexes.len())]);
        let (left, right) = if ordered_indexes.last().unwrap() == &anchor {
            let left = ordered_indexes
                .range(..Rc::clone(&anchor))
                .next_back()
                .unwrap();
            (left.as_ref(), anchor.as_ref())
        } else {
            let right = ordered_indexes
                .range((Excluded(Rc::clone(&anchor)), Unbounded))
                .next()
                .unwrap();
            (anchor.as_ref(), right.as_ref())
        };
        let index = Fracindex::new_between(left, right)
            .unwrap_or_else(|| panic!("iteration {iteration}, no index between adjacent orders"));
        assert!(
            left < &index && &index < right,
            "iteration {iteration}, invalid index between adjacent orders"
        );
        index
    }

    fn assert_random_insertions_stay_strictly_ordered(policy: impl Fn() -> FracindexPolicy) {
        let mut rng = rand::rng();

        let mut ordered_indexes = BTreeSet::new();
        let mut random_indexes = vec![];
        let mut to_insert_index = Rc::new(Fracindex::default());
        for _ in 0..4 {
            ordered_indexes.insert(Rc::clone(&to_insert_index));
            random_indexes.push(Rc::clone(&to_insert_index));

            to_insert_index = Rc::new(Fracindex::new_after(&to_insert_index))
        }

        for iteration in 0..10_000 {
            let rand_choice = rng.random_range(0..100);
            match rand_choice {
                i if i <= 5 => {
                    let first = ordered_indexes.first().unwrap();
                    let index = Fracindex::new_before_with_policy(first.as_ref(), policy());
                    assert!(
                        &index < first.as_ref(),
                        "iteration {iteration}, before insertion"
                    );
                    insert_index(
                        &mut ordered_indexes,
                        &mut random_indexes,
                        index,
                        iteration,
                        "before insertion",
                    );
                }
                i if i > 5 && i <= 10 => {
                    let last = ordered_indexes.last().unwrap();
                    let index = Fracindex::new_after_with_policy(last.as_ref(), policy());
                    assert!(
                        last.as_ref() < &index,
                        "iteration {iteration}, after insertion"
                    );
                    insert_index(
                        &mut ordered_indexes,
                        &mut random_indexes,
                        index,
                        iteration,
                        "after insertion",
                    );
                }
                i if i > 10 && i <= 30 => {
                    let index =
                        random_between(&ordered_indexes, &random_indexes, &mut rng, iteration);
                    insert_index(
                        &mut ordered_indexes,
                        &mut random_indexes,
                        index,
                        iteration,
                        "between insertion",
                    );
                }
                _ => {
                    let removal_position = rng.random_range(0..random_indexes.len());
                    let removed = random_indexes.swap_remove(removal_position);
                    assert!(
                        ordered_indexes.remove(&removed),
                        "iteration {iteration}, removal at position {removal_position}"
                    );

                    let index =
                        random_between(&ordered_indexes, &random_indexes, &mut rng, iteration);
                    insert_index(
                        &mut ordered_indexes,
                        &mut random_indexes,
                        index,
                        iteration,
                        "shuffled insertion",
                    );
                }
            }

            assert_eq!(ordered_indexes.len(), random_indexes.len());
        }

        let mut indexes = ordered_indexes.iter();
        let mut previous = indexes.next().unwrap();
        for (position, current) in indexes.enumerate() {
            assert!(
                previous.as_ref() < current.as_ref(),
                "unordered indexes at position {position}"
            );
            assert!(
                previous.to_bytes() < current.to_bytes(),
                "unordered bytes at position {position}"
            );
            previous = current;
        }
    }

    #[test]
    fn test_random_policy_insertions_stay_strictly_ordered() {
        for _i in 1..10 {
            assert_random_insertions_stay_strictly_ordered(|| FracindexPolicy::Random);
        }
    }

    #[test]
    fn test_sequential_policy_insertions_stay_strictly_ordered() {
        for _i in 1..10 {
            assert_random_insertions_stay_strictly_ordered(|| FracindexPolicy::Sequential);
        }
    }
}
