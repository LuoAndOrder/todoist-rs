//! Tests for the filter parser.

use super::*;

// ==================== Date Keyword Tests ====================

#[test]
fn test_parse_today() {
    let filter = FilterParser::parse("today").unwrap();
    assert_eq!(
        filter,
        Filter::Today,
        "parsing 'today' should produce Filter::Today"
    );
}

#[test]
fn test_parse_today_case_insensitive() {
    assert_eq!(
        FilterParser::parse("TODAY").unwrap(),
        Filter::Today,
        "'TODAY' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("Today").unwrap(),
        Filter::Today,
        "'Today' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("ToDAy").unwrap(),
        Filter::Today,
        "'ToDAy' should be case-insensitive"
    );
}

#[test]
fn test_parse_today_with_whitespace() {
    assert_eq!(
        FilterParser::parse("  today  ").unwrap(),
        Filter::Today,
        "leading/trailing spaces should be trimmed"
    );
    assert_eq!(
        FilterParser::parse("\ttoday\n").unwrap(),
        Filter::Today,
        "tabs and newlines should be trimmed"
    );
}

#[test]
fn test_parse_tomorrow() {
    let filter = FilterParser::parse("tomorrow").unwrap();
    assert_eq!(
        filter,
        Filter::Tomorrow,
        "parsing 'tomorrow' should produce Filter::Tomorrow"
    );
}

#[test]
fn test_parse_tomorrow_case_insensitive() {
    assert_eq!(
        FilterParser::parse("TOMORROW").unwrap(),
        Filter::Tomorrow,
        "'TOMORROW' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("Tomorrow").unwrap(),
        Filter::Tomorrow,
        "'Tomorrow' should be case-insensitive"
    );
}

#[test]
fn test_parse_overdue() {
    let filter = FilterParser::parse("overdue").unwrap();
    assert_eq!(
        filter,
        Filter::Overdue,
        "parsing 'overdue' should produce Filter::Overdue"
    );
}

#[test]
fn test_parse_overdue_case_insensitive() {
    assert_eq!(
        FilterParser::parse("OVERDUE").unwrap(),
        Filter::Overdue,
        "'OVERDUE' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("Overdue").unwrap(),
        Filter::Overdue,
        "'Overdue' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("OverDue").unwrap(),
        Filter::Overdue,
        "'OverDue' should be case-insensitive"
    );
}

#[test]
fn test_parse_no_date() {
    let filter = FilterParser::parse("no date").unwrap();
    assert_eq!(
        filter,
        Filter::NoDate,
        "parsing 'no date' should produce Filter::NoDate"
    );
}

#[test]
fn test_parse_no_date_case_insensitive() {
    assert_eq!(
        FilterParser::parse("NO DATE").unwrap(),
        Filter::NoDate,
        "'NO DATE' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("No Date").unwrap(),
        Filter::NoDate,
        "'No Date' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("no DATE").unwrap(),
        Filter::NoDate,
        "'no DATE' should be case-insensitive"
    );
}

#[test]
fn test_parse_no_date_with_extra_whitespace() {
    // Multiple spaces between "no" and "date"
    assert_eq!(
        FilterParser::parse("no   date").unwrap(),
        Filter::NoDate,
        "multiple spaces between 'no' and 'date' should be allowed"
    );
    assert_eq!(
        FilterParser::parse("no\tdate").unwrap(),
        Filter::NoDate,
        "tab between 'no' and 'date' should be allowed"
    );
}

// ==================== No Labels Tests ====================

#[test]
fn test_parse_no_labels() {
    let filter = FilterParser::parse("no labels").unwrap();
    assert_eq!(
        filter,
        Filter::NoLabels,
        "parsing 'no labels' should produce Filter::NoLabels"
    );
}

#[test]
fn test_parse_no_labels_case_insensitive() {
    assert_eq!(
        FilterParser::parse("NO LABELS").unwrap(),
        Filter::NoLabels,
        "'NO LABELS' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("No Labels").unwrap(),
        Filter::NoLabels,
        "'No Labels' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("no LABELS").unwrap(),
        Filter::NoLabels,
        "'no LABELS' should be case-insensitive"
    );
}

#[test]
fn test_parse_no_labels_with_extra_whitespace() {
    // Multiple spaces between "no" and "labels"
    assert_eq!(
        FilterParser::parse("no   labels").unwrap(),
        Filter::NoLabels,
        "multiple spaces between 'no' and 'labels' should be allowed"
    );
    assert_eq!(
        FilterParser::parse("no\tlabels").unwrap(),
        Filter::NoLabels,
        "tab between 'no' and 'labels' should be allowed"
    );
}

#[test]
fn test_parse_no_labels_with_operators() {
    // Combined with other filters
    let filter = FilterParser::parse("no labels & p1").unwrap();
    assert_eq!(filter, Filter::and(Filter::NoLabels, Filter::Priority1));

    let filter = FilterParser::parse("no labels | today").unwrap();
    assert_eq!(filter, Filter::or(Filter::NoLabels, Filter::Today));
}

#[test]
fn test_parse_no_labels_negation() {
    // "!no labels" should match tasks that HAVE labels
    let filter = FilterParser::parse("!no labels").unwrap();
    assert_eq!(filter, Filter::negate(Filter::NoLabels));
}

// ==================== Specific Date Tests ====================

#[test]
fn test_parse_specific_date_short_month() {
    let filter = FilterParser::parse("Jan 15").unwrap();
    assert_eq!(filter, Filter::SpecificDate { month: 1, day: 15 });

    let filter = FilterParser::parse("Dec 25").unwrap();
    assert_eq!(filter, Filter::SpecificDate { month: 12, day: 25 });
}

#[test]
fn test_parse_specific_date_full_month() {
    let filter = FilterParser::parse("January 15").unwrap();
    assert_eq!(filter, Filter::SpecificDate { month: 1, day: 15 });

    let filter = FilterParser::parse("December 25").unwrap();
    assert_eq!(filter, Filter::SpecificDate { month: 12, day: 25 });
}

#[test]
fn test_parse_specific_date_case_insensitive() {
    assert_eq!(
        FilterParser::parse("JAN 15").unwrap(),
        Filter::SpecificDate { month: 1, day: 15 }
    );
    assert_eq!(
        FilterParser::parse("JANUARY 15").unwrap(),
        Filter::SpecificDate { month: 1, day: 15 }
    );
    assert_eq!(
        FilterParser::parse("jan 15").unwrap(),
        Filter::SpecificDate { month: 1, day: 15 }
    );
}

#[test]
fn test_parse_specific_date_all_months() {
    assert_eq!(
        FilterParser::parse("Jan 1").unwrap(),
        Filter::SpecificDate { month: 1, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("Feb 1").unwrap(),
        Filter::SpecificDate { month: 2, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("Mar 1").unwrap(),
        Filter::SpecificDate { month: 3, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("Apr 1").unwrap(),
        Filter::SpecificDate { month: 4, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("May 1").unwrap(),
        Filter::SpecificDate { month: 5, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("Jun 1").unwrap(),
        Filter::SpecificDate { month: 6, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("Jul 1").unwrap(),
        Filter::SpecificDate { month: 7, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("Aug 1").unwrap(),
        Filter::SpecificDate { month: 8, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("Sep 1").unwrap(),
        Filter::SpecificDate { month: 9, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("Sept 1").unwrap(),
        Filter::SpecificDate { month: 9, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("Oct 1").unwrap(),
        Filter::SpecificDate { month: 10, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("Nov 1").unwrap(),
        Filter::SpecificDate { month: 11, day: 1 }
    );
    assert_eq!(
        FilterParser::parse("Dec 1").unwrap(),
        Filter::SpecificDate { month: 12, day: 1 }
    );
}

#[test]
fn test_parse_specific_date_with_operators() {
    let filter = FilterParser::parse("Jan 15 & p1").unwrap();
    assert_eq!(
        filter,
        Filter::and(
            Filter::SpecificDate { month: 1, day: 15 },
            Filter::Priority1
        )
    );
}

#[test]
fn test_parse_specific_date_in_complex_expression() {
    let filter = FilterParser::parse("(Jan 15 | Dec 25) & @holiday").unwrap();
    assert_eq!(
        filter,
        Filter::and(
            Filter::or(
                Filter::SpecificDate { month: 1, day: 15 },
                Filter::SpecificDate { month: 12, day: 25 }
            ),
            Filter::Label("holiday".to_string())
        )
    );
}

// ==================== Priority Tests ====================

#[test]
fn test_parse_priority_1() {
    let filter = FilterParser::parse("p1").unwrap();
    assert_eq!(
        filter,
        Filter::Priority1,
        "parsing 'p1' should produce Filter::Priority1"
    );
}

#[test]
fn test_parse_priority_2() {
    let filter = FilterParser::parse("p2").unwrap();
    assert_eq!(
        filter,
        Filter::Priority2,
        "parsing 'p2' should produce Filter::Priority2"
    );
}

#[test]
fn test_parse_priority_3() {
    let filter = FilterParser::parse("p3").unwrap();
    assert_eq!(
        filter,
        Filter::Priority3,
        "parsing 'p3' should produce Filter::Priority3"
    );
}

#[test]
fn test_parse_priority_4() {
    let filter = FilterParser::parse("p4").unwrap();
    assert_eq!(
        filter,
        Filter::Priority4,
        "parsing 'p4' should produce Filter::Priority4"
    );
}

#[test]
fn test_parse_priority_case_insensitive() {
    assert_eq!(
        FilterParser::parse("P1").unwrap(),
        Filter::Priority1,
        "'P1' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("P2").unwrap(),
        Filter::Priority2,
        "'P2' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("P3").unwrap(),
        Filter::Priority3,
        "'P3' should be case-insensitive"
    );
    assert_eq!(
        FilterParser::parse("P4").unwrap(),
        Filter::Priority4,
        "'P4' should be case-insensitive"
    );
}

// ==================== Label Tests ====================

#[test]
fn test_parse_label() {
    let filter = FilterParser::parse("@urgent").unwrap();
    assert_eq!(filter, Filter::Label("urgent".to_string()));
}

#[test]
fn test_parse_label_with_special_chars() {
    let filter = FilterParser::parse("@work-tasks").unwrap();
    assert_eq!(filter, Filter::Label("work-tasks".to_string()));

    let filter = FilterParser::parse("@my_label").unwrap();
    assert_eq!(filter, Filter::Label("my_label".to_string()));
}

#[test]
fn test_parse_quoted_label() {
    let filter = FilterParser::parse("@\"My Label\"").unwrap();
    assert_eq!(filter, Filter::Label("My Label".to_string()));
}

// ==================== Project Tests ====================

#[test]
fn test_parse_project() {
    let filter = FilterParser::parse("#Work").unwrap();
    assert_eq!(filter, Filter::Project("Work".to_string()));
}

#[test]
fn test_parse_project_with_subprojects() {
    let filter = FilterParser::parse("##Work").unwrap();
    assert_eq!(filter, Filter::ProjectWithSubprojects("Work".to_string()));
}

#[test]
fn test_parse_quoted_project() {
    let filter = FilterParser::parse("#\"My Project\"").unwrap();
    assert_eq!(filter, Filter::Project("My Project".to_string()));
}

// ==================== Section Tests ====================

#[test]
fn test_parse_section() {
    let filter = FilterParser::parse("/Inbox").unwrap();
    assert_eq!(filter, Filter::Section("Inbox".to_string()));
}

// ==================== Boolean Operator Tests ====================

#[test]
fn test_parse_and() {
    let filter = FilterParser::parse("today & p1").unwrap();
    assert_eq!(
        filter,
        Filter::and(Filter::Today, Filter::Priority1),
        "'&' operator should combine filters with AND"
    );
}

#[test]
fn test_parse_or() {
    let filter = FilterParser::parse("today | overdue").unwrap();
    assert_eq!(
        filter,
        Filter::or(Filter::Today, Filter::Overdue),
        "'|' operator should combine filters with OR"
    );
}

#[test]
fn test_parse_not() {
    let filter = FilterParser::parse("!no date").unwrap();
    assert_eq!(
        filter,
        Filter::negate(Filter::NoDate),
        "'!' operator should negate the filter"
    );
}

#[test]
fn test_parse_double_not() {
    let filter = FilterParser::parse("!!today").unwrap();
    assert_eq!(
        filter,
        Filter::negate(Filter::negate(Filter::Today)),
        "double negation should create nested Not filters"
    );
}

// ==================== Operator Precedence Tests ====================

#[test]
fn test_and_has_higher_precedence_than_or() {
    // "today | tomorrow & p1" should be parsed as "today | (tomorrow & p1)"
    let filter = FilterParser::parse("today | tomorrow & p1").unwrap();
    let expected = Filter::or(
        Filter::Today,
        Filter::and(Filter::Tomorrow, Filter::Priority1),
    );
    assert_eq!(
        filter, expected,
        "AND should bind tighter than OR: 'today | tomorrow & p1' = 'today | (tomorrow & p1)'"
    );
}

#[test]
fn test_not_has_highest_precedence() {
    // "!today & p1" should be parsed as "(!today) & p1"
    let filter = FilterParser::parse("!today & p1").unwrap();
    let expected = Filter::and(Filter::negate(Filter::Today), Filter::Priority1);
    assert_eq!(
        filter, expected,
        "NOT should bind tightest: '!today & p1' = '(!today) & p1'"
    );
}

#[test]
fn test_parentheses_override_precedence() {
    // "(today | tomorrow) & p1"
    let filter = FilterParser::parse("(today | tomorrow) & p1").unwrap();
    let expected = Filter::and(
        Filter::or(Filter::Today, Filter::Tomorrow),
        Filter::Priority1,
    );
    assert_eq!(
        filter, expected,
        "parentheses should override default precedence"
    );
}

// ==================== Complex Expression Tests ====================

#[test]
fn test_parse_complex_expression() {
    // "(today | overdue) & p1 & @urgent"
    let filter = FilterParser::parse("(today | overdue) & p1 & @urgent").unwrap();
    let expected = Filter::and(
        Filter::and(
            Filter::or(Filter::Today, Filter::Overdue),
            Filter::Priority1,
        ),
        Filter::Label("urgent".to_string()),
    );
    assert_eq!(
        filter, expected,
        "complex expression with multiple operators should parse correctly"
    );
}

#[test]
fn test_parse_nested_parentheses() {
    // "((today | tomorrow) & p1) | overdue"
    let filter = FilterParser::parse("((today | tomorrow) & p1) | overdue").unwrap();
    let expected = Filter::or(
        Filter::and(
            Filter::or(Filter::Today, Filter::Tomorrow),
            Filter::Priority1,
        ),
        Filter::Overdue,
    );
    assert_eq!(
        filter, expected,
        "nested parentheses should parse correctly"
    );
}

#[test]
fn test_parse_multiple_and() {
    // "today & p1 & @urgent & #Work"
    let filter = FilterParser::parse("today & p1 & @urgent & #Work").unwrap();
    let expected = Filter::and(
        Filter::and(
            Filter::and(Filter::Today, Filter::Priority1),
            Filter::Label("urgent".to_string()),
        ),
        Filter::Project("Work".to_string()),
    );
    assert_eq!(
        filter, expected,
        "chained AND operators should be left-associative"
    );
}

#[test]
fn test_parse_multiple_or() {
    // "today | tomorrow | overdue"
    let filter = FilterParser::parse("today | tomorrow | overdue").unwrap();
    let expected = Filter::or(Filter::or(Filter::Today, Filter::Tomorrow), Filter::Overdue);
    assert_eq!(
        filter, expected,
        "chained OR operators should be left-associative"
    );
}

// ==================== Error Tests ====================

#[test]
fn test_error_empty_expression() {
    let result = FilterParser::parse("");
    assert!(matches!(result, Err(FilterError::EmptyExpression)));

    let result = FilterParser::parse("   ");
    assert!(matches!(result, Err(FilterError::EmptyExpression)));
}

#[test]
fn test_error_unclosed_parenthesis() {
    let result = FilterParser::parse("(today");
    assert!(matches!(
        result,
        Err(FilterError::UnclosedParenthesis { position: 0 })
    ));

    let result = FilterParser::parse("((today | tomorrow)");
    // The outer ( at position 0 is unclosed
    assert!(matches!(
        result,
        Err(FilterError::UnclosedParenthesis { position: 0 })
    ));
}

#[test]
fn test_error_unexpected_operator() {
    // & at position 0
    let result = FilterParser::parse("& today");
    match result {
        Err(FilterError::UnexpectedToken { token, position }) => {
            assert_eq!(token, "&", "unexpected token should be '&'");
            assert_eq!(position, 0, "error position should be 0 for leading '&'");
        }
        other => panic!("Expected UnexpectedToken error, got {:?}", other),
    }

    // | at position 0
    let result = FilterParser::parse("| today");
    match result {
        Err(FilterError::UnexpectedToken { token, position }) => {
            assert_eq!(token, "|", "unexpected token should be '|'");
            assert_eq!(position, 0, "error position should be 0 for leading '|'");
        }
        other => panic!("Expected UnexpectedToken error, got {:?}", other),
    }
}

#[test]
fn test_error_unexpected_close_paren() {
    // ) at position 0
    let result = FilterParser::parse(")today");
    match result {
        Err(FilterError::UnexpectedToken { token, position }) => {
            assert_eq!(token, ")", "unexpected token should be ')'");
            assert_eq!(position, 0, "error position should be 0 for leading ')'");
        }
        other => panic!("Expected UnexpectedToken error, got {:?}", other),
    }
}

#[test]
fn test_error_positions_with_unicode() {
    // "日本語 & " = 9 bytes (3 chars × 3 bytes each) + 1 space + 1 & + 1 space = 12 bytes
    // But lexer won't recognize 日本語, so let's use a simpler example
    // "今日 |" where 今日 is 6 bytes
    let result = FilterParser::parse("p1 & |");
    // p1 (2 bytes) + space + & + space + | = position 5
    match result {
        Err(FilterError::UnexpectedToken { token, position }) => {
            assert_eq!(token, "|", "unexpected token should be '|'");
            assert_eq!(position, 5, "error position should be 5 (after 'p1 & ')");
        }
        other => panic!("Expected UnexpectedToken error, got {:?}", other),
    }
}

#[test]
fn test_error_nested_unclosed_parenthesis() {
    // "(((today" - the innermost ( at position 2 is the one that's unclosed
    // Actually, parsing starts from outermost, so position 0 is unclosed first
    let result = FilterParser::parse("today & (p1");
    // The ( at position 8 is unclosed
    match result {
        Err(FilterError::UnclosedParenthesis { position }) => {
            assert_eq!(
                position, 8,
                "unclosed parenthesis at position 8 (after 'today & ')"
            );
        }
        other => panic!("Expected UnclosedParenthesis error, got {:?}", other),
    }
}

#[test]
fn test_error_trailing_operator() {
    let result = FilterParser::parse("today &");
    // Input is 7 bytes: "today &"
    assert!(matches!(
        result,
        Err(FilterError::UnexpectedEndOfInput { position: 7 })
    ));

    let result = FilterParser::parse("today |");
    assert!(matches!(
        result,
        Err(FilterError::UnexpectedEndOfInput { position: 7 })
    ));
}

// ==================== AST Helper Tests ====================

#[test]
fn test_filter_and_helper() {
    let filter = Filter::and(Filter::Today, Filter::Priority1);
    match filter {
        Filter::And(left, right) => {
            assert_eq!(*left, Filter::Today, "left operand of AND should be Today");
            assert_eq!(
                *right,
                Filter::Priority1,
                "right operand of AND should be Priority1"
            );
        }
        _ => panic!("Expected And filter"),
    }
}

#[test]
fn test_filter_or_helper() {
    let filter = Filter::or(Filter::Today, Filter::Overdue);
    match filter {
        Filter::Or(left, right) => {
            assert_eq!(*left, Filter::Today, "left operand of OR should be Today");
            assert_eq!(
                *right,
                Filter::Overdue,
                "right operand of OR should be Overdue"
            );
        }
        _ => panic!("Expected Or filter"),
    }
}

#[test]
fn test_filter_not_helper() {
    let filter = Filter::negate(Filter::NoDate);
    match filter {
        Filter::Not(inner) => {
            assert_eq!(
                *inner,
                Filter::NoDate,
                "inner filter of NOT should be NoDate"
            );
        }
        _ => panic!("Expected Not filter"),
    }
}

// ==================== Clone and Debug Tests ====================

#[test]
fn test_filter_clone() {
    let filter = Filter::and(Filter::Today, Filter::Priority1);
    let cloned = filter.clone();
    assert_eq!(filter, cloned, "cloned filter should equal original");
}

#[test]
fn test_filter_debug() {
    let filter = Filter::Today;
    let debug_str = format!("{:?}", filter);
    assert!(
        debug_str.contains("Today"),
        "Debug output should contain 'Today'"
    );
}

#[test]
fn test_filter_error_display() {
    let err = FilterError::EmptyExpression;
    assert_eq!(format!("{}", err), "filter expression is empty");

    let err = FilterError::unexpected_token("&", 5);
    assert_eq!(format!("{}", err), "unexpected token '&' at position 5");

    let err = FilterError::unclosed_parenthesis(10);
    assert_eq!(format!("{}", err), "unclosed parenthesis at position 10");

    let err = FilterError::unexpected_end_of_input(15);
    assert_eq!(
        format!("{}", err),
        "unexpected end of expression after position 15"
    );
}

// ==================== Unknown Character Error Tests ====================

#[test]
fn test_error_unknown_character() {
    let result = FilterParser::parse("today $ p1");
    match result {
        Err(FilterError::UnknownCharacters { errors }) => {
            assert_eq!(
                errors.len(),
                1,
                "should report exactly one unknown character"
            );
            assert_eq!(errors[0].character, '$', "unknown character should be '$'");
            assert_eq!(
                errors[0].position, 6,
                "position should be 6 (after 'today ')"
            );
        }
        other => panic!("Expected UnknownCharacters error, got {:?}", other),
    }
}

#[test]
fn test_error_multiple_unknown_characters() {
    let result = FilterParser::parse("$today % p1");
    match result {
        Err(FilterError::UnknownCharacters { errors }) => {
            assert_eq!(errors.len(), 2, "should report two unknown characters");
            assert_eq!(
                errors[0].character, '$',
                "first unknown character should be '$'"
            );
            assert_eq!(errors[0].position, 0, "first error position should be 0");
            assert_eq!(
                errors[1].character, '%',
                "second unknown character should be '%'"
            );
            assert_eq!(
                errors[1].position, 7,
                "second error position should be 7 (after '$today ')"
            );
        }
        other => panic!("Expected UnknownCharacters error, got {:?}", other),
    }
}

#[test]
fn test_error_unknown_character_display() {
    use super::lexer::LexerError;
    let err = FilterError::UnknownCharacters {
        errors: vec![LexerError {
            character: '$',
            position: 6,
        }],
    };
    let msg = format!("{}", err);
    assert!(
        msg.contains("'$'"),
        "error message should contain the character"
    );
    assert!(
        msg.contains("position 6"),
        "error message should contain the position"
    );
}

#[test]
fn test_error_unknown_character_unicode() {
    // Test with a Unicode character
    let result = FilterParser::parse("today 🎉 p1");
    match result {
        Err(FilterError::UnknownCharacters { errors }) => {
            assert_eq!(
                errors.len(),
                1,
                "should report exactly one unknown character"
            );
            assert_eq!(
                errors[0].character, '🎉',
                "unknown character should be emoji"
            );
            assert_eq!(
                errors[0].position, 6,
                "position should be 6 (byte offset after 'today ')"
            );
        }
        other => panic!("Expected UnknownCharacters error, got {:?}", other),
    }
}

// ==================== Assignment Filter Parser Tests ====================

#[test]
fn test_parse_assigned_to_me() {
    let filter = FilterParser::parse("assigned to: me").unwrap();
    assert_eq!(filter, Filter::AssignedTo(AssignedTarget::Me));
}

#[test]
fn test_parse_assigned_to_others() {
    let filter = FilterParser::parse("assigned to: others").unwrap();
    assert_eq!(filter, Filter::AssignedTo(AssignedTarget::Others));
}

#[test]
fn test_parse_assigned_to_name() {
    let filter = FilterParser::parse("assigned to: Alice").unwrap();
    assert_eq!(
        filter,
        Filter::AssignedTo(AssignedTarget::User("Alice".to_string()))
    );
}

#[test]
fn test_parse_assigned_by_me() {
    let filter = FilterParser::parse("assigned by: me").unwrap();
    assert_eq!(filter, Filter::AssignedBy(AssignedTarget::Me));
}

#[test]
fn test_parse_assigned_by_name() {
    let filter = FilterParser::parse("assigned by: Alice").unwrap();
    assert_eq!(
        filter,
        Filter::AssignedBy(AssignedTarget::User("Alice".to_string()))
    );
}

#[test]
fn test_parse_assigned() {
    let filter = FilterParser::parse("assigned").unwrap();
    assert_eq!(filter, Filter::Assigned);
}

#[test]
fn test_parse_no_assignee() {
    let filter = FilterParser::parse("no assignee").unwrap();
    assert_eq!(filter, Filter::NoAssignee);
}

#[test]
fn test_parse_assigned_case_insensitive() {
    assert_eq!(
        FilterParser::parse("Assigned To: Me").unwrap(),
        Filter::AssignedTo(AssignedTarget::Me)
    );
    assert_eq!(
        FilterParser::parse("ASSIGNED TO: ME").unwrap(),
        Filter::AssignedTo(AssignedTarget::Me)
    );
    assert_eq!(
        FilterParser::parse("Assigned By: Me").unwrap(),
        Filter::AssignedBy(AssignedTarget::Me)
    );
}

#[test]
fn test_parse_assigned_combined() {
    let filter = FilterParser::parse("assigned to: me & p1").unwrap();
    assert_eq!(
        filter,
        Filter::and(Filter::AssignedTo(AssignedTarget::Me), Filter::Priority1)
    );
}

#[test]
fn test_parse_assigned_to_name_with_spaces() {
    let filter = FilterParser::parse("assigned to: Alice Smith").unwrap();
    assert_eq!(
        filter,
        Filter::AssignedTo(AssignedTarget::User("Alice Smith".to_string()))
    );
}

#[test]
fn test_parse_not_assigned() {
    let filter = FilterParser::parse("!assigned").unwrap();
    assert_eq!(filter, Filter::negate(Filter::Assigned));
}
