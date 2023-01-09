use std::borrow::Cow;

use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, take_until};
use nom::character::complete::{alpha1, char, i64, multispace0, alphanumeric1};
use nom::combinator::{all_consuming, map, opt, recognize, value, verify};
use nom::error::ParseError;
use nom::multi::{fold_many0, many0, many1, separated_list0, separated_list1, many0_count, many1_count};
use nom::sequence::{delimited, pair, preceded, separated_pair, terminated, tuple};
use nom::IResult;

use crate::assignment::{Assignment, AssignmentSet};
use crate::expression::*;
use crate::identifier::Identifier;
use crate::literal::Literal;
use crate::pattern::*;
use crate::query::{CrossPredicate, Predicate, Query};
use crate::statement::Statement;
use crate::value::ValueType;

fn array_item_expression<'v>(input: &str) -> IResult<&str, ArrayItem<'v>> {
    alt((
        map(preceded(ws(tag("...")), expression), ArrayItem::Spread),
        map(expression, ArrayItem::Single),
    ))(input)
}

fn ws<'a, F, O, E: ParseError<&'a str>>(inner: F) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: FnMut(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn expression_call<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    map(
        pair(
            identifier,
            delimited(ws(tag("(")), expression, ws(tag(")"))),
        ),
        |(function, arg)| {
            Expression::Call(CallExpression {
                function,
                argument: Box::new(arg),
            })
        },
    )(input)
}

fn expression_array<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    delimited(
        ws(tag("[")),
        terminated(
            map(
                separated_list0(ws(tag(",")), array_item_expression),
                Expression::Array,
            ),
            opt(ws(tag(","))),
        ),
        ws(tag("]")),
    )(input)
}

fn object_prop_expression<'v>(input: &str) -> IResult<&str, ObjectProperty<'v>> {
    alt((
        map(
            separated_pair(
                delimited(ws(tag("[")), expression, ws(tag("]"))),
                ws(tag(":")),
                expression,
            ),
            |(prop, value)| {
                ObjectProperty::Property(Property {
                    key: PropertyKey::Expression(prop),
                    value,
                })
            },
        ),
        map(
            separated_pair(identifier, ws(tag(":")), expression),
            |(prop, value)| {
                ObjectProperty::Property(Property {
                    key: PropertyKey::Identifier(prop),
                    value,
                })
            },
        ),
        map(
            separated_pair(literal_string_raw, ws(tag(":")), expression),
            |(prop, value)| {
                ObjectProperty::Property(Property {
                    key: PropertyKey::Identifier(Identifier { name: prop }),
                    value,
                })
            },
        ),
        map(preceded(ws(tag("...")), expression), ObjectProperty::Spread),
        map(identifier, ObjectProperty::Single),
    ))(input)
}

fn expression_object<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    delimited(
        ws(tag("{")),
        terminated(
            map(
                separated_list0(ws(ws(tag(","))), object_prop_expression),
                Expression::Object,
            ),
            opt(ws(tag(","))),
        ),
        ws(tag("}")),
    )(input)
}

fn expression_literal<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    alt((
        expression_object,
        expression_array,
        expression_string_template,
        expression_call,
        expression_atom,
    ))(input)
}

fn expression_atom<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    map(
        alt((
            literal_null,
            literal_string,
            literal_bool,
            literal_number,
            literal_type,
        )),
        Expression::Literal,
    )(input)
}

fn expression_identifier<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    map(identifier, Expression::Identifier)(input)
}

fn literal_null<'v>(input: &str) -> IResult<&str, Literal<'v>> {
    value(Literal::Null, tag("null"))(input)
}

fn string_template_part<'v>(input: &str) -> IResult<&str, StringTemplatePart<'v>> {
    map(
        tuple((
            recognize(take_until("${")),
            delimited(tag("${"), expression, tag("}")),
        )),
        |(fixed_start, dynamic_end)| StringTemplatePart {
            fixed_start: Cow::Owned(fixed_start.into()),
            dynamic_end: Box::new(dynamic_end),
        },
    )(input)
}

fn expression_string_template<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    map(
        delimited(
            tag("`"),
            tuple((many0(string_template_part), recognize(many0(is_not("`"))))),
            tag("`"),
        ),
        |(parts, s)| {
            Expression::Template(StringTemplate {
                parts,
                suffix: Cow::Owned(s.to_string()),
            })
        },
    )(input)
}

fn literal_string_raw<'v>(input: &str) -> IResult<&str, Cow<'v, str>> {
    map(
        delimited(tag("\""), take_until("\""), tag("\"")),
        |s: &str| Cow::Owned(s.to_string()),
    )(input)
}

fn literal_string<'v>(input: &str) -> IResult<&str, Literal<'v>> {
    map(literal_string_raw, Literal::String)(input)
}

fn literal_bool<'v>(input: &str) -> IResult<&str, Literal<'v>> {
    alt((
        value(Literal::Boolean(true), tag("true")),
        value(Literal::Boolean(false), tag("false")),
    ))(input)
}

fn literal_number<'v>(input: &str) -> IResult<&str, Literal<'v>> {
    map(recognize(i64), |s: &str| {
        Literal::Number(Cow::Owned(s.to_owned()))
    })(input)
}

fn no_keyword(input: &str) -> bool {
    !matches!(input, "where" | "into" | "limit")
}

fn identifier_name(input: &str) -> IResult<&str, &str> {
    recognize(
        alt((
            pair(
                alpha1,
                many0_count(alt((alphanumeric1, tag("_"))))
            ),
            pair(
            tag("_"),
            many1_count(alt((alphanumeric1, tag("_"))))
            )
        ))
      )(input)
}

fn non_keyword_identifier<'v>(input: &str) -> IResult<&str, Identifier<'v>> {
    map(verify(identifier_name, no_keyword), |name: &str| Identifier {
        name: Cow::Owned(name.to_string()),
    })(input)
}

fn raw_identifier<'v>(input: &str) -> IResult<&str, Identifier<'v>> {
    map(preceded(tag("#"), identifier_name), |name: &str| Identifier {
        name: Cow::Owned(name.to_string()),
    })(input)
}

fn identifier<'v>(input: &str) -> IResult<&str, Identifier<'v>> {
    alt((
        raw_identifier,
        non_keyword_identifier
    ))(input)
}

fn expression_logic_additive<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_logic_multiplicative(input)?;

    fold_many0(
        pair(
            ws(alt((value(LogicalOperator::Or, tag("||")),))),
            expression_logic_multiplicative,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Logical(LogicalExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn expression_logic_multiplicative<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_type_predicate(input)?;

    fold_many0(
        pair(
            ws(alt((value(LogicalOperator::And, tag("&&")),))),
            expression_type_predicate,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Logical(LogicalExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn literal_type_raw(input: &str) -> IResult<&str, ValueType> {
    alt((
        value(ValueType::Type, tag("Type")),
        value(ValueType::Null, tag("Null")),
        value(ValueType::Boolean, tag("Boolean")),
        value(ValueType::Integer, tag("Integer")),
        value(ValueType::Array, tag("Array")),
        value(ValueType::Object, tag("Object")),
        value(ValueType::String, tag("String")),
    ))(input)
}

fn literal_type<'v>(input: &str) -> IResult<&str, Literal<'v>> {
    map(literal_type_raw, Literal::Type)(input)
}

fn expression_type_predicate<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_type_additive(input)?;

    let Ok((input, (op, t))) = tuple((ws(alt((
        value(BinaryOperator::Is, tag("is")),
    ))), expression_numeric_predicative))(input) else {
        return Ok((input, init));
    };

    Ok((
        input,
        Expression::Binary(BinaryExpression {
            operator: op,
            left: Box::new(init),
            right: Box::new(t),
        }),
    ))
}

fn expression_type_additive<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_numeric_predicative(input)?;

    fold_many0(
        pair(
            ws(alt((value(BinaryOperator::Cast, tag("as")),))),
            expression_numeric_predicative,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn expression_numeric_predicative<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_numeric_additive(input)?;

    fold_many0(
        pair(
            ws(alt((
                value(BinaryOperator::GreaterThanEqual, tag(">=")),
                value(BinaryOperator::LessThanEqual, tag("<=")),
                value(BinaryOperator::LessThan, char('<')),
                value(BinaryOperator::GreaterThan, char('>')),
                value(BinaryOperator::StrictEqual, tag("==")),
                value(BinaryOperator::StrictNotEqual, tag("!=")),
                value(BinaryOperator::In, tag("in")),
            ))),
            expression_numeric_additive,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn expression_numeric_additive<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_numeric_multiplicative(input)?;

    fold_many0(
        pair(
            ws(alt((
                value(BinaryOperator::Plus, char('+')),
                value(BinaryOperator::Minus, char('-')),
            ))),
            expression_numeric_multiplicative,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn expression_numeric_multiplicative<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_numeric_exponential(input)?;

    fold_many0(
        pair(
            ws(alt((
                value(BinaryOperator::Times, char('*')),
                value(BinaryOperator::Over, char('/')),
                value(BinaryOperator::Mod, char('%')),
            ))),
            expression_numeric_exponential,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn expression_numeric_exponential<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_indexed(input)?;

    fold_many0(
        pair(
            ws(alt((value(BinaryOperator::PowerOf, char('^')),))),
            expression_indexed,
        ),
        move || init.clone(),
        |left, (operator, right)| {
            Expression::Binary(BinaryExpression {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            })
        },
    )(input)
}

fn expression_indexed<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_member(input)?;

    fold_many0(
        delimited(ws(tag("[")), expression, ws(tag("]"))),
        move || init.clone(),
        |acc, ident| {
            Expression::Member(MemberExpression {
                object: Box::new(acc),
                property: Box::new(ident),
            })
        },
    )(input)
}

fn expression_member<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    let (input, init) = expression_primary(input)?;

    fold_many0(
        alt((preceded(ws(char('.')), identifier),)),
        move || init.clone(),
        |acc, ident| {
            Expression::Member(MemberExpression {
                object: Box::new(acc),
                property: Box::new(Expression::Literal(Literal::String(ident.name))),
            })
        },
    )(input)
}

fn expression_primary<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    alt((
        expression_with_paren,
        expression_literal,
        expression_identifier,
        expression_unary,
    ))(input)
}

fn expression_with_paren<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    delimited(tag("("), expression, tag(")"))(input)
}

fn expression_unary<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    alt((expression_unary_logic, expression_unary_numeric))(input)
}

fn expression_unary_logic<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    map(
        pair(
            ws(alt((value(UnaryOperator::Not, tag("!")),))),
            expression_primary,
        ),
        |(operator, argument)| {
            Expression::Unary(UnaryExpression {
                operator,
                argument: Box::new(argument),
            })
        },
    )(input)
}

fn expression_unary_numeric<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    map(
        pair(
            ws(alt((
                value(UnaryOperator::Minus, tag("-")),
                value(UnaryOperator::Plus, tag("+")),
            ))),
            alt((expression_indexed,)),
        ),
        |(operator, argument)| {
            Expression::Unary(UnaryExpression {
                operator,
                argument: Box::new(argument),
            })
        },
    )(input)
}

fn expression<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    alt((expression_logic_additive,))(input)
}

fn expression_bag<'v>(input: &str) -> IResult<&str, std::vec::Vec<Expression<'v>>> {
    separated_list1(ws(tag(";")), expression)(input)
}

pub(crate) fn full_expression<'v>(input: &str) -> IResult<&str, Expression<'v>> {
    all_consuming(expression)(input)
}

pub(crate) fn expression_multi<'v>(input: &str) -> IResult<&str, ExpressionSet<'v>> {
    map(
        separated_list1(ws(tag(";")), expression),
        |expressions| ExpressionSet{ expressions }
    )(input)
}

fn full_pattern<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    all_consuming(pattern)(input)
}

fn pattern_discard<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    value(Pattern::Discard, tag("_"))(input)
}

fn pattern_typed_discard<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    map(
        preceded(ws(tag("_ is ")), literal_type_raw),
        Pattern::TypedDiscard,
    )(input)
}

fn pattern_identifier<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    map(identifier, Pattern::Identifier)(input)
}

fn pattern_typed_identifier<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    map(
        separated_pair(identifier, tag(" is "), literal_type_raw),
        |(i, t)| Pattern::TypedIdentifier(i, t),
    )(input)
}

fn object_prop_pattern<'v>(input: &str) -> IResult<&str, ObjectPropertyPattern<'v>> {
    alt((
        map(
            separated_pair(
                delimited(ws(tag("[")), expression, ws(tag("]"))),
                ws(tag(":")),
                pattern,
            ),
            |(prop, value)| {
                ObjectPropertyPattern::Match(PropertyPattern {
                    key: PropertyKey::Expression(prop),
                    value,
                })
            },
        ),
        map(
            separated_pair(identifier, ws(tag(":")), pattern),
            |(prop, value)| {
                ObjectPropertyPattern::Match(PropertyPattern {
                    key: PropertyKey::Identifier(prop),
                    value,
                })
            },
        ),
        map(identifier, ObjectPropertyPattern::Single),
    ))(input)
}

fn pattern_object<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    delimited(
        ws(tag("{")),
        alt((
            map(pattern_rest, |r| Pattern::Object(vec![], r)),
            map(
                tuple((
                    separated_list0(ws(ws(tag(","))), object_prop_pattern),
                    opt(preceded(ws(tag(",")), pattern_rest)),
                )),
                |(props, rest)| Pattern::Object(props, rest.unwrap_or(Rest::Exact)),
            ),
        )),
        ws(tag("}")),
    )(input)
}

fn pattern_rest<'v>(input: &str) -> IResult<&str, Rest<'v>> {
    alt((
        map(preceded(ws(tag("...")), pattern), |r| {
            Rest::Collect(Box::new(r))
        }),
        value(Rest::Discard, ws(tag("..."))),
    ))(input)
}

fn pattern_array<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    delimited(
        ws(tag("[")),
        alt((
            map(pattern_rest, |r| Pattern::Array(vec![], r)),
            map(
                tuple((
                    separated_list0(ws(tag(",")), map(pattern, ArrayPatternItem::Pattern)),
                    opt(preceded(ws(tag(",")), pattern_rest)),
                )),
                |(items, rest)| Pattern::Array(items, rest.unwrap_or(Rest::Exact)),
            ),
        )),
        ws(tag("]")),
    )(input)
}

fn pattern_capture<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    map(
        separated_pair(
            ws(identifier),
            ws(tag("@")),
            alt((pattern_atom, pattern_array, pattern_object)),
        ),
        |(id, pat)| Pattern::Capture(id, Box::new(pat)),
    )(input)
}

fn pattern_atom<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    map(
        alt((
            literal_null,
            literal_string,
            literal_bool,
            literal_number,
            literal_type,
        )),
        Pattern::Literal,
    )(input)
}

pub(crate) fn pattern<'v>(input: &str) -> IResult<&str, Pattern<'v>> {
    alt((
        pattern_atom,
        pattern_capture,
        pattern_array,
        pattern_typed_identifier,
        pattern_typed_discard,
        pattern_identifier,
        pattern_discard,
        pattern_object,
    ))(input)
}

pub(crate) fn assignment_multi<'v, 'w>(input: &str) -> IResult<&str, Statement<'v, 'w>> {
    map(preceded(
        ws(tag("let ")),
        separated_list1(ws(tag(";")), 
        map(separated_pair(pattern, ws(tag("=")), expression),
        |(pattern, expression)| Assignment {
            pattern,
            expression,
        })),
    ), |assignments| Statement::AssignSet(AssignmentSet{assignments}))(input)
}

pub(crate) fn try_match_multi<'v, 'w>(input: &str) -> IResult<&str, Statement<'v, 'w>> {
    map(
        separated_list1(ws(tag(";")), map(separated_pair(pattern, ws(tag("=")), expression),
        |(pattern, expression)| Assignment {
            pattern,
            expression,
        }),
    ), |assignments| Statement::MatchSet(AssignmentSet{assignments}))(input)
}

fn filename(input: &str) -> IResult<&str, &str> {
    recognize(many1(alt((alpha1, tag("_")))))(input)
}

pub(crate) fn statement<'a, 'b>(input: &str) -> IResult<&str, Statement<'a, 'b>> {
    all_consuming(alt((
        all_consuming(value(Statement::Clear, tag(".clear"))),
        all_consuming(value(Statement::Exit, ws(alt((tag(".exit"), tag(".quit")))))),
        all_consuming(value(Statement::Help, ws(alt((tag(".help"), tag(".h")))))),
        map(preceded(ws(tag(".load ")), filename), |f| {
            Statement::Import(Cow::Owned(f.into()))
        }),
        map(preceded(ws(tag(".dump ")), filename), |f| {
            Statement::Export(Cow::Owned(f.into()))
        }),
        map(
            preceded(ws(tag(".inspect ")), full_expression),
            Statement::Inspect,
        ),
        map(
            preceded(ws(tag(".format ")), full_expression),
            Statement::Format,
        ),
        map(
            preceded(ws(tag(".insert ")), expression_bag),
            Statement::Insert,
        ),
        map(preceded(ws(tag(".pop ")), full_expression), Statement::Pop),
        map(
            preceded(ws(tag(".pattern ")), full_pattern),
            Statement::Pattern,
        ),
        map(
            preceded(
                ws(tag(".delete ")),
                tuple((
                    ws(pattern),
                    opt(preceded(ws(tag("where")), expression)),
                    opt(preceded(ws(tag("limit")), nom::character::complete::u32)),
                )),
            ),
            |(pattern, guard, limit)| {
                Statement::Deletion(Predicate {
                    pattern,
                    guard: guard.unwrap_or(Expression::Literal(Literal::Boolean(true))),
                    limit: limit.map(|l| l as usize),
                })
            },
        ),
        map(
            tuple((
                ws(alt((
                    value(true, tag(".queryx ")),
                    value(false, tag(".query ")),
                ))),
                tuple((
                    separated_list1(ws(tag(";")), ws(pattern)),
                    opt(preceded(ws(tag("into")), expression)),
                    opt(preceded(ws(tag("where")), expression)),
                    opt(preceded(ws(tag("limit")), nom::character::complete::u32)),
                )),
            )),
            |(outer, (patterns, proj, guard, limit))| {
                Statement::Query(Query {
                    outer,
                    projection: proj.unwrap_or_else(|| {
                        if patterns.len() == 1 {
                            Expression::Identifier(Identifier {
                                name: Cow::Borrowed("$0"),
                            })
                        } else {
                            Expression::Array(
                                (0..patterns.len())
                                    .map(|i| {
                                        ArrayItem::Single(Expression::Identifier(Identifier {
                                            name: Cow::Owned(format!("${i}")),
                                        }))
                                    })
                                    .collect(),
                            )
                        }
                    }),
                    predicate: CrossPredicate {
                        patterns: patterns
                            .into_iter()
                            .enumerate()
                            .map(|(i, p)| {
                                Pattern::Capture(
                                    Identifier {
                                        name: Cow::Owned(format!("${i}")),
                                    },
                                    Box::new(p),
                                )
                            })
                            .collect(),
                        guard: guard.unwrap_or(Expression::Literal(Literal::Boolean(true))),
                        limit: limit.map(|l| l as usize),
                    },
                })
            },
        ),
        map(
            preceded(
                ws(tuple((tag(".query"), opt(tag(" "))))),
                opt(preceded(ws(tag("limit")), nom::character::complete::u32)),
            ),
            |limit| {
                Statement::Query(Query {
                    outer: false,
                    projection: Expression::Identifier(Identifier {
                        name: Cow::Borrowed("$"),
                    }),
                    predicate: CrossPredicate {
                        patterns: vec![Pattern::Identifier(Identifier {
                            name: Cow::Borrowed("$"),
                        })],
                        guard: Expression::Literal(Literal::Boolean(true)),
                        limit: limit.map(|l| l as usize),
                    },
                })
            },
        ),
        map(
            preceded(ws(tag(".literal ")), full_expression),
            Statement::Literal,
        ),
        value(Statement::TellBag, all_consuming(ws(tag(".bag")))),
        map(
            preceded(ws(tag(".bag ")), all_consuming(ws(identifier))),
            |p| Statement::UseBag(p,None),
        ),

        map(
            preceded(ws(tag(".bag ")), tuple((
                identifier,
                preceded(ws(tag("as")), pattern),
                opt(preceded(ws(tag("where")), expression)),
                opt(preceded(ws(tag("limit")), nom::character::complete::u32)),
            ))),
            |(name, pattern, guard, limit)| Statement::UseBag(name,Some(Predicate {
                pattern,
                guard: guard.unwrap_or(Expression::Literal(Literal::Boolean(true))),
                limit: limit.map(|l| l as usize),
            })),
        ),
        alt((
            all_consuming(assignment_multi),
            all_consuming(try_match_multi),
        )),
        all_consuming(map(expression_multi, Statement::Eval)),
    )))(input)
}
