#![feature(iter_array_chunks)]
#![feature(assert_matches)]

use damasc::{
    assignment::{AssignmentError, Assignment},
    parser::{expression_multi, try_match_multi}, statement::Statement, env::Environment, matcher::Matcher,
};
use std::{assert_matches::assert_matches, collections::BTreeMap};

#[test]
fn test_expressions() {
    let mut tests = include_str!("test_expressions.txt").lines().array_chunks();
    let env = damasc::env::Environment {
        bindings: BTreeMap::new(),
    };

    for [expr, result, sep] in &mut tests {
        assert_eq!("---", sep, "Expression pairs are separated by --- line");
        let Ok((_, parsed)) = expression_multi(expr) else {
            unreachable!("Expression set A can be parsed, {expr}");
        };
        let Ok((_, value)) = expression_multi(result) else {
            unreachable!("Expression set B can be parsed, {result}");
        };

        for (a, b) in std::iter::zip(parsed.expressions, value.expressions) {
            let evaled = env.eval_expr(&a);
            let valued_evaled = env.eval_expr(&b);

            assert!(evaled.is_ok(), "Expression A can be evaluated");
            assert!(valued_evaled.is_ok(), "Expression B can be parsed");

            assert_eq!(
                evaled.unwrap(),
                valued_evaled.unwrap(),
                "Expression A and B evaluate to the same value"
            );
        }
    }

    let Some(e) = tests.into_remainder() else {
        unreachable!("Number of Test Expression lines are multiple of 3");
    };
    assert_eq!(
        e.count(),
        0,
        "Last expression pair is followed terminated by ---"
    );
}

#[test]
fn test_patterns() {
    let tests = include_str!("test_patterns.txt").lines();
    let env = Environment {
        bindings: BTreeMap::new(),
    };

    for case in tests {
        let mut matcher = Matcher {
            env: &env,
            bindings: BTreeMap::new(),
        };

        let Ok((_, Statement::MatchSet(mut assignment_set))) = try_match_multi(case) else {
            unreachable!("Test Pattern and Expression can be parsed: {case}");
        };

        if assignment_set.sort_topological(env.identifiers()).is_err() {
            unreachable!("Topological Error in: {case}");
        }

        for Assignment {
            pattern,
            expression,
        } in assignment_set.assignments
        {
            let Ok(value) = env.eval_expr(&expression) else {
                unreachable!("TestExpression can be evaluated: {case}");
            };

            assert_matches!(
                matcher.match_pattern(&pattern, &value),
                Ok(_),
                "Test Expression Value matches the test pattern: {case}"
            );
        }
    }
}

#[test]
fn test_negative_patterns() {
    let tests = include_str!("test_negative_patterns.txt").lines();
    let env = Environment {
        bindings: BTreeMap::new(),
    };

    for case in tests {
        let mut matcher = Matcher {
            env: &env,
            bindings: BTreeMap::new(),
        };
        let Ok((_, Statement::MatchSet(mut assignment_set))) = try_match_multi(case) else {
            unreachable!("Test Pattern and Expression can be parsed: {case}");
        };

        if assignment_set.sort_topological(env.identifiers()).is_err() {
            unreachable!("Topological Error in: {case}");
        }

        for Assignment {
            pattern,
            expression,
        } in assignment_set.assignments
        {
            let Ok(value) = env.eval_expr(&expression) else {
                unreachable!("TestExpression can be evaluated: {case}");
            };

            assert_matches!(
                matcher.match_pattern(&pattern, &value),
                Err(_),
                "Test Expression Value does not match the test pattern: {case}"
            );
        }
    }
}

#[test]
fn test_topological_assignments() {
    let tests = include_str!("test_topological.txt").lines();
    let env = Environment {
        bindings: BTreeMap::new(),
    };

    for case in tests {
        let mut tmp_env = env.clone();
        let mut matcher = Matcher {
            env: &tmp_env.clone(),
            bindings: BTreeMap::new(),
        };
        let Ok((_, Statement::MatchSet(mut assignment_set))) = try_match_multi(case) else {
            unreachable!("Test Pattern and Expression can be parsed: {case}");
        };

        if assignment_set
            .sort_topological(tmp_env.identifiers())
            .is_err()
        {
            unreachable!("Topological Error in: {case}");
        }

        for Assignment {
            pattern,
            expression,
        } in assignment_set.assignments
        {
            let Ok(value) = tmp_env.eval_expr(&expression) else {
                unreachable!("TestExpression can be evaluated: {case}");
            };

            assert_matches!(
                matcher.match_pattern(&pattern, &value),
                Ok(_),
                "Test Expression Value matches the test pattern: {case}"
            );

            matcher.apply_to_env(&mut tmp_env);
        }
    }
}
#[test]
fn test_topological_fail() {
    let tests = include_str!("test_topological_fail.txt").lines();
    let env = Environment {
        bindings: BTreeMap::new(),
    };

    for case in tests {
        let Ok((_, Statement::MatchSet(mut assignment_set))) = try_match_multi(case) else {
            unreachable!("Test Pattern and Expression can be parsed: {case}");
        };

        assert_matches!(
            assignment_set.sort_topological(env.identifiers()),
            Err(AssignmentError::TopologicalConflict(_))
        )
    }
}