use smallvec::{SmallVec, smallvec};

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct Fracindex {
    inner: SmallVec<[u8; 16]>,
}

impl Default for Fracindex {
    fn default() -> Self {
        Self {
            inner: smallvec![128],
        }
    }
}

impl Fracindex {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            // Empty bytes cannot represent a valid fracindex.
            return None;
        }

        let len = bytes.len();
        if bytes[len - 1] == 0 {
            // Trailing zero byte cannot represent a valid fracindex.
            return None;
        }

        Some(Self {
            inner: SmallVec::from_slice(bytes),
        })
    }

    pub fn new_after(other: &Self) -> Self {
        let other_bytes = other.to_bytes();
        for i in 0..other_bytes.len() {
            if other_bytes[i] < 255 {
                let mut inner = SmallVec::from_slice(other_bytes);
                inner[i] = inner[i] / 2 + 128;
                return Self { inner };
            }
        }
        let len = other.inner.len() + 1;
        let mut inner = SmallVec::with_capacity(len);
        inner.extend_from_slice(&other.inner);
        inner.push(128);
        Self { inner }
    }

    pub fn new_before(other: &Self) -> Self {
        let other_bytes = other.to_bytes();
        let other_len = other_bytes.len();
        // The last byte must not be zero (not a valid fracindex).
        debug_assert_ne!(other_bytes[other_len - 1], 0);
        for i in 0..other_bytes.len() {
            if other_bytes[i] > 1 {
                let mut inner = SmallVec::from_slice(&other_bytes[0..=i]);
                inner[i] /= 2;
                return Self { inner };
            }
        }
        let len = other.inner.len() + 1;
        let mut inner = SmallVec::with_capacity(len);
        inner.extend_from_slice(&other.inner);
        inner[len - 2] = 0;
        inner.push(128);
        Self { inner }
    }

    pub fn new_between(a: &Self, b: &Self) -> Option<Self> {
        let a_len = a.inner.len();
        let b_len = b.inner.len();
        let min_len = a_len.min(b_len);
        for i in 0..min_len {
            // Deal with the zero byte in `b` to avoid underflow.
            if b.inner[i] == 0 {
                if a.inner[i] != 0 {
                    // `a` is bigger than `b`.
                    return None;
                } else {
                    continue;
                }
            }

            if a.inner[i] < b.inner[i] - 1 {
                let mut inner = SmallVec::from_slice(&a.inner[..=i]);
                inner[i] = a.inner[i] + (b.inner[i] - a.inner[i]) / 2;
                return Some(Self { inner });
            } else if a.inner[i] == b.inner[i] - 1 {
                // Find the first non-255 byte in `a`.
                let mut a_index_not_255 = Some(i + 1);
                while let Some(ai) = a_index_not_255 {
                    if ai >= a_len {
                        a_index_not_255 = None;
                        break;
                    }
                    if a.inner[ai] != 255 {
                        break;
                    } else {
                        a_index_not_255 = Some(ai + 1);
                    }
                }

                match a_index_not_255 {
                    Some(ai) => {
                        let mut inner = SmallVec::with_capacity(ai + 1);
                        inner.extend_from_slice(&a.inner[..=ai]);
                        inner[ai] = inner[ai] / 2 + 128;
                        return Some(Self { inner });
                    }
                    None => {
                        let mut inner = SmallVec::with_capacity(a_len + 1);
                        inner.extend_from_slice(&a.inner);
                        inner.push(128);
                        return Some(Self { inner });
                    }
                }
            } else if a.inner[i] > b.inner[i] {
                // `a` is bigger than `b`...
                return None;
            }
        }

        if a.inner.len() != min_len || b.inner.len() == min_len {
            return None;
        }
        let b_len = b.inner.len();
        let (len, push_128) = {
            if b.inner[b_len - 1] >= 2 {
                (b_len, false)
            } else {
                (b_len + 1, true)
            }
        };
        let mut inner = SmallVec::with_capacity(len);
        inner.extend_from_slice(&b.inner);
        if push_128 {
            inner.push(128);
            inner[len - 2] -= 1;
        } else {
            inner[len - 1] /= 2;
        }
        Some(Self { inner })
    }

    pub fn to_bytes(&self) -> &[u8] {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let fracindex = Fracindex::default();
        assert_eq!(fracindex.to_bytes(), &[128]);
    }

    #[test]
    fn test_new_after() {
        let mut fracindex = Fracindex::default();
        let wanted: &[&[u8]] = &[
            &[128],
            &[192],
            &[224],
            &[240],
            &[248],
            &[252],
            &[254],
            &[255],
            &[255, 128],
            &[255, 192],
            &[255, 224],
            &[255, 240],
            &[255, 248],
            &[255, 252],
            &[255, 254],
            &[255, 255],
            &[255, 255, 128],
        ];
        for (i, expected) in wanted.iter().enumerate() {
            assert_eq!(fracindex.to_bytes(), *expected, "iteration {}", i);
            let next_fracindex = Fracindex::new_after(&fracindex);
            assert!(next_fracindex > fracindex, "iteration {}", i);
            fracindex = next_fracindex;
        }
    }

    #[test]
    fn test_new_before() {
        let mut fracindex = Fracindex::default();
        let wanted: &[&[u8]] = &[
            &[128],
            &[64],
            &[32],
            &[16],
            &[8],
            &[4],
            &[2],
            &[1],
            &[0, 128],
            &[0, 64],
            &[0, 32],
            &[0, 16],
            &[0, 8],
            &[0, 4],
            &[0, 2],
            &[0, 1],
            &[0, 0, 128],
        ];
        for (i, expected) in wanted.iter().enumerate() {
            assert_eq!(fracindex.to_bytes(), *expected, "iteration {}", i);
            let next_fracindex = Fracindex::new_before(&fracindex);
            assert!(next_fracindex < fracindex, "iteration {}", i);
            fracindex = next_fracindex;
        }
    }

    #[test]
    fn test_new_between() {
        struct TestCase {
            a: &'static [u8],
            b: &'static [u8],
            expected: Option<&'static [u8]>,
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
        let test_cases: &[TestCase] = &[
            test_case! { &[1], &[2] => Some(&[1, 128]) },
            test_case! { &[0, 128], &[128] => Some(&[64]) },
            test_case! { &[62], &[64] => Some(&[63]) },
            test_case! { &[63], &[64] => Some(&[63, 128]) },
            test_case! { &[63], &[63, 128] => Some(&[63, 64]) },
            test_case! { &[63], &[63] => None },
            test_case! { &[64], &[63] => None },
            test_case! { &[63], &[63, 1] => Some(&[63, 0, 128]) },
            test_case! { &[63], &[63, 2] => Some(&[63, 1]) },
            test_case! { &[0, 0, 1], &[0, 0, 3] => Some(&[0, 0, 2]) },
            test_case! { &[255, 255, 253], &[255, 255, 255] => Some(&[255, 255, 254]) },
            test_case! { &[64, 1], &[63, 3] => None },
            test_case! { &[1, 255], &[2] => Some(&[1, 255, 128]) },
            test_case! { &[1, 255, 255], &[2] => Some(&[1, 255, 255, 128]) },
            test_case! { &[1, 255, 254], &[2] => Some(&[1, 255, 255]) },
        ];
        for (i, test_case) in test_cases.iter().enumerate() {
            let a = Fracindex::from_bytes(test_case.a).unwrap();
            let b = Fracindex::from_bytes(test_case.b).unwrap();
            let result = Fracindex::new_between(&a, &b);
            if let Some(expected) = test_case.expected {
                if let Some(result) = result {
                    assert_eq!(result.to_bytes(), expected, "iteration {}", i);
                    assert!(a < result && result < b, "iteration {}", i);
                } else {
                    panic!("iteration {}", i);
                }
            } else {
                assert!(result.is_none(), "iteration {}", i);
            }
        }
    }
}
