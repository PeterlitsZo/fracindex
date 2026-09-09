use std::borrow::Cow;

use smallvec::SmallVec;

use crate::{
    DEFAULT, Fracindex, FracindexError, FracindexErrorKind, FracindexResult, GAP, MAX, MIN,
    RebalancePolicy,
};

/// Controls how space is allocated when generating an index before or after
/// another index.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpacePolicy {
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

/// Builder for creating one or more [`Fracindex`] values.
///
/// The builder can create an index after a lower bound, before an upper bound,
/// between two bounds, or with no bounds. With no bounds, it produces the
/// default index.
#[derive(Clone, Copy, Debug, Default)]
pub struct FracindexBuilder<'a> {
    after: Option<&'a Fracindex>,
    before: Option<&'a Fracindex>,
    space_policy: SpacePolicy,
}

impl<'a> FracindexBuilder<'a> {
    /// Sets the lower bound that generated indexes must sort after.
    pub fn after(mut self, after: &'a Fracindex) -> Self {
        self.after = Some(after);
        self
    }

    /// Sets the upper bound that generated indexes must sort before.
    pub fn before(mut self, before: &'a Fracindex) -> Self {
        self.before = Some(before);
        self
    }

    /// Sets both bounds that generated indexes must sort between.
    pub fn between(mut self, after: &'a Fracindex, before: &'a Fracindex) -> Self {
        self.after = Some(after);
        self.before = Some(before);
        self
    }

    /// Sets the policy used to allocate space near open bounds.
    pub fn space_policy(mut self, space_policy: SpacePolicy) -> Self {
        self.space_policy = space_policy;
        self
    }

    /// Builds one fractional index using the configured bounds and policy.
    pub fn build(self) -> FracindexResult<Fracindex> {
        match (self.after, self.before) {
            (None, None) => Ok(Fracindex::default()),
            (Some(after), None) => Ok(after_with_policy(after, self.space_policy)),
            (None, Some(before)) => Ok(before_with_policy(before, self.space_policy)),
            (Some(after), Some(before)) => between(after, before),
        }
    }

    /// Builds `count` fractional indexes using the configured bounds and policy.
    ///
    /// The returned indexes are sorted in ascending order.
    pub fn batch_build(self, count: usize) -> FracindexResult<Vec<Fracindex>> {
        match (self.after, self.before) {
            (None, None) => {
                let placeholder = vec![Fracindex::default(); count];
                Fracindex::rebalance_with_policy(
                    &placeholder,
                    RebalancePolicy::ReplaceAll,
                    self.space_policy,
                )
                .map(Cow::into_owned)
            }
            (Some(after), None) => Ok(batch_after(after, count, self.space_policy)),
            (None, Some(before)) => Ok(batch_before(before, count, self.space_policy)),
            (Some(after), Some(before)) => batch_between(after, before, count),
        }
    }
}

fn after_with_policy(other: &Fracindex, policy: SpacePolicy) -> Fracindex {
    let other_inner = &other.inner;
    let other_len = other_inner.len();
    // The final component must not be zero in a valid fracindex.
    debug_assert_ne!(other_inner[other_len - 1], MIN);

    for i in 0..other_inner.len() {
        use SpacePolicy::*;

        let value = other_inner[i];
        let get_midpoint_to_max = || {
            let distance = MAX - value;
            let mut inner = SmallVec::from_slice(&other_inner[..=i]);
            inner[i] = value + distance / 2 + distance % 2;
            Fracindex { inner }
        };
        match &policy {
            Sequential if value <= MAX - GAP * 2 => {
                let mut inner = SmallVec::from_slice(&other_inner[..=i]);
                inner[i] += GAP;
                return Fracindex { inner };
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
        SpacePolicy::Random => DEFAULT,
        SpacePolicy::Sequential => GAP,
    });
    Fracindex { inner }
}

fn before_with_policy(other: &Fracindex, policy: SpacePolicy) -> Fracindex {
    let other_inner = &other.inner;
    let other_len = other_inner.len();
    // The final component must not be zero in a valid fracindex.
    debug_assert_ne!(other_inner[other_len - 1], MIN);

    for i in 0..other_inner.len() {
        use SpacePolicy::*;

        let value = other_inner[i];
        let get_midpoint_to_min = || {
            let mut inner = SmallVec::from_slice(&other_inner[..=i]);
            inner[i] /= 2;
            Fracindex { inner }
        };
        match &policy {
            Sequential if value >= GAP * 2 => {
                let mut inner = SmallVec::from_slice(&other_inner[..=i]);
                inner[i] -= GAP;
                return Fracindex { inner };
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
        SpacePolicy::Random => DEFAULT,
        SpacePolicy::Sequential => MAX,
    });
    Fracindex { inner }
}

fn between(a: &Fracindex, b: &Fracindex) -> FracindexResult<Fracindex> {
    let a_len = a.inner.len();
    let b_len = b.inner.len();
    let min_len = a_len.min(b_len);

    for i in 0..min_len {
        // Handle a zero component in `b` separately to avoid underflow.
        if b.inner[i] == MIN {
            if a.inner[i] != MIN {
                let err = FracindexError::new(
                    FracindexErrorKind::Invalid,
                    "a MUST be before or equal to b to find a fractional index between them",
                )
                .with_context("a", format!("{a:?}"))
                .with_context("b", format!("{b:?}"));
                return Err(err);
            }
            continue;
        }

        // If the `i` part is not adjacent to `b`, find the midpoint directly.
        if a.inner[i] < b.inner[i] - 1 {
            let mut inner = SmallVec::from_slice(&a.inner[..=i]);
            inner[i] = a.inner[i] + (b.inner[i] - a.inner[i]) / 2;
            return Ok(Fracindex { inner });
        }

        if a.inner[i] == b.inner[i] - 1 {
            // A value with this prefix remains below `b`. Increase the first
            // non-MAX suffix component in `a`, or append a component.
            if let Some(ai) = (i + 1..a_len).find(|&ai| a.inner[ai] != MAX) {
                let value = a.inner[ai];
                let distance = MAX - value;
                let mut inner = SmallVec::from_slice(&a.inner[..=ai]);
                inner[ai] = value + distance / 2 + distance % 2;
                return Ok(Fracindex { inner });
            }

            let mut inner = SmallVec::with_capacity(a_len + 1);
            inner.extend_from_slice(&a.inner);
            inner.push(DEFAULT);
            return Ok(Fracindex { inner });
        }

        if a.inner[i] > b.inner[i] {
            let err = FracindexError::new(
                FracindexErrorKind::Invalid,
                "a MUST be before or equal to b to find a fractional index between them",
            )
            .with_context("a", format!("{a:?}"))
            .with_context("b", format!("{b:?}"));
            return Err(err);
        }
    }

    // A longer `a` with the same prefix is ordered after `b`.
    if a_len != min_len {
        let err = FracindexError::new(
            FracindexErrorKind::Invalid,
            "a MUST be before or equal to b to find a fractional index between them",
        )
        .with_context("a", format!("{a:?}"))
        .with_context("b", format!("{b:?}"));
        return Err(err);
    }

    if b_len == min_len {
        return Ok(a.clone());
    }

    // `a` is a strict prefix of `b`. Move the final component of `b` towards
    // MIN; if that would become zero, extend the index instead.
    let mut inner = SmallVec::with_capacity(b_len + 1);
    inner.extend_from_slice(&b.inner);
    if inner[b_len - 1] >= 2 {
        inner[b_len - 1] /= 2;
    } else {
        inner[b_len - 1] = MIN;
        inner.push(DEFAULT);
    }
    Ok(Fracindex { inner })
}

fn batch_between(a: &Fracindex, b: &Fracindex, count: usize) -> FracindexResult<Vec<Fracindex>> {
    fn inner(
        sink: &mut Vec<Fracindex>,
        a: &Fracindex,
        b: &Fracindex,
        start: usize,
        end: usize,
    ) -> FracindexResult<()> {
        if start >= end {
            return Ok(());
        }
        if end - start == 1 {
            sink[start] = between(a, b)?;
            return Ok(());
        }

        let mid = (start + end) / 2;
        let mid_val = between(a, b)?;
        inner(sink, a, &mid_val, start, mid)?;
        inner(sink, &mid_val, b, mid + 1, end)?;
        sink[mid] = mid_val;

        Ok(())
    }

    let mut result = vec![Fracindex::default(); count];
    inner(&mut result, a, b, 0, count)?;
    Ok(result)
}

fn batch_after(after: &Fracindex, count: usize, space_policy: SpacePolicy) -> Vec<Fracindex> {
    let mut result = Vec::with_capacity(count);
    if count == 0 {
        return result;
    }

    let mut current = after_with_policy(after, space_policy);
    for _ in 0..count {
        let next = after_with_policy(&current, space_policy);
        result.push(current);
        current = next;
    }
    result
}

fn batch_before(before: &Fracindex, count: usize, space_policy: SpacePolicy) -> Vec<Fracindex> {
    let mut result = Vec::with_capacity(count);
    if count == 0 {
        return result;
    }

    let mut current = before_with_policy(before, space_policy);
    for _ in 0..count {
        let next = before_with_policy(&current, space_policy);
        result.push(current);
        current = next;
    }
    result.reverse();
    result
}
