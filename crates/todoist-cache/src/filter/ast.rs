//! Abstract Syntax Tree (AST) for filter expressions.

/// Target for assignment filters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedTarget {
    /// The current user ("me").
    Me,
    /// Anyone other than the current user ("others").
    Others,
    /// A specific user by name.
    User(String),
}

/// Represents a parsed filter expression.
///
/// The `Filter` enum is the AST for Todoist filter expressions. Each variant
/// represents a different filter predicate or combination of predicates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Filter {
    // ==================== Date Filters ====================
    /// Matches items due today.
    Today,

    /// Matches items due tomorrow.
    Tomorrow,

    /// Matches items that are past their due date.
    Overdue,

    /// Matches items without any due date set.
    NoDate,

    /// Matches items due within the next 7 days (including today).
    Next7Days,

    /// Matches items due on a specific date (month and day).
    /// The year is inferred: if the date is in the past this year, it's next year.
    SpecificDate {
        /// Month (1-12)
        month: u32,
        /// Day (1-31)
        day: u32,
    },

    // ==================== Priority Filters ====================
    /// Matches items with priority level 1 (highest/red).
    Priority1,

    /// Matches items with priority level 2 (orange).
    Priority2,

    /// Matches items with priority level 3 (yellow).
    Priority3,

    /// Matches items with priority level 4 (lowest/blue, default).
    Priority4,

    // ==================== Label Filters ====================
    /// Matches items with the specified label.
    Label(String),

    /// Matches items without any labels.
    NoLabels,

    // ==================== Project Filters ====================
    /// Matches items in the specified project (exact match).
    Project(String),

    /// Matches items in the specified project or any of its subprojects.
    ProjectWithSubprojects(String),

    // ==================== Section Filter ====================
    /// Matches items in the specified section.
    Section(String),

    // ==================== Assignment Filters ====================
    /// Matches items assigned to someone.
    AssignedTo(AssignedTarget),

    /// Matches items assigned by someone.
    AssignedBy(AssignedTarget),

    /// Matches items that have any assignee.
    Assigned,

    /// Matches items that have no assignee.
    NoAssignee,

    // ==================== Boolean Operators ====================
    /// Logical AND of two filters.
    And(Box<Filter>, Box<Filter>),

    /// Logical OR of two filters.
    Or(Box<Filter>, Box<Filter>),

    /// Logical NOT of a filter.
    Not(Box<Filter>),
}

impl Filter {
    /// Creates an AND filter from two filters.
    ///
    /// # Example
    ///
    /// ```
    /// use todoist_cache_rs::filter::Filter;
    ///
    /// let filter = Filter::and(Filter::Today, Filter::Priority1);
    /// assert!(matches!(filter, Filter::And(_, _)));
    /// ```
    pub fn and(left: Filter, right: Filter) -> Self {
        Filter::And(Box::new(left), Box::new(right))
    }

    /// Creates an OR filter from two filters.
    ///
    /// # Example
    ///
    /// ```
    /// use todoist_cache_rs::filter::Filter;
    ///
    /// let filter = Filter::or(Filter::Today, Filter::Overdue);
    /// assert!(matches!(filter, Filter::Or(_, _)));
    /// ```
    pub fn or(left: Filter, right: Filter) -> Self {
        Filter::Or(Box::new(left), Box::new(right))
    }

    /// Creates a NOT filter from another filter.
    ///
    /// # Example
    ///
    /// ```
    /// use todoist_cache_rs::filter::Filter;
    ///
    /// let filter = Filter::negate(Filter::NoDate);
    /// assert!(matches!(filter, Filter::Not(_)));
    /// ```
    pub fn negate(inner: Filter) -> Self {
        Filter::Not(Box::new(inner))
    }
}
