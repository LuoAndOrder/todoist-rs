//! Prelude module for convenient imports.
//!
//! This module re-exports the most commonly used types from the todoist-api crate,
//! making it easy for library consumers to import everything they need with a single
//! use statement.
//!
//! # Example
//!
//! ```
//! use todoist_api_rs::prelude::*;
//!
//! // Now you have access to:
//! // - TodoistClient, TodoistClientBuilder (API client)
//! // - Error, ApiError, Result (error handling)
//! // - SyncRequest, SyncResponse, SyncCommand (sync API)
//! // - QuickAddRequest, QuickAddResponse (quick add API)
//! // - Item, Project, Label, Section, etc. (data models)
//! ```

// Client types
pub use crate::client::{TodoistClient, TodoistClientBuilder};

// Error types
pub use crate::error::{ApiError, Error, Result};

// Sync API types
pub use crate::sync::{
    // Data models
    Collaborator,
    CollaboratorState,
    // Response
    CommandError,
    CommandResult,
    FileAttachment,
    Filter,
    Item,
    Label,
    Note,
    Project,
    ProjectNote,
    Reminder,
    Section,
    // Request
    SyncCommand,
    SyncRequest,
    SyncResponse,
    User,
};

// Quick Add types
pub use crate::quick_add::{QuickAddRequest, QuickAddResponse};

// Common model types
pub use crate::models::{Deadline, Due, Duration, DurationUnit, LocationTrigger, ReminderType};
