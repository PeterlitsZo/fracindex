//! Fractional indexes for assigning stable, sortable positions to ordered
//! values.
//!
//! A [`Fracindex`] can be generated before, after, or between existing indexes
//! without renumbering the rest of the sequence. Its byte representation
//! preserves the same ordering as the index itself.
//!
//! See [README](https://github.com/PeterlitsZo/fracindex) for more information.

use std::{borrow::Cow, fmt::Debug};

use smallvec::{SmallVec, smallvec};

mod builder;
mod error;

pub use builder::*;
pub use error::*;

/// Controls which existing boundary indexes are preserved when rebalancing a
/// sorted slice of [`Fracindex`] values.
///
/// Preserving boundaries keeps external references to those boundary positions
/// stable, while replacing more values usually gives the rebalanced sequence
/// more compact encodings.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum RebalancePolicy {
    /// Keeps both the first and last indexes unchanged.
    ///
    /// All interior indexes are redistributed between the original boundaries.
    #[default]
    PreserveBoth,
    /// Keeps only the first index unchanged.
    ///
    /// The remaining indexes are regenerated after the first index.
    PreserveFirst,
    /// Keeps only the last index unchanged.
    ///
    /// The preceding indexes are regenerated before the last index.
    PreserveLast,
    /// Replaces every index in the slice.
    ///
    /// The new sequence is centered around [`Fracindex::default`].
    ReplaceAll,
}

/// Convenient result type used by fallible `fracindex` APIs.
pub type FracindexResult<T> = Result<T, FracindexError>;

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
    /// Starts building a new fractional index.
    pub fn builder<'a>() -> FracindexBuilder<'a> {
        FracindexBuilder::default()
    }

    /// Decodes a fractional index from its big-endian byte representation.
    ///
    /// An incomplete final four-byte component is padded with zero bytes. The
    /// decoded value is normalized by removing trailing zero components.
    ///
    /// Returns [`Err`] when `bytes` is empty or contains no non-zero component.
    pub fn from_bytes(bytes: &[u8]) -> FracindexResult<Self> {
        if bytes.is_empty() {
            // Empty bytes cannot represent a valid fracindex.
            let err = FracindexError::new(FracindexErrorKind::Invalid, "bytes must not be empty")
                .with_context("bytes", format!("{:?}", bytes));
            return Err(err);
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
            let err = FracindexError::new(
                FracindexErrorKind::Invalid,
                "decoded bytes must not be empty",
            )
            .with_context("bytes", format!("{:?}", bytes))
            .with_context("parsed", format!("{:?}", inner));
            return Err(err);
        }

        Ok(Self { inner })
    }

    /// Decodes a fractional index from an unprefixed hexadecimal string.
    ///
    /// Both lowercase and uppercase ASCII hexadecimal digits are accepted.
    /// Decoded bytes use the same normalization as [`Fracindex::from_bytes`].
    ///
    /// Zero padding is allowed:
    ///
    /// ```rust
    /// # use fracindex::Fracindex;
    /// assert_eq!(Fracindex::from_hex("1000 0000").unwrap(), Fracindex::from_hex("1").unwrap());
    /// ```
    ///
    /// Returns [`Err`] when `hex` is empty string, contains a non-hexadecimal
    /// character (but space is allowed), or decodes to no non-zero component.
    pub fn from_hex(hex: &str) -> FracindexResult<Self> {
        if hex.is_empty() {
            let err = FracindexError::new(
                FracindexErrorKind::Invalid,
                "hex string MUST contain digits but now it is empty",
            )
            .with_context("hex", format!("{:?}", hex));
            return Err(err);
        }

        let decode_nibble = |digit| match digit {
            b'0'..=b'9' => Ok(digit - b'0'),
            b'a'..=b'f' => Ok(digit - b'a' + 10),
            b'A'..=b'F' => Ok(digit - b'A' + 10),
            digit => {
                let err = FracindexError::new(
                    FracindexErrorKind::Invalid,
                    "hex string must contain only hexadecimal digits",
                )
                .with_context("hex", format!("{:?}", hex))
                .with_context("digit", format!("{:?}", digit));
                Err(err)
            }
        };

        let mut bytes = Vec::with_capacity(
            hex.as_bytes()
                .iter()
                .filter(|c| !c.is_ascii_whitespace())
                .count()
                / 2,
        );
        let mut i = 0;
        for hex in hex.as_bytes().iter() {
            // Skip whitespace characters.
            if hex.is_ascii_whitespace() {
                continue;
            }

            let num = decode_nibble(*hex)?;
            if i % 2 == 0 {
                bytes.push(num << 4);
            } else {
                bytes[i / 2] |= num;
            }

            i += 1;
        }

        Self::from_bytes(&bytes)
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

    /// The bytes length returned by [`Fracindex::to_bytes`]. Cheaper than calling
    /// [`Fracindex::to_bytes`] and checking its length.
    pub fn bytes_len(&self) -> usize {
        self.inner.len() * BYTES_CNT
            - self
                .inner
                .last()
                .expect("a fractional index must contain at least one component")
                .trailing_zeros() as usize
                / 8
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

    /// Rebalances the fracindexes slice.
    ///
    /// The slice MUST be sorted in ascending order. If the slice is not sorted,
    /// the result is undefined. We do not check it.
    pub fn rebalance(slice: &[Self], policy: RebalancePolicy) -> FracindexResult<Cow<'_, [Self]>> {
        Self::rebalance_with_policy(slice, policy, SpacePolicy::default())
    }

    /// Rebalances the fracindexes slice.
    ///
    /// The slice MUST be sorted in ascending order. If the slice is not sorted,
    /// the result is undefined. We do not check it.
    pub fn rebalance_with_policy(
        slice: &[Self],
        rp: RebalancePolicy,
        fp: SpacePolicy,
    ) -> FracindexResult<Cow<'_, [Self]>> {
        fn rebalance_in_range(
            sink: &mut [Fracindex],
            start: usize,
            end: usize,
            first: &Fracindex,
            last: &Fracindex,
        ) -> FracindexResult<()> {
            if end <= start {
                return Ok(());
            }
            if end - start == 1 {
                sink[start] = Fracindex::builder().between(first, last).build()?;
                return Ok(());
            }

            let mid = (start + end) / 2;
            let mid_val = Fracindex::builder().between(first, last).build()?;
            sink[mid] = mid_val.clone();
            rebalance_in_range(sink, start, mid, first, &mid_val)?;
            rebalance_in_range(sink, mid + 1, end, &mid_val, last)?;

            Ok(())
        }

        fn rebalance_after_part_random(
            sink: &mut [Fracindex],
            start: usize,
            end: usize,
            first: &Fracindex,
        ) -> FracindexResult<()> {
            if end <= start {
                return Ok(());
            }
            if end - start == 1 {
                sink[start] = Fracindex::builder()
                    .after(first)
                    .space_policy(SpacePolicy::Random)
                    .build()?;
                return Ok(());
            }

            let mid = (start + end) / 2;
            let mid_val = Fracindex::builder()
                .after(first)
                .space_policy(SpacePolicy::Random)
                .build()?;
            sink[mid] = mid_val.clone();
            rebalance_in_range(sink, start, mid, first, &mid_val)?;
            rebalance_after_part_random(sink, mid + 1, end, &mid_val)?;

            Ok(())
        }

        fn rebalance_before_part_random(
            sink: &mut [Fracindex],
            start: usize,
            end: usize,
            last: &Fracindex,
        ) -> FracindexResult<()> {
            if end <= start {
                return Ok(());
            }
            if end - start == 1 {
                sink[start] = Fracindex::builder()
                    .before(last)
                    .space_policy(SpacePolicy::Random)
                    .build()?;
                return Ok(());
            }

            let mid = (start + end) / 2;
            let mid_val = Fracindex::builder()
                .before(last)
                .space_policy(SpacePolicy::Random)
                .build()?;
            sink[mid] = mid_val.clone();
            rebalance_in_range(sink, mid + 1, end, &mid_val, last)?;
            rebalance_before_part_random(sink, start, mid, &mid_val)?;

            Ok(())
        }

        let len = slice.len();

        // Fast path: for some case, we can return the slice as-is.
        match rp {
            RebalancePolicy::PreserveBoth => {
                if len <= 2 {
                    return Ok(Cow::Borrowed(slice));
                }
            }
            RebalancePolicy::PreserveFirst | RebalancePolicy::PreserveLast => {
                if len <= 1 {
                    return Ok(Cow::Borrowed(slice));
                }
            }
            RebalancePolicy::ReplaceAll => {
                if len == 0 {
                    return Ok(Cow::Borrowed(slice));
                }
            }
        }

        let mut result = vec![Self::default(); len];

        match rp {
            RebalancePolicy::PreserveBoth => {
                result[0] = slice[0].clone();
                result[len - 1] = slice[len - 1].clone();
                rebalance_in_range(&mut result, 1, len - 1, &slice[0], &slice[len - 1])?;
                Ok(Cow::Owned(result))
            }
            RebalancePolicy::PreserveFirst => {
                result[0] = slice[0].clone();

                match fp {
                    SpacePolicy::Sequential => {
                        for i in 1..len {
                            result[i] = Fracindex::builder()
                                .after(&result[i - 1])
                                .space_policy(fp)
                                .build()?;
                        }
                    }
                    SpacePolicy::Random => {
                        rebalance_after_part_random(&mut result, 1, len, &slice[0])?;
                    }
                }

                Ok(Cow::Owned(result))
            }
            RebalancePolicy::PreserveLast => {
                result[len - 1] = slice[len - 1].clone();

                match fp {
                    SpacePolicy::Sequential => {
                        for i in 1..len {
                            let j = len - 1 - i;
                            result[j] = Fracindex::builder()
                                .before(&result[j + 1])
                                .space_policy(fp)
                                .build()?;
                        }
                    }
                    SpacePolicy::Random => {
                        rebalance_before_part_random(&mut result, 0, len - 1, &slice[len - 1])?;
                    }
                }

                Ok(Cow::Owned(result))
            }
            RebalancePolicy::ReplaceAll => {
                let mid = len / 2;
                result[mid] = Fracindex::default();

                match fp {
                    SpacePolicy::Sequential => {
                        for i in 0..mid {
                            let j = mid - 1 - i;
                            result[j] = Fracindex::builder()
                                .before(&result[j + 1])
                                .space_policy(fp)
                                .build()?;
                        }
                        for i in 0..len - mid - 1 {
                            let j = mid + 1 + i;
                            result[j] = Fracindex::builder()
                                .after(&result[j - 1])
                                .space_policy(fp)
                                .build()?;
                        }
                    }
                    SpacePolicy::Random => {
                        rebalance_before_part_random(&mut result, 0, mid, &Fracindex::default())?;
                        rebalance_after_part_random(
                            &mut result,
                            mid + 1,
                            len,
                            &Fracindex::default(),
                        )?;
                    }
                }

                Ok(Cow::Owned(result))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngExt;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    use std::collections::BTreeSet;
    use std::ops::Bound::{Excluded, Unbounded};
    use std::rc::Rc;

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
        assert!(Fracindex::from_bytes(&[]).is_err());
        assert!(Fracindex::from_bytes(&[0; BYTES_CNT]).is_err());

        let encoded = bytes(&[GAP, MIN]);
        let fracindex = Fracindex::from_bytes(&encoded).unwrap();
        assert_eq!(fracindex.inner.as_slice(), &[GAP]);
        assert_eq!(fracindex.to_bytes(), bytes(&[GAP]));
    }

    #[test]
    fn test_from_hex_decodes_lowercase_uppercase_and_partial_words() {
        let test_cases: &[(&str, &[u32])] = &[
            ("12", &[0x1200_0000]),
            ("12345678", &[0x1234_5678]),
            ("aBcDeF", &[0xabcd_ef00]),
            ("00000001123456", &[0x0000_0001, 0x1234_5600]),
        ];

        for &(hex, expected) in test_cases {
            let fracindex = Fracindex::from_hex(hex).unwrap();
            assert_words(&fracindex, expected);
        }
    }

    #[test]
    fn test_from_hex_round_trips_canonical_hex_and_normalizes_trailing_zero_words() {
        let indexes = [
            fracindex(&[0x1200_0000]),
            fracindex(&[0x1234_5678]),
            fracindex(&[0x0000_0001, 0x1234_5600]),
        ];

        for expected in indexes {
            let hex = expected.to_hex();
            assert_eq!(Fracindex::from_hex(&hex).unwrap(), expected);
        }

        let normalized = Fracindex::from_hex("0000010000000000").unwrap();
        assert_words(&normalized, &[0x0000_0100]);
        assert_eq!(normalized.to_hex(), "000001");
    }

    #[test]
    fn test_from_hex_rejects_invalid_inputs() {
        for invalid in ["", "0", "gg", "0x12", "１２", "00", "00000000"] {
            assert!(
                Fracindex::from_hex(invalid).is_err(),
                "input should be rejected: {invalid:?}"
            );
        }
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
    fn test_builder_after_random() {
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
            let next = Fracindex::builder()
                .after(&current)
                .space_policy(SpacePolicy::Random)
                .build()
                .unwrap();
            assert_words(&next, expected);
            assert!(next > current, "input: {input:?}");
        }
    }

    #[test]
    fn test_builder_after_sequential() {
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
            let next = Fracindex::builder().after(&current).build().unwrap();
            assert_words(&next, expected);
            assert!(next > current, "input: {input:?}");
        }
    }

    #[test]
    fn test_builder_before_random() {
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
            let previous = Fracindex::builder()
                .before(&current)
                .space_policy(SpacePolicy::Random)
                .build()
                .unwrap();
            assert_words(&previous, expected);
            assert!(previous < current, "input: {input:?}");
        }
    }

    #[test]
    fn test_builder_before_sequential() {
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
            let previous = Fracindex::builder().before(&current).build().unwrap();
            assert_words(&previous, expected);
            assert!(previous < current, "input: {input:?}");
        }
    }

    #[test]
    fn test_builder_between() {
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
            // Part I.
            test_case! { &[1], &[2] => Some(&[1, DEFAULT]) },
            test_case! { &[MIN, DEFAULT], &[DEFAULT] => Some(&[DEFAULT / 2]) },
            test_case! { &[62], &[64] => Some(&[63]) },
            test_case! { &[63], &[64] => Some(&[63, DEFAULT]) },
            test_case! { &[63], &[64, MAX] => Some(&[63, DEFAULT]) },
            test_case! { &[63], &[63, DEFAULT] => Some(&[63, DEFAULT / 2]) },
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
            // Part II.
            test_case! { &[1], &[1] => Some(&[1]) },
            test_case! { &[63], &[63] => Some(&[63]) },
            test_case! { &[MIN, DEFAULT], &[MIN, DEFAULT] => Some(&[MIN, DEFAULT]) },
            test_case! { &[MAX], &[MAX] => Some(&[MAX]) },
        ];

        for (i, test_case) in test_cases.iter().enumerate() {
            let a = fracindex(test_case.a);
            let b = fracindex(test_case.b);
            let result = Fracindex::builder().between(&a, &b).build();

            match (result, test_case.expected) {
                (Ok(result), Some(expected)) => {
                    assert_words(&result, expected);
                    assert!(a <= result && result <= b, "iteration {i}");
                }
                (Err(_), None) => {}
                (result, expected) => {
                    panic!("unexpected result at iteration {i}, want {expected:?}, got {result:?}")
                }
            }
        }
    }

    #[test]
    fn test_repeated_builder_between_stays_strictly_ordered() {
        let left = Fracindex::default();
        let mut right = Fracindex::builder().after(&left).build().unwrap();

        for iteration in 0..64 {
            let next = Fracindex::builder()
                .between(&left, &right)
                .build()
                .expect("ordered bounds must always have a midpoint");
            assert!(left < next, "iteration {iteration}");
            assert!(next < right, "iteration {iteration}");
            right = next;
        }
    }

    #[test]
    fn test_builder_builds_with_each_bound_configuration() {
        let first = Fracindex::default();
        let last = Fracindex::builder().after(&first).build().unwrap();

        assert_eq!(Fracindex::builder().build().unwrap(), first);

        let after = Fracindex::builder().after(&first).build().unwrap();
        assert!(first < after);

        let before = Fracindex::builder().before(&last).build().unwrap();
        assert!(before < last);

        let between = Fracindex::builder().between(&first, &last).build().unwrap();
        let chained = Fracindex::builder()
            .after(&first)
            .before(&last)
            .build()
            .unwrap();
        assert_eq!(between, chained);
        assert!(first < chained && chained < last);
    }

    #[test]
    fn test_builder_uses_random_space_policy() {
        let current = Fracindex::default();

        let after = Fracindex::builder()
            .after(&current)
            .space_policy(SpacePolicy::Random)
            .build()
            .unwrap();
        let before = Fracindex::builder()
            .before(&current)
            .space_policy(SpacePolicy::Random)
            .build()
            .unwrap();

        assert_words(&after, &[3_221_225_471]);
        assert_words(&before, &[DEFAULT / 2]);
    }

    #[test]
    fn test_builder_batch_after_before_and_no_bounds() {
        let current = Fracindex::default();

        let after = Fracindex::builder().after(&current).batch_build(3).unwrap();
        assert_eq!(after.len(), 3);
        assert!(current < after[0]);
        assert_eq!(after[0], Fracindex::from_hex("8000 00ff").unwrap());
        assert_eq!(after[2], Fracindex::from_hex("8000 02ff").unwrap());
        assert!(after.windows(2).all(|pair| pair[0] < pair[1]));

        let before = Fracindex::builder()
            .before(&current)
            .batch_build(3)
            .unwrap();
        assert_eq!(before.len(), 3);
        assert!(before[2] < current);
        assert_eq!(before[0], Fracindex::from_hex("7fff fcff").unwrap());
        assert_eq!(before[2], Fracindex::from_hex("7fff feff").unwrap());
        assert!(before.windows(2).all(|pair| pair[0] < pair[1]));

        let unbounded = Fracindex::builder().batch_build(5).unwrap();
        assert_eq!(unbounded.len(), 5);
        assert_eq!(unbounded[0], Fracindex::from_hex("7fff fdff").unwrap());
        assert_eq!(unbounded[2], Fracindex::default());
        assert_eq!(unbounded[4], Fracindex::from_hex("8000 01ff").unwrap());
        assert!(unbounded.windows(2).all(|pair| pair[0] < pair[1]));

        assert!(Fracindex::builder().batch_build(0).unwrap().is_empty());
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
        let index = Fracindex::builder()
            .between(left, right)
            .build()
            .expect("iteration {iteration}, no index between adjacent orders");
        assert!(
            left < &index && &index < right,
            "iteration {iteration}, invalid index between adjacent orders"
        );
        index
    }

    fn assert_random_insertions_stay_strictly_ordered(policy: impl Fn() -> SpacePolicy) {
        let mut rng = rand::rng();

        let mut ordered_indexes = BTreeSet::new();
        let mut random_indexes = vec![];
        let mut to_insert_index = Rc::new(Fracindex::default());
        for _ in 0..4 {
            ordered_indexes.insert(Rc::clone(&to_insert_index));
            random_indexes.push(Rc::clone(&to_insert_index));

            to_insert_index = Rc::new(
                Fracindex::builder()
                    .after(&to_insert_index)
                    .build()
                    .unwrap(),
            )
        }

        for iteration in 0..10_000 {
            let rand_choice = rng.random_range(0..100);
            match rand_choice {
                i if i <= 5 => {
                    let first = ordered_indexes.first().unwrap();
                    let index = Fracindex::builder()
                        .before(first.as_ref())
                        .space_policy(policy())
                        .build()
                        .unwrap();
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
                    let index = Fracindex::builder()
                        .after(last.as_ref())
                        .space_policy(policy())
                        .build()
                        .unwrap();
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
            assert_random_insertions_stay_strictly_ordered(|| SpacePolicy::Random);
        }
    }

    #[test]
    fn test_sequential_policy_insertions_stay_strictly_ordered() {
        for _i in 1..10 {
            assert_random_insertions_stay_strictly_ordered(|| SpacePolicy::Sequential);
        }
    }

    #[test]
    fn test_rebalance() {
        struct TestCase {
            index: usize,
            expected: &'static str,
        }
        macro_rules! test_case {
            ($index:literal => $expected:literal) => {
                TestCase {
                    index: $index,
                    expected: $expected,
                }
            };
        }

        let first = Fracindex::default();
        let mut source_btree = BTreeSet::new();
        source_btree.insert(first.clone());

        // Very bad case: Construct an interval with very high density.
        let mut tmp = Fracindex::builder().after(&first).build().unwrap();
        for _ in 0..1023 {
            source_btree.insert(tmp.clone());
            tmp = Fracindex::builder().between(&first, &tmp).build().unwrap();
        }

        // Some Fracindex object needs many bytes to encode...
        let source = source_btree.into_iter().collect::<Vec<_>>();
        let max_bytes_len = source.iter().map(|i| i.bytes_len()).max().unwrap();
        assert_eq!(max_bytes_len, 136);

        // ...But we can rebalance the source.
        let result = Fracindex::rebalance(source.as_slice(), RebalancePolicy::PreserveBoth)
            .unwrap()
            .into_owned();
        let first = Fracindex::default();
        let last = Fracindex::builder().after(&first).build().unwrap();
        assert_eq!(result.len(), 1024);
        assert_eq!(result.first(), Some(&first));
        assert_eq!(result.last(), Some(&last));

        // After rebalancing, the max bytes we need is decreased a lot!
        let max_bytes_len = result.iter().map(|i| i.bytes_len()).max().unwrap();
        assert_eq!(max_bytes_len, 8);

        // Check the result.
        for i in 1..1024 {
            assert!(result[i - 1] < result[i]);
        }
        let test_cases = [
            test_case! { 0 => "7fff ffff 0000 0000" },
            test_case! { 1 => "7fff ffff 3fff ffff" },
            test_case! { 2 => "7fff ffff 7fff ffff" },
            test_case! { 3 => "7fff ffff bfff ffff" },
            test_case! { 4 => "8000 0000 0000 0000" },
            test_case! { 5 => "8000 0000 3fff ffff" },
            test_case! { 6 => "8000 0000 7fff ffff" },
            test_case! { 7 => "8000 0000 bfff ffff" },
            test_case! { 8 => "8000 0001 0000 0000" },
            test_case! { 9 => "8000 0001 3fff ffff" },
            test_case! { 510 => "8000 007e 7fff ffff" },
            test_case! { 511 => "8000 007e bfff ffff" },
            test_case! { 512 => "8000 007f 0000 0000" },
            test_case! { 513 => "8000 007f 3fff ffff" },
            test_case! { 514 => "8000 007f 7fff ffff" },
            test_case! { 1021 => "8000 00fe 3fff ffff" },
            test_case! { 1022 => "8000 00fe 7fff ffff" },
            test_case! { 1023 => "8000 00ff 0000 0000" },
        ];
        for test_case in test_cases {
            let expected = Fracindex::from_hex(test_case.expected).unwrap();
            assert_eq!(
                result[test_case.index], expected,
                "Expected {:?} but got {:?} for index {}",
                expected, result[test_case.index], test_case.index
            );
        }

        // If we do not care the first and last values, we can use another
        // policy to rebalance.
        let result = Fracindex::rebalance(source.as_slice(), RebalancePolicy::ReplaceAll).unwrap();
        let max_bytes_len = result.iter().map(|i| i.bytes_len()).max().unwrap();
        assert_eq!(max_bytes_len, 4);
        for i in 1..1024 {
            assert!(result[i - 1] < result[i]);
        }
        let test_cases = [
            test_case! { 0 => "7ffd ffff" },
            test_case! { 1 => "7ffe 00ff" },
            test_case! { 2 => "7ffe 01ff" },
            test_case! { 509 => "7fff fcff" },
            test_case! { 510 => "7fff fdff" },
            test_case! { 511 => "7fff feff" },
            test_case! { 512 => "7fff ffff" },
            test_case! { 513 => "8000 00ff" },
            test_case! { 514 => "8000 01ff" },
            test_case! { 515 => "8000 02ff" },
            test_case! { 1021 => "8001 fcff" },
            test_case! { 1022 => "8001 fdff" },
            test_case! { 1023 => "8001 feff" },
        ];
        for test_case in test_cases {
            let expected = Fracindex::from_hex(test_case.expected).unwrap();
            assert_eq!(
                result[test_case.index], expected,
                "Expected {:?} but got {:?} for index {}",
                expected, result[test_case.index], test_case.index
            );
        }

        // We can also rebalance with the other 2 policies.
        let result =
            Fracindex::rebalance(source.as_slice(), RebalancePolicy::PreserveFirst).unwrap();
        let max_bytes_len = result.iter().map(|i| i.bytes_len()).max().unwrap();
        assert_eq!(max_bytes_len, 4);
        for i in 1..1024 {
            assert!(result[i - 1] < result[i]);
        }
        let test_cases = [
            test_case! { 0 => "7fff ffff" },
            test_case! { 1 => "8000 00ff" },
            test_case! { 2 => "8000 01ff" },
            test_case! { 3 => "8000 02ff" },
            test_case! { 4 => "8000 03ff" },
            test_case! { 1023 => "8003 feff" },
        ];
        for test_case in test_cases {
            let expected = Fracindex::from_hex(test_case.expected).unwrap();
            assert_eq!(
                result[test_case.index], expected,
                "Expected {:?} but got {:?} for index {}",
                expected, result[test_case.index], test_case.index
            );
        }

        let result =
            Fracindex::rebalance(source.as_slice(), RebalancePolicy::PreserveLast).unwrap();
        let max_bytes_len = result.iter().map(|i| i.bytes_len()).max().unwrap();
        assert_eq!(max_bytes_len, 4);
        for i in 1..1024 {
            assert!(result[i - 1] < result[i]);
        }
        let test_cases = [
            test_case! { 0 => "7ffc 01ff" },
            test_case! { 1019 => "7fff fcff" },
            test_case! { 1020 => "7fff fdff" },
            test_case! { 1021 => "7fff feff" },
            test_case! { 1022 => "7fff ffff" },
            test_case! { 1023 => "8000 00ff" },
        ];
        for test_case in test_cases {
            let expected = Fracindex::from_hex(test_case.expected).unwrap();
            assert_eq!(
                result[test_case.index], expected,
                "Expected {:?} but got {:?} for index {}",
                expected, result[test_case.index], test_case.index
            );
        }

        // We can also use the different SpacePolicy.
        let result = Fracindex::rebalance_with_policy(
            source.as_slice(),
            RebalancePolicy::PreserveFirst,
            SpacePolicy::Random,
        )
        .unwrap();
        let max_bytes_len = result.iter().map(|i| i.bytes_len()).max().unwrap();
        assert_eq!(max_bytes_len, 4);
        for i in 1..1024 {
            assert!(result[i - 1] < result[i]);
        }
        let test_cases = [
            test_case! { 0 => "7fff ffff" },
            test_case! { 1 => "801f ffff" },
            test_case! { 2 => "803f ffff" },
            test_case! { 3 => "805f ffff" },
            test_case! { 4 => "807f ffff" },
            test_case! { 1022 => "ffbf ffff" },
            test_case! { 1023 => "ffdf ffff" },
        ];
        for test_case in test_cases {
            let expected = Fracindex::from_hex(test_case.expected).unwrap();
            assert_eq!(
                result[test_case.index], expected,
                "Expected {:?} but got {:?} for index {}",
                expected, result[test_case.index], test_case.index
            );
        }

        let result = Fracindex::rebalance_with_policy(
            source.as_slice(),
            RebalancePolicy::PreserveLast,
            SpacePolicy::Random,
        )
        .unwrap();
        let max_bytes_len = result.iter().map(|i| i.bytes_len()).max().unwrap();
        assert_eq!(max_bytes_len, 4);
        for i in 1..1024 {
            assert!(result[i - 1] < result[i]);
        }
        let test_cases = [
            test_case! { 0 => "0020 0000" },
            test_case! { 1 => "0040 0000" },
            test_case! { 1019 => "7f80 00fe" },
            test_case! { 1020 => "7fa0 00fe" },
            test_case! { 1021 => "7fc0 00fe" },
            test_case! { 1022 => "7fe0 00fe" },
            test_case! { 1023 => "8000 00ff" },
        ];
        for test_case in test_cases {
            let expected = Fracindex::from_hex(test_case.expected).unwrap();
            assert_eq!(
                result[test_case.index], expected,
                "Expected {:?} but got {:?} for index {}",
                expected, result[test_case.index], test_case.index
            );
        }
    }

    #[test]
    fn test_builder_batch_between() {
        let a = Fracindex::default();
        let b = Fracindex::builder().after(&a).build().unwrap();
        let result = Fracindex::builder()
            .between(&a, &b)
            .batch_build(10)
            .unwrap();
        assert_eq!(result.len(), 10);
        for i in 1..10 {
            assert!(result[i - 1] < result[i]);
        }
        assert_eq!(result[0], Fracindex::from_hex("8000 000f").unwrap());
        assert_eq!(result[9], Fracindex::from_hex("8000 00df").unwrap());
    }
}
