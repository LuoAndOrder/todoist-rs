//! Error types for the filter parser.

use super::lexer::LexerError;
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
    #[error("unexpected token '{token}' at position {position}")]
    UnexpectedToken {
        /// The unexpected token that was encountered.
        token: String,
        /// The byte position where the token was found (0-indexed).
        position: usize,
    },

    /// An unexpected end of input was encountered.
    #[error("unexpected end of expression after position {position}")]
    UnexpectedEndOfInput {
        /// The byte position where the input ended (0-indexed).
        position: usize,
    },

    /// An invalid priority value was specified.
    #[error("invalid priority '{value}' at position {position} (expected 1-4)")]
    InvalidPriority {
        /// The invalid priority value.
        value: String,
        /// The byte position where the priority was found (0-indexed).
        position: usize,
    },

    /// An unclosed parenthesis was found.
    #[error("unclosed parenthesis at position {position}")]
    UnclosedParenthesis {
        /// The byte position of the opening parenthesis (0-indexed).
        position: usize,
    },

    /// An invalid filter keyword was used.
    #[error("unknown filter keyword '{keyword}' at position {position}")]
    UnknownKeyword {
        /// The unrecognized keyword.
        keyword: String,
        /// The byte position where the keyword was found (0-indexed).
        position: usize,
    },

    /// Unknown characters were encountered during lexing.
    #[error("unknown character(s) in filter: {}", format_lexer_errors(.errors))]
    UnknownCharacters {
        /// The lexer errors for each unknown character.
        errors: Vec<LexerError>,
    },
}

/// Formats a list of lexer errors for display.
fn format_lexer_errors(errors: &[LexerError]) -> String {
    if errors.len() == 1 {
        format!("'{}' at position {}", errors[0].character, errors[0].position)
    } else {
        errors
            .iter()
            .map(|e| format!("'{}' at {}", e.character, e.position))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl FilterError {
    /// Creates an unexpected token error with position.
    pub fn unexpected_token(token: impl Into<String>, position: usize) -> Self {
        FilterError::UnexpectedToken {
            token: token.into(),
            position,
        }
    }

    /// Creates an unexpected end of input error with position.
    pub fn unexpected_end_of_input(position: usize) -> Self {
        FilterError::UnexpectedEndOfInput { position }
    }

    /// Creates an invalid priority error with position.
    pub fn invalid_priority(value: impl Into<String>, position: usize) -> Self {
        FilterError::InvalidPriority {
            value: value.into(),
            position,
        }
    }

    /// Creates an unclosed parenthesis error with position.
    pub fn unclosed_parenthesis(position: usize) -> Self {
        FilterError::UnclosedParenthesis { position }
    }

    /// Creates an unknown keyword error with position.
    pub fn unknown_keyword(keyword: impl Into<String>, position: usize) -> Self {
        FilterError::UnknownKeyword {
            keyword: keyword.into(),
            position,
        }
    }
}
