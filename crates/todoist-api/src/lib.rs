//! Todoist API client library
//!
//! # Quick Start
//!
//! For convenient imports, use the prelude:
//!
//! ```
//! use todoist_api_rs::prelude::*;
//! ```
//!
//! This re-exports the most commonly used types including [`TodoistClient`],
//! error types, sync API types, and data models.

pub mod client;
pub mod error;
pub mod models;
pub mod prelude;
pub mod quick_add;
pub mod sync;
