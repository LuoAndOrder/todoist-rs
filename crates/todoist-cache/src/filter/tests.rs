//! Tests for the filter parser.

use super::*;

// ==================== Date Keyword Tests ====================

#[test]
fn test_parse_today() {
    let filter = FilterParser::parse("today").unwrap();
    assert_eq!(filter, Filter::Today);
}

#[test]
fn test_parse_today_case_insensitive() {
    assert_eq!(FilterParser::parse("TODAY").unwrap(), Filter::Today);
    assert_eq!(FilterParser::parse("Today").unwrap(), Filter::Today);
    assert_eq!(FilterParser::parse("ToDAy").unwrap(), Filter::Today);
}

#[test]
fn test_parse_today_with_whitespace() {
    assert_eq!(FilterParser::parse("  today  ").unwrap(), Filter::Today);
    assert_eq!(FilterParser::parse("\ttoday\n").unwrap(), Filter::Today);
}

#[test]
fn test_parse_tomorrow() {
    let filter = FilterParser::parse("tomorrow").unwrap();
    assert_eq!(filter, Filter::Tomorrow);
}

#[test]
fn test_parse_tomorrow_case_insensitive() {
    assert_eq!(FilterParser::parse("TOMORROW").unwrap(), Filter::Tomorrow);
    assert_eq!(FilterParser::parse("Tomorrow").unwrap(), Filter::Tomorrow);
}

#[test]
fn test_parse_overdue() {
    let filter = FilterParser::parse("overdue").unwrap();
    assert_eq!(filter, Filter::Overdue);
}

#[test]
fn test_parse_overdue_case_insensitive() {
    assert_eq!(FilterParser::parse("OVERDUE").unwrap(), Filter::Overdue);
    assert_eq!(FilterParser::parse("Overdue").unwrap(), Filter::Overdue);
    assert_eq!(FilterParser::parse("OverDue").unwrap(), Filter::Overdue);
}

#[test]
fn test_parse_no_date() {
    let filter = FilterParser::parse("no date").unwrap();
    assert_eq!(filter, Filter::NoDate);
}

#[test]
fn test_parse_no_date_case_insensitive() {
    assert_eq!(FilterParser::parse("NO DATE").unwrap(), Filter::NoDate);
    assert_eq!(FilterParser::parse("No Date").unwrap(), Filter::NoDate);
    assert_eq!(FilterParser::parse("no DATE").unwrap(), Filter::NoDate);
}

#[test]
fn test_parse_no_date_with_extra_whitespace() {
    // Multiple spaces between "no" and "date"
    assert_eq!(FilterParser::parse("no   date").unwrap(), Filter::NoDate);
    assert_eq!(FilterParser::parse("no\tdate").unwrap(), Filter::NoDate);
}

// ==================== Priority Tests ====================

#[test]
fn test_parse_priority_1() {
    let filter = FilterParser::parse("p1").unwrap();
    assert_eq!(filter, Filter::Priority1);
}

#[test]
fn test_parse_priority_2() {
    let filter = FilterParser::parse("p2").unwrap();
    assert_eq!(filter, Filter::Priority2);
}

#[test]
fn test_parse_priority_3() {
    let filter = FilterParser::parse("p3").unwrap();
    assert_eq!(filter, Filter::Priority3);
}

#[test]
fn test_parse_priority_4() {
    let filter = FilterParser::parse("p4").unwrap();
    assert_eq!(filter, Filter::Priority4);
}

#[test]
fn test_parse_priority_case_insensitive() {
    assert_eq!(FilterParser::parse("P1").unwrap(), Filter::Priority1);
    assert_eq!(FilterParser::parse("P2").unwrap(), Filter::Priority2);
    assert_eq!(FilterParser::parse("P3").unwrap(), Filter::Priority3);
    assert_eq!(FilterParser::parse("P4").unwrap(), Filter::Priority4);
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
    assert_eq!(filter, Filter::and(Filter::Today, Filter::Priority1));
}

#[test]
fn test_parse_or() {
    let filter = FilterParser::parse("today | overdue").unwrap();
    assert_eq!(filter, Filter::or(Filter::Today, Filter::Overdue));
}

#[test]
fn test_parse_not() {
    let filter = FilterParser::parse("!no date").unwrap();
    assert_eq!(filter, Filter::negate(Filter::NoDate));
}

#[test]
fn test_parse_double_not() {
    let filter = FilterParser::parse("!!today").unwrap();
    assert_eq!(filter, Filter::negate(Filter::negate(Filter::Today)));
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
    assert_eq!(filter, expected);
}

#[test]
fn test_not_has_highest_precedence() {
    // "!today & p1" should be parsed as "(!today) & p1"
    let filter = FilterParser::parse("!today & p1").unwrap();
    let expected = Filter::and(Filter::negate(Filter::Today), Filter::Priority1);
    assert_eq!(filter, expected);
}

#[test]
fn test_parentheses_override_precedence() {
    // "(today | tomorrow) & p1"
    let filter = FilterParser::parse("(today | tomorrow) & p1").unwrap();
    let expected = Filter::and(
        Filter::or(Filter::Today, Filter::Tomorrow),
        Filter::Priority1,
    );
    assert_eq!(filter, expected);
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
    assert_eq!(filter, expected);
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
    assert_eq!(filter, expected);
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
    assert_eq!(filter, expected);
}

#[test]
fn test_parse_multiple_or() {
    // "today | tomorrow | overdue"
    let filter = FilterParser::parse("today | tomorrow | overdue").unwrap();
    let expected = Filter::or(
        Filter::or(Filter::Today, Filter::Tomorrow),
        Filter::Overdue,
    );
    assert_eq!(filter, expected);
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
    assert!(matches!(result, Err(FilterError::UnclosedParenthesis)));

    let result = FilterParser::parse("((today | tomorrow)");
    assert!(matches!(result, Err(FilterError::UnclosedParenthesis)));
}

#[test]
fn test_error_unexpected_operator() {
    let result = FilterParser::parse("& today");
    assert!(matches!(result, Err(FilterError::UnexpectedToken { .. })));

    let result = FilterParser::parse("| today");
    assert!(matches!(result, Err(FilterError::UnexpectedToken { .. })));
}

#[test]
fn test_error_unexpected_close_paren() {
    let result = FilterParser::parse(")today");
    assert!(matches!(result, Err(FilterError::UnexpectedToken { .. })));
}

#[test]
fn test_error_trailing_operator() {
    let result = FilterParser::parse("today &");
    assert!(matches!(result, Err(FilterError::UnexpectedEndOfInput)));

    let result = FilterParser::parse("today |");
    assert!(matches!(result, Err(FilterError::UnexpectedEndOfInput)));
}

// ==================== AST Helper Tests ====================

#[test]
fn test_filter_and_helper() {
    let filter = Filter::and(Filter::Today, Filter::Priority1);
    match filter {
        Filter::And(left, right) => {
            assert_eq!(*left, Filter::Today);
            assert_eq!(*right, Filter::Priority1);
        }
        _ => panic!("Expected And filter"),
    }
}

#[test]
fn test_filter_or_helper() {
    let filter = Filter::or(Filter::Today, Filter::Overdue);
    match filter {
        Filter::Or(left, right) => {
            assert_eq!(*left, Filter::Today);
            assert_eq!(*right, Filter::Overdue);
        }
        _ => panic!("Expected Or filter"),
    }
}

#[test]
fn test_filter_not_helper() {
    let filter = Filter::negate(Filter::NoDate);
    match filter {
        Filter::Not(inner) => {
            assert_eq!(*inner, Filter::NoDate);
        }
        _ => panic!("Expected Not filter"),
    }
}

// ==================== Clone and Debug Tests ====================

#[test]
fn test_filter_clone() {
    let filter = Filter::and(Filter::Today, Filter::Priority1);
    let cloned = filter.clone();
    assert_eq!(filter, cloned);
}

#[test]
fn test_filter_debug() {
    let filter = Filter::Today;
    let debug_str = format!("{:?}", filter);
    assert!(debug_str.contains("Today"));
}

#[test]
fn test_filter_error_display() {
    let err = FilterError::EmptyExpression;
    assert_eq!(format!("{}", err), "filter expression is empty");

    let err = FilterError::unexpected_token("&");
    assert_eq!(format!("{}", err), "unexpected token: &");

    let err = FilterError::UnclosedParenthesis;
    assert_eq!(format!("{}", err), "unclosed parenthesis");
}

// ==================== Unknown Character Error Tests ====================

#[test]
fn test_error_unknown_character() {
    let result = FilterParser::parse("today $ p1");
    match result {
        Err(FilterError::UnknownCharacters { errors }) => {
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].character, '$');
            assert_eq!(errors[0].position, 6); // "today " = 6 bytes
        }
        other => panic!("Expected UnknownCharacters error, got {:?}", other),
    }
}

#[test]
fn test_error_multiple_unknown_characters() {
    let result = FilterParser::parse("$today % p1");
    match result {
        Err(FilterError::UnknownCharacters { errors }) => {
            assert_eq!(errors.len(), 2);
            assert_eq!(errors[0].character, '$');
            assert_eq!(errors[0].position, 0);
            assert_eq!(errors[1].character, '%');
            assert_eq!(errors[1].position, 7); // "$today " = 7 bytes
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
    assert!(msg.contains("'$'"));
    assert!(msg.contains("position 6"));
}

#[test]
fn test_error_unknown_character_unicode() {
    // Test with a Unicode character
    let result = FilterParser::parse("today 🎉 p1");
    match result {
        Err(FilterError::UnknownCharacters { errors }) => {
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].character, '🎉');
            assert_eq!(errors[0].position, 6);
        }
        other => panic!("Expected UnknownCharacters error, got {:?}", other),
    }
}
