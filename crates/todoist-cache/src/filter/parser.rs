//! Recursive descent parser for filter expressions.

use super::ast::Filter;
use super::error::{FilterError, FilterResult};
use super::lexer::{FilterToken, Lexer};

/// Parser for Todoist filter expressions.
///
/// This parser implements a recursive descent parser for the filter grammar.
/// It supports date keywords, priority filters, labels, projects, sections,
/// and boolean operators with proper precedence.
///
/// # Grammar
///
/// ```text
/// expression ::= or_expr
/// or_expr    ::= and_expr ("|" and_expr)*
/// and_expr   ::= unary_expr ("&" unary_expr)*
/// unary_expr ::= "!" unary_expr | primary
/// primary    ::= "(" expression ")" | keyword | identifier
/// keyword    ::= "today" | "tomorrow" | "overdue" | "no date"
///              | "p1" | "p2" | "p3" | "p4"
/// identifier ::= "@" name | "#" name | "##" name | "/" name
/// ```
///
/// # Operator Precedence (highest to lowest)
///
/// 1. `!` (NOT) - unary
/// 2. `&` (AND) - binary, left-associative
/// 3. `|` (OR) - binary, left-associative
///
/// # Example
///
/// ```
/// use todoist_cache::filter::{FilterParser, Filter};
///
/// // Simple keyword
/// let filter = FilterParser::parse("today").unwrap();
/// assert!(matches!(filter, Filter::Today));
///
/// // Boolean expression
/// let filter = FilterParser::parse("today | overdue").unwrap();
/// assert!(matches!(filter, Filter::Or(_, _)));
/// ```
pub struct FilterParser {
    tokens: Vec<FilterToken>,
    position: usize,
}

impl FilterParser {
    /// Parses a filter expression string into a Filter AST.
    ///
    /// # Arguments
    ///
    /// * `input` - The filter expression to parse
    ///
    /// # Returns
    ///
    /// Returns the parsed `Filter` on success, or a `FilterError` if the
    /// expression is invalid.
    ///
    /// # Errors
    ///
    /// Returns `FilterError::EmptyExpression` if the input is empty or contains
    /// no valid tokens.
    ///
    /// Returns `FilterError::UnexpectedToken` if an unexpected token is encountered.
    ///
    /// Returns `FilterError::UnclosedParenthesis` if parentheses are not balanced.
    pub fn parse(input: &str) -> FilterResult<Filter> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(FilterError::EmptyExpression);
        }

        let tokens = Lexer::new(trimmed).tokenize();
        if tokens.is_empty() {
            return Err(FilterError::EmptyExpression);
        }

        let mut parser = Self { tokens, position: 0 };
        let filter = parser.parse_expression()?;

        // Check that we consumed all tokens
        if parser.position < parser.tokens.len() {
            let remaining = &parser.tokens[parser.position];
            return Err(FilterError::unexpected_token(format!("{:?}", remaining)));
        }

        Ok(filter)
    }

    /// Returns the current token without consuming it.
    fn peek(&self) -> Option<&FilterToken> {
        self.tokens.get(self.position)
    }

    /// Consumes and returns the current token.
    fn advance(&mut self) -> Option<&FilterToken> {
        let token = self.tokens.get(self.position);
        if token.is_some() {
            self.position += 1;
        }
        token
    }

    /// Checks if the current token matches the expected token type.
    fn check(&self, expected: &FilterToken) -> bool {
        self.peek() == Some(expected)
    }

    /// Parses the top-level expression (OR expression).
    fn parse_expression(&mut self) -> FilterResult<Filter> {
        self.parse_or_expr()
    }

    /// Parses OR expressions: `and_expr ("|" and_expr)*`
    fn parse_or_expr(&mut self) -> FilterResult<Filter> {
        let mut left = self.parse_and_expr()?;

        while self.check(&FilterToken::Or) {
            self.advance(); // consume '|'
            let right = self.parse_and_expr()?;
            left = Filter::or(left, right);
        }

        Ok(left)
    }

    /// Parses AND expressions: `unary_expr ("&" unary_expr)*`
    fn parse_and_expr(&mut self) -> FilterResult<Filter> {
        let mut left = self.parse_unary_expr()?;

        while self.check(&FilterToken::And) {
            self.advance(); // consume '&'
            let right = self.parse_unary_expr()?;
            left = Filter::and(left, right);
        }

        Ok(left)
    }

    /// Parses unary expressions: `"!" unary_expr | primary`
    fn parse_unary_expr(&mut self) -> FilterResult<Filter> {
        if self.check(&FilterToken::Not) {
            self.advance(); // consume '!'
            let inner = self.parse_unary_expr()?;
            return Ok(Filter::negate(inner));
        }

        self.parse_primary()
    }

    /// Parses primary expressions: `"(" expression ")" | keyword | identifier`
    fn parse_primary(&mut self) -> FilterResult<Filter> {
        let token = self.advance().ok_or(FilterError::UnexpectedEndOfInput)?;

        match token.clone() {
            // Parenthesized expression
            FilterToken::OpenParen => {
                let inner = self.parse_expression()?;
                if !self.check(&FilterToken::CloseParen) {
                    return Err(FilterError::UnclosedParenthesis);
                }
                self.advance(); // consume ')'
                Ok(inner)
            }

            // Date keywords
            FilterToken::Today => Ok(Filter::Today),
            FilterToken::Tomorrow => Ok(Filter::Tomorrow),
            FilterToken::Overdue => Ok(Filter::Overdue),
            FilterToken::NoDate => Ok(Filter::NoDate),

            // Priority
            FilterToken::Priority(level) => match level {
                1 => Ok(Filter::Priority1),
                2 => Ok(Filter::Priority2),
                3 => Ok(Filter::Priority3),
                4 => Ok(Filter::Priority4),
                _ => Err(FilterError::invalid_priority(level.to_string())),
            },

            // Identifiers
            FilterToken::Label(name) => Ok(Filter::Label(name)),
            FilterToken::Project(name) => Ok(Filter::Project(name)),
            FilterToken::ProjectWithSubprojects(name) => Ok(Filter::ProjectWithSubprojects(name)),
            FilterToken::Section(name) => Ok(Filter::Section(name)),

            // Unexpected tokens
            FilterToken::And => Err(FilterError::unexpected_token("&")),
            FilterToken::Or => Err(FilterError::unexpected_token("|")),
            FilterToken::CloseParen => Err(FilterError::unexpected_token(")")),
            FilterToken::Not => Err(FilterError::unexpected_token("!")),
        }
    }
}
