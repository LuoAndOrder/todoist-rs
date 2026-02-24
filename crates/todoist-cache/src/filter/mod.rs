//! Filter expression parser and evaluator for Todoist filter syntax.
//!
//! This module provides a parser and evaluator for Todoist's filter syntax,
//! allowing local filtering of cached items without server round-trips.
//!
//! # Supported Syntax
//!
//! ## Date Keywords
//! - `today` - Items due today
//! - `tomorrow` - Items due tomorrow
//! - `overdue` - Items past their due date
//! - `no date` - Items without a due date
//!
//! ## Priority
//! - `p1`, `p2`, `p3`, `p4` - Filter by priority level
//!
//! ## Labels
//! - `@label` - Items with a specific label
//!
//! ## Projects
//! - `#project` - Items in a specific project
//! - `##project` - Items in a project or its subprojects
//!
//! ## Sections
//! - `/section` - Items in a specific section
//!
//! ## Boolean Operators
//! - `&` - AND
//! - `|` - OR
//! - `!` - NOT
//! - `()` - Grouping
//!
//! # Example
//!
//! ```
//! use todoist_cache_rs::filter::{FilterParser, Filter, FilterEvaluator, FilterContext};
//!
//! // Parse a filter expression
//! let filter = FilterParser::parse("today").unwrap();
//! assert!(matches!(filter, Filter::Today));
//!
//! // Create evaluation context
//! let context = FilterContext::new(&[], &[], &[]);
//!
//! // Create an evaluator
//! let evaluator = FilterEvaluator::new(&filter, &context);
//!
//! // Filter items (empty example)
//! let items: Vec<todoist_api_rs::sync::Item> = vec![];
//! let results = evaluator.filter_items(&items);
//! ```

mod ast;
mod error;
mod evaluator;
mod lexer;
mod parser;

pub use ast::{AssignedTarget, Filter};
pub use error::{FilterError, FilterResult};
pub use evaluator::{FilterContext, FilterEvaluator};
pub use parser::FilterParser;

#[cfg(test)]
mod tests;
