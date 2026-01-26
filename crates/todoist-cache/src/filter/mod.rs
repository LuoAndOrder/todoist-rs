//! Filter expression parser for Todoist filter syntax.
//!
//! This module provides a parser for Todoist's filter syntax, allowing
//! local filtering of cached items without server round-trips.
//!
//! # Supported Syntax
//!
//! ## Date Keywords
//! - `today` - Items due today
//! - `tomorrow` - Items due tomorrow
//! - `overdue` - Items past their due date
//! - `no date` - Items without a due date
//!
//! ## Priority (coming soon)
//! - `p1`, `p2`, `p3`, `p4` - Filter by priority level
//!
//! ## Labels (coming soon)
//! - `@label` - Items with a specific label
//!
//! ## Projects (coming soon)
//! - `#project` - Items in a specific project
//! - `##project` - Items in a project or its subprojects
//!
//! ## Boolean Operators (coming soon)
//! - `&` - AND
//! - `|` - OR
//! - `!` - NOT
//! - `()` - Grouping
//!
//! # Example
//!
//! ```
//! use todoist_cache::filter::{FilterParser, Filter};
//!
//! let filter = FilterParser::parse("today").unwrap();
//! assert!(matches!(filter, Filter::Today));
//!
//! let filter = FilterParser::parse("no date").unwrap();
//! assert!(matches!(filter, Filter::NoDate));
//! ```

mod ast;
mod error;
mod lexer;
mod parser;

pub use ast::Filter;
pub use error::{FilterError, FilterResult};
pub use parser::FilterParser;

#[cfg(test)]
mod tests;
