//! Lexer (tokenizer) for filter expressions.

use std::iter::Peekable;
use std::str::Chars;

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
}

impl<'a> Lexer<'a> {
    /// Creates a new lexer for the given input string.
    pub fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().peekable(),
        }
    }

    /// Peeks at the next character without consuming it.
    fn peek(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    /// Consumes and returns the next character.
    fn next_char(&mut self) -> Option<char> {
        self.chars.next()
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

    /// Returns the next token, or None if at end of input.
    pub fn next_token(&mut self) -> Option<FilterToken> {
        self.skip_whitespace();

        let c = self.peek()?;

        match *c {
            // Operators
            '&' => {
                self.next_char();
                Some(FilterToken::And)
            }
            '|' => {
                self.next_char();
                Some(FilterToken::Or)
            }
            '!' => {
                self.next_char();
                Some(FilterToken::Not)
            }
            '(' => {
                self.next_char();
                Some(FilterToken::OpenParen)
            }
            ')' => {
                self.next_char();
                Some(FilterToken::CloseParen)
            }

            // Label reference
            '@' => {
                self.next_char();
                let name = self.read_name();
                Some(FilterToken::Label(name))
            }

            // Project reference (# or ##)
            '#' => {
                self.next_char();
                // Check for ## (with subprojects)
                if self.peek() == Some(&'#') {
                    self.next_char();
                    let name = self.read_name();
                    Some(FilterToken::ProjectWithSubprojects(name))
                } else {
                    let name = self.read_name();
                    Some(FilterToken::Project(name))
                }
            }

            // Section reference
            '/' => {
                self.next_char();
                let name = self.read_name();
                Some(FilterToken::Section(name))
            }

            // Priority (p1, p2, p3, p4)
            'p' | 'P' => {
                let ident = self.read_identifier();
                let lower = ident.to_lowercase();
                match lower.as_str() {
                    "p1" => Some(FilterToken::Priority(1)),
                    "p2" => Some(FilterToken::Priority(2)),
                    "p3" => Some(FilterToken::Priority(3)),
                    "p4" => Some(FilterToken::Priority(4)),
                    _ => self.try_keyword(&lower),
                }
            }

            // Keywords and identifiers
            _ if c.is_alphabetic() => {
                let ident = self.read_identifier();
                let lower = ident.to_lowercase();
                self.try_keyword(&lower)
            }

            // Unknown character - skip it
            _ => {
                self.next_char();
                self.next_token()
            }
        }
    }

    /// Tries to match a keyword, returns None if not recognized.
    fn try_keyword(&mut self, lower: &str) -> Option<FilterToken> {
        match lower {
            "today" => Some(FilterToken::Today),
            "tomorrow" => Some(FilterToken::Tomorrow),
            "overdue" => Some(FilterToken::Overdue),
            "no" => {
                // Check for "no date"
                self.skip_whitespace();
                if let Some(&c) = self.peek() {
                    if c.is_alphabetic() {
                        let next_word = self.read_identifier();
                        if next_word.to_lowercase() == "date" {
                            return Some(FilterToken::NoDate);
                        }
                    }
                }
                // Just "no" by itself is not valid, return None
                None
            }
            _ => None,
        }
    }

    /// Collects all tokens into a vector.
    pub fn tokenize(mut self) -> Vec<FilterToken> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token() {
            tokens.push(token);
        }
        tokens
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
