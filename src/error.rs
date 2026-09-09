use std::{
    collections::BTreeMap,
    error::Error,
    fmt::{Debug, Display},
};

/// Error returned by fallible fractional-index operations.
///
/// The error carries a stable [`FracindexErrorKind`] plus a formatted diagnostic
/// message. Additional context can be attached with
/// [`FracindexError::with_context`].
pub struct FracindexError {
    kind: FracindexErrorKind,
    context: BTreeMap<String, String>,
    message: String,
}

impl Debug for FracindexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)?;
        if !self.context.is_empty() {
            write!(f, " (")?;
            let mut first = true;
            for (key, value) in &self.context {
                if !first {
                    write!(f, ", ")?;
                    first = false;
                }
                write!(f, "{}={}", key, value)?;
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

impl Display for FracindexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for FracindexError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl FracindexError {
    pub(crate) fn new<M>(kind: FracindexErrorKind, message: M) -> Self
    where
        M: Display,
    {
        Self {
            kind,
            context: BTreeMap::new(),
            message: message.to_string(),
        }
    }

    /// Returns the error kind.
    pub fn kind(&self) -> FracindexErrorKind {
        self.kind
    }

    /// Adds a context key-value pair to the error.
    pub fn with_context(mut self, key: impl Display, value: impl Display) -> Self {
        self.context.insert(key.to_string(), value.to_string());
        self
    }
}

/// Machine-readable category for a [`FracindexError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FracindexErrorKind {
    /// The provided input or bounds are not valid for the requested operation.
    Invalid,
}
