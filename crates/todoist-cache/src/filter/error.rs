//! Error types for the filter parser.

use thiserror::Error;

/// A specialized Result type for filter parsing operations.
pub type FilterResult<T> = Result<T, FilterError>;

/// Errors that can occur during filter parsing.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum FilterError {
    /// The filter expression is empty.
    #[error("filter expression is empty")]
    EmptyExpression,

    /// An unexpected token was encountered during parsing.
    #[error("unexpected token: {token}")]
    UnexpectedToken {
        /// The unexpected token that was encountered.
        token: String,
    },

    /// An unexpected end of input was encountered.
    #[error("unexpected end of expression")]
    UnexpectedEndOfInput,

    /// An invalid priority value was specified.
    #[error("invalid priority: {value} (expected 1-4)")]
    InvalidPriority {
        /// The invalid priority value.
        value: String,
    },

    /// An unclosed parenthesis was found.
    #[error("unclosed parenthesis")]
    UnclosedParenthesis,

    /// An invalid filter keyword was used.
    #[error("unknown filter keyword: {keyword}")]
    UnknownKeyword {
        /// The unrecognized keyword.
        keyword: String,
    },
}

impl FilterError {
    /// Creates an unexpected token error.
    pub fn unexpected_token(token: impl Into<String>) -> Self {
        FilterError::UnexpectedToken {
            token: token.into(),
        }
    }

    /// Creates an invalid priority error.
    pub fn invalid_priority(value: impl Into<String>) -> Self {
        FilterError::InvalidPriority {
            value: value.into(),
        }
    }

    /// Creates an unknown keyword error.
    pub fn unknown_keyword(keyword: impl Into<String>) -> Self {
        FilterError::UnknownKeyword {
            keyword: keyword.into(),
        }
    }
}
