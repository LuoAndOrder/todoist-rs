//! Lexer (tokenizer) for filter expressions.

use std::iter::Peekable;
use std::str::Chars;

/// Error encountered during lexical analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexerError {
    /// The character that could not be tokenized.
    pub character: char,
    /// The position (0-indexed byte offset) where the error occurred.
    pub position: usize,
}

impl std::fmt::Display for LexerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unexpected character '{}' at position {}",
            self.character, self.position
        )
    }
}

impl std::error::Error for LexerError {}

/// Result of tokenizing a filter expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexerResult {
    /// The tokens successfully parsed, with their positions.
    pub tokens: Vec<PositionedToken>,
    /// Any errors encountered (unknown characters).
    pub errors: Vec<LexerError>,
}

/// A token with its position in the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionedToken {
    /// The token.
    pub token: FilterToken,
    /// The byte position where the token starts (0-indexed).
    pub position: usize,
}

/// A token in a filter expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterToken {
    // ==================== Date Keywords ====================
    /// The `today` keyword.
    Today,

    /// The `tomorrow` keyword.
    Tomorrow,

    /// The `overdue` keyword.
    Overdue,

    /// The `no date` keyword (parsed as two words).
    NoDate,

    // ==================== Priority ====================
    /// Priority level (1-4).
    Priority(u8),

    // ==================== Identifiers ====================
    /// A label reference (prefixed with @).
    Label(String),

    /// A project reference (prefixed with #).
    Project(String),

    /// A project reference including subprojects (prefixed with ##).
    ProjectWithSubprojects(String),

    /// A section reference (prefixed with /).
    Section(String),

    // ==================== Operators ====================
    /// The AND operator (`&`).
    And,

    /// The OR operator (`|`).
    Or,

    /// The NOT operator (`!`).
    Not,

    /// Opening parenthesis `(`.
    OpenParen,

    /// Closing parenthesis `)`.
    CloseParen,
}

/// Lexer for tokenizing filter expressions.
pub struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
    /// Current byte position in the input string.
    position: usize,
    /// Errors encountered during tokenization.
    errors: Vec<LexerError>,
}

impl<'a> Lexer<'a> {
    /// Creates a new lexer for the given input string.
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
            position: 0,
            errors: Vec::new(),
        }
    }

    /// Peeks at the next character without consuming it.
    fn peek(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    /// Consumes and returns the next character, updating position.
    fn next_char(&mut self) -> Option<char> {
        let c = self.chars.next();
        if let Some(ch) = c {
            self.position += ch.len_utf8();
        }
        c
    }

    /// Returns the current position (for error reporting).
    fn current_position(&self) -> usize {
        self.position
    }

    /// Skips whitespace characters.
    fn skip_whitespace(&mut self) {
        while let Some(&c) = self.peek() {
            if c.is_whitespace() {
                self.next_char();
            } else {
                break;
            }
        }
    }

    /// Reads an identifier (alphanumeric word).
    fn read_identifier(&mut self) -> String {
        let mut ident = String::new();
        while let Some(&c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                ident.push(self.next_char().unwrap());
            } else {
                break;
            }
        }
        ident
    }

    /// Reads a quoted string (single or double quotes).
    fn read_quoted_string(&mut self, quote_char: char) -> String {
        // Consume the opening quote
        self.next_char();

        let mut result = String::new();
        while let Some(c) = self.next_char() {
            if c == quote_char {
                break;
            }
            // Handle escape sequences
            if c == '\\' {
                if let Some(escaped) = self.next_char() {
                    result.push(escaped);
                }
            } else {
                result.push(c);
            }
        }
        result
    }

    /// Reads a project/label/section name (after the prefix).
    fn read_name(&mut self) -> String {
        // Check for quoted string
        if let Some(&c) = self.peek() {
            if c == '"' || c == '\'' {
                return self.read_quoted_string(c);
            }
        }

        // Otherwise read until whitespace or operator
        let mut name = String::new();
        while let Some(&c) = self.peek() {
            if c.is_whitespace() || c == '&' || c == '|' || c == ')' || c == '(' {
                break;
            }
            name.push(self.next_char().unwrap());
        }
        name
    }

    /// Returns the next token with its position, or None if at end of input.
    pub fn next_token(&mut self) -> Option<PositionedToken> {
        self.skip_whitespace();

        let c = *self.peek()?;
        let token_start = self.current_position();

        match c {
            // Operators
            '&' => {
                self.next_char();
                Some(PositionedToken {
                    token: FilterToken::And,
                    position: token_start,
                })
            }
            '|' => {
                self.next_char();
                Some(PositionedToken {
                    token: FilterToken::Or,
                    position: token_start,
                })
            }
            '!' => {
                self.next_char();
                Some(PositionedToken {
                    token: FilterToken::Not,
                    position: token_start,
                })
            }
            '(' => {
                self.next_char();
                Some(PositionedToken {
                    token: FilterToken::OpenParen,
                    position: token_start,
                })
            }
            ')' => {
                self.next_char();
                Some(PositionedToken {
                    token: FilterToken::CloseParen,
                    position: token_start,
                })
            }

            // Label reference
            '@' => {
                self.next_char();
                let name = self.read_name();
                Some(PositionedToken {
                    token: FilterToken::Label(name),
                    position: token_start,
                })
            }

            // Project reference (# or ##)
            '#' => {
                self.next_char();
                // Check for ## (with subprojects)
                if self.peek() == Some(&'#') {
                    self.next_char();
                    let name = self.read_name();
                    Some(PositionedToken {
                        token: FilterToken::ProjectWithSubprojects(name),
                        position: token_start,
                    })
                } else {
                    let name = self.read_name();
                    Some(PositionedToken {
                        token: FilterToken::Project(name),
                        position: token_start,
                    })
                }
            }

            // Section reference
            '/' => {
                self.next_char();
                let name = self.read_name();
                Some(PositionedToken {
                    token: FilterToken::Section(name),
                    position: token_start,
                })
            }

            // Priority (p1, p2, p3, p4)
            'p' | 'P' => {
                let ident = self.read_identifier();
                let lower = ident.to_lowercase();
                match lower.as_str() {
                    "p1" => Some(PositionedToken {
                        token: FilterToken::Priority(1),
                        position: token_start,
                    }),
                    "p2" => Some(PositionedToken {
                        token: FilterToken::Priority(2),
                        position: token_start,
                    }),
                    "p3" => Some(PositionedToken {
                        token: FilterToken::Priority(3),
                        position: token_start,
                    }),
                    "p4" => Some(PositionedToken {
                        token: FilterToken::Priority(4),
                        position: token_start,
                    }),
                    _ => self.try_keyword(&lower, token_start),
                }
            }

            // Keywords and identifiers
            _ if c.is_alphabetic() => {
                let ident = self.read_identifier();
                let lower = ident.to_lowercase();
                self.try_keyword(&lower, token_start)
            }

            // Unknown character - record error and continue
            _ => {
                let error_pos = self.current_position();
                let unknown_char = self.next_char().unwrap();
                self.errors.push(LexerError {
                    character: unknown_char,
                    position: error_pos,
                });
                self.next_token()
            }
        }
    }

    /// Tries to match a keyword, returns None if not recognized.
    fn try_keyword(&mut self, lower: &str, position: usize) -> Option<PositionedToken> {
        match lower {
            "today" => Some(PositionedToken {
                token: FilterToken::Today,
                position,
            }),
            "tomorrow" => Some(PositionedToken {
                token: FilterToken::Tomorrow,
                position,
            }),
            "overdue" => Some(PositionedToken {
                token: FilterToken::Overdue,
                position,
            }),
            "no" => {
                // Check for "no date"
                self.skip_whitespace();
                if let Some(&c) = self.peek() {
                    if c.is_alphabetic() {
                        let next_word = self.read_identifier();
                        if next_word.to_lowercase() == "date" {
                            return Some(PositionedToken {
                                token: FilterToken::NoDate,
                                position,
                            });
                        }
                    }
                }
                // Just "no" by itself is not valid, return None
                None
            }
            _ => None,
        }
    }

    /// Collects all tokens into a vector (without positions).
    ///
    /// This method is provided for backward compatibility with tests.
    /// For error reporting, use [`tokenize_with_errors`] instead.
    #[cfg(test)]
    pub fn tokenize(self) -> Vec<FilterToken> {
        self.tokenize_with_errors()
            .tokens
            .into_iter()
            .map(|pt| pt.token)
            .collect()
    }

    /// Collects all tokens and any errors encountered.
    ///
    /// Returns a [`LexerResult`] containing both the successfully parsed tokens
    /// (with positions) and any errors for unknown characters.
    pub fn tokenize_with_errors(mut self) -> LexerResult {
        let mut tokens = Vec::new();
        while let Some(positioned_token) = self.next_token() {
            tokens.push(positioned_token);
        }
        LexerResult {
            tokens,
            errors: self.errors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_today() {
        let tokens = Lexer::new("today").tokenize();
        assert_eq!(tokens, vec![FilterToken::Today]);
    }

    #[test]
    fn test_tokenize_tomorrow() {
        let tokens = Lexer::new("tomorrow").tokenize();
        assert_eq!(tokens, vec![FilterToken::Tomorrow]);
    }

    #[test]
    fn test_tokenize_overdue() {
        let tokens = Lexer::new("overdue").tokenize();
        assert_eq!(tokens, vec![FilterToken::Overdue]);
    }

    #[test]
    fn test_tokenize_no_date() {
        let tokens = Lexer::new("no date").tokenize();
        assert_eq!(tokens, vec![FilterToken::NoDate]);
    }

    #[test]
    fn test_tokenize_no_date_case_insensitive() {
        let tokens = Lexer::new("NO DATE").tokenize();
        assert_eq!(tokens, vec![FilterToken::NoDate]);

        let tokens = Lexer::new("No Date").tokenize();
        assert_eq!(tokens, vec![FilterToken::NoDate]);
    }

    #[test]
    fn test_tokenize_priority() {
        let tokens = Lexer::new("p1").tokenize();
        assert_eq!(tokens, vec![FilterToken::Priority(1)]);

        let tokens = Lexer::new("P2").tokenize();
        assert_eq!(tokens, vec![FilterToken::Priority(2)]);

        let tokens = Lexer::new("p3").tokenize();
        assert_eq!(tokens, vec![FilterToken::Priority(3)]);

        let tokens = Lexer::new("p4").tokenize();
        assert_eq!(tokens, vec![FilterToken::Priority(4)]);
    }

    #[test]
    fn test_tokenize_label() {
        let tokens = Lexer::new("@urgent").tokenize();
        assert_eq!(tokens, vec![FilterToken::Label("urgent".to_string())]);
    }

    #[test]
    fn test_tokenize_project() {
        let tokens = Lexer::new("#Work").tokenize();
        assert_eq!(tokens, vec![FilterToken::Project("Work".to_string())]);
    }

    #[test]
    fn test_tokenize_project_with_subprojects() {
        let tokens = Lexer::new("##Work").tokenize();
        assert_eq!(
            tokens,
            vec![FilterToken::ProjectWithSubprojects("Work".to_string())]
        );
    }

    #[test]
    fn test_tokenize_section() {
        let tokens = Lexer::new("/Inbox").tokenize();
        assert_eq!(tokens, vec![FilterToken::Section("Inbox".to_string())]);
    }

    #[test]
    fn test_tokenize_operators() {
        let tokens = Lexer::new("today & p1").tokenize();
        assert_eq!(
            tokens,
            vec![FilterToken::Today, FilterToken::And, FilterToken::Priority(1)]
        );

        let tokens = Lexer::new("today | overdue").tokenize();
        assert_eq!(
            tokens,
            vec![FilterToken::Today, FilterToken::Or, FilterToken::Overdue]
        );

        let tokens = Lexer::new("!no date").tokenize();
        assert_eq!(tokens, vec![FilterToken::Not, FilterToken::NoDate]);
    }

    #[test]
    fn test_tokenize_parentheses() {
        let tokens = Lexer::new("(today | overdue) & p1").tokenize();
        assert_eq!(
            tokens,
            vec![
                FilterToken::OpenParen,
                FilterToken::Today,
                FilterToken::Or,
                FilterToken::Overdue,
                FilterToken::CloseParen,
                FilterToken::And,
                FilterToken::Priority(1),
            ]
        );
    }

    #[test]
    fn test_tokenize_quoted_project() {
        let tokens = Lexer::new("#\"My Project\"").tokenize();
        assert_eq!(tokens, vec![FilterToken::Project("My Project".to_string())]);
    }

    #[test]
    fn test_tokenize_complex_expression() {
        let tokens = Lexer::new("(today | tomorrow) & @urgent & #Work").tokenize();
        assert_eq!(
            tokens,
            vec![
                FilterToken::OpenParen,
                FilterToken::Today,
                FilterToken::Or,
                FilterToken::Tomorrow,
                FilterToken::CloseParen,
                FilterToken::And,
                FilterToken::Label("urgent".to_string()),
                FilterToken::And,
                FilterToken::Project("Work".to_string()),
            ]
        );
    }
}
