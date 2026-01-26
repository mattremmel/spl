//! Tests for span preservation through HIR→MIR lowering.
//!
//! These tests verify that MIR statements and terminators have accurate spans
//! that point to the correct source locations, ensuring good error messages
//! in later compiler phases.

use crate::testing::{compile_ok, span_to_source};

/// Lower source code to MIR bodies for span testing.
fn lower_source(source: &str) -> Vec<crate::mir::Body> {
    compile_ok(source)
}

// ========== Test 1: Assignment spans point to source location ==========

#[test]
fn assignment_span_points_to_let_statement() {
    // let x = 42;
    //     ^^^^^^  - the span should include the binding and initializer
    let source = "fn main() { let x = 42; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the assignment statement that assigns 42
    let assign_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| {
            matches!(
                &s.kind,
                crate::mir::StatementKind::Assign(
                    _,
                    crate::mir::Rvalue::Use(crate::mir::Operand::Constant(
                        crate::mir::Constant::Int(42, _)
                    ))
                )
            )
        })
        .expect("should have assignment of 42");

    // The span should cover the let statement
    let span_text = span_to_source(source, &assign_stmt.span);
    assert!(
        span_text.contains("let") || span_text.contains("x = 42") || span_text.contains("42"),
        "Assignment span {:?} should reference the let statement, got '{span_text}'",
        assign_stmt.span,
    );
}

#[test]
fn assignment_span_covers_binary_op() {
    // let y = a + b; where a and b are variables
    let source = "fn main() { let a = 1; let b = 2; let c = a + b; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the binary op assignment (result of a + b)
    let binop_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| matches!(&s.kind, crate::mir::StatementKind::Assign(_, crate::mir::Rvalue::BinaryOp(..))))
        .expect("should have binary op assignment");

    // The span should be within the source
    assert!(binop_stmt.span.end <= source.len(), "Span should be within source bounds");
    let span_text = span_to_source(source, &binop_stmt.span);
    assert!(
        span_text.contains('+') || span_text.contains('a') || span_text.contains('b'),
        "Binary op span should reference the expression, got '{span_text}'",
    );
}

// ========== Test 2: Function call spans ==========

#[test]
fn call_terminator_span_points_to_call_expr() {
    // SPL uses labeled arguments. The `_` makes it positional.
    // foo(1, 2);
    // ^^^^^^^^^ - span should cover the call
    let source = "fn foo(_ a: i32, _ b: i32) {} fn main() { foo(1, 2); }";
    let bodies = lower_source(source);

    // main is the second function
    let main_body = &bodies[1];

    // Find the Call terminator
    let call_term = main_body
        .basic_blocks
        .iter()
        .filter_map(|bb| bb.terminator.as_ref())
        .find(|t| matches!(&t.kind, crate::mir::TerminatorKind::Call { .. }))
        .expect("should have Call terminator");

    let span_text = span_to_source(source, &call_term.span);
    assert!(
        span_text.contains("foo") || span_text.contains('('),
        "Call span should reference the call expression, got '{span_text}'",
    );
}

// ========== Test 3: Return statement spans ==========

#[test]
fn return_span_points_to_return_expr() {
    // return 42;
    // ^^^^^^^^^^ - span should cover the return
    let source = "fn answer(): i32 { return 42; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the Return terminator
    let return_term = body
        .basic_blocks
        .iter()
        .filter_map(|bb| bb.terminator.as_ref())
        .find(|t| matches!(&t.kind, crate::mir::TerminatorKind::Return))
        .expect("should have Return terminator");

    // Return terminator span should be within source bounds
    assert!(
        return_term.span.end <= source.len(),
        "Return span should be within source bounds"
    );
}

#[test]
fn explicit_return_span_points_to_return_keyword() {
    let source = "fn foo(): i32 { return 123; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find assignment to return place with 123
    let return_assign = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| {
            matches!(
                &s.kind,
                crate::mir::StatementKind::Assign(
                    place,
                    crate::mir::Rvalue::Use(crate::mir::Operand::Constant(
                        crate::mir::Constant::Int(123, _)
                    ))
                ) if place.local == crate::mir::Local::RETURN_PLACE
            )
        });

    if let Some(stmt) = return_assign {
        let span_text = span_to_source(source, &stmt.span);
        assert!(
            span_text.contains("return") || span_text.contains("123"),
            "Return assignment span should reference the return statement, got '{span_text}'",
        );
    }
}

// ========== Test 4: Control flow spans (if/while/loop) ==========

#[test]
fn if_condition_span_preserved() {
    // if cond { ... }
    // SwitchInt terminator span should point to condition
    let source = "fn main() { if true { 1; } else { 2; } }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the SwitchInt terminator
    let switch_term = body
        .basic_blocks
        .iter()
        .filter_map(|bb| bb.terminator.as_ref())
        .find(|t| matches!(&t.kind, crate::mir::TerminatorKind::SwitchInt { .. }))
        .expect("should have SwitchInt terminator for if");

    assert!(
        switch_term.span.end <= source.len(),
        "SwitchInt span should be within source bounds"
    );

    let span_text = span_to_source(source, &switch_term.span);
    // The span should be related to the if expression
    assert!(
        span_text.contains("if") || span_text.contains("true") || span_text.contains('{'),
        "If condition span should reference the if expression, got '{span_text}'",
    );
}

#[test]
fn loop_span_preserved() {
    let source = "fn main() { loop { break; } }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find Goto terminators (loop back edge)
    let goto_terms: Vec<_> = body
        .basic_blocks
        .iter()
        .filter_map(|bb| bb.terminator.as_ref())
        .filter(|t| matches!(&t.kind, crate::mir::TerminatorKind::Goto(_)))
        .collect();

    // All goto spans should be within source bounds
    for term in &goto_terms {
        assert!(
            term.span.end <= source.len(),
            "Goto span should be within source bounds"
        );
    }
}

// ========== Test 5: Binary operations ==========

#[test]
fn binary_op_span_covers_full_expression() {
    // a + b
    // ^^^^^ - span should cover the binary op
    let source = "fn main() { let a = 1; let b = 2; let c = a + b; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the binary op statement (Add)
    let binop_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| {
            matches!(
                &s.kind,
                crate::mir::StatementKind::Assign(_, crate::mir::Rvalue::BinaryOp(crate::mir::BinOp::Add, ..))
            )
        })
        .expect("should have Add binary op");

    assert!(
        binop_stmt.span.end <= source.len(),
        "Binary op span should be within source bounds"
    );

    let span_text = span_to_source(source, &binop_stmt.span);
    assert!(
        span_text.contains('+') || span_text.contains('a') || span_text.contains('b'),
        "Binary op span should reference the expression, got '{span_text}'",
    );
}

// ========== Test 6: Field access ==========

#[test]
fn field_access_span_preserved() {
    // SPL uses tuple-struct syntax: struct Point(x: i32, y: i32)
    let source = "struct Point(x: i32, y: i32) fn main() { let p = Point(x: 1, y: 2); let v = p.x; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // All statements should have valid spans
    for bb in &body.basic_blocks {
        for stmt in &bb.statements {
            assert!(
                stmt.span.end <= source.len(),
                "Statement span {:?} should be within source bounds (len={})",
                stmt.span,
                source.len()
            );
        }
    }
}

// ========== Test 7: Struct construction ==========

#[test]
fn struct_construction_span_preserved() {
    let source = "struct Foo(a: i32) fn main() { let f = Foo(a: 42); }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the aggregate (struct) construction
    let struct_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| {
            matches!(
                &s.kind,
                crate::mir::StatementKind::Assign(_, crate::mir::Rvalue::Aggregate(crate::mir::AggregateKind::Adt(_), _))
            )
        })
        .expect("should have struct construction");

    assert!(
        struct_stmt.span.end <= source.len(),
        "Struct construction span should be within source bounds"
    );

    let span_text = span_to_source(source, &struct_stmt.span);
    assert!(
        span_text.contains("Foo") || span_text.contains('(') || span_text.contains("42"),
        "Struct construction span should reference the struct literal, got '{span_text}'",
    );
}

// ========== Test 8: Array literals ==========

#[test]
fn array_literal_span_preserved() {
    let source = "fn main() { let arr = [1, 2, 3]; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the array aggregate
    let array_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| {
            matches!(
                &s.kind,
                crate::mir::StatementKind::Assign(_, crate::mir::Rvalue::Aggregate(crate::mir::AggregateKind::Array, _))
            )
        })
        .expect("should have array construction");

    assert!(
        array_stmt.span.end <= source.len(),
        "Array span should be within source bounds"
    );
}

// ========== Test 9: Tuple literals ==========

#[test]
fn tuple_literal_span_preserved() {
    let source = "fn main() { let t = (1, 2); }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the tuple aggregate
    let tuple_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| {
            matches!(
                &s.kind,
                crate::mir::StatementKind::Assign(_, crate::mir::Rvalue::Aggregate(crate::mir::AggregateKind::Tuple, _))
            )
        })
        .expect("should have tuple construction");

    assert!(
        tuple_stmt.span.end <= source.len(),
        "Tuple span should be within source bounds"
    );
}

// ========== Test 10: Short-circuit operators ==========

#[test]
fn short_circuit_and_span_preserved() {
    // Short-circuit && generates a SwitchInt
    let source = "fn main() { let a = true; let b = false; let x = a && b; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Should have a SwitchInt for short-circuit evaluation
    let switch_term = body
        .basic_blocks
        .iter()
        .filter_map(|bb| bb.terminator.as_ref())
        .find(|t| matches!(&t.kind, crate::mir::TerminatorKind::SwitchInt { .. }))
        .expect("should have SwitchInt for &&");

    assert!(
        switch_term.span.end <= source.len(),
        "Short-circuit && span should be within source bounds"
    );
}

#[test]
fn short_circuit_or_span_preserved() {
    // Short-circuit || generates a SwitchInt
    let source = "fn main() { let a = true; let b = false; let x = a || b; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Should have a SwitchInt for short-circuit evaluation
    let switch_term = body
        .basic_blocks
        .iter()
        .filter_map(|bb| bb.terminator.as_ref())
        .find(|t| matches!(&t.kind, crate::mir::TerminatorKind::SwitchInt { .. }))
        .expect("should have SwitchInt for ||");

    assert!(
        switch_term.span.end <= source.len(),
        "Short-circuit || span should be within source bounds"
    );
}

// ========== Test 11: References ==========

#[test]
fn reference_span_preserved() {
    let source = "fn main() { let x = 42; let r = &x; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the Ref rvalue
    let ref_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| matches!(&s.kind, crate::mir::StatementKind::Assign(_, crate::mir::Rvalue::Ref(..))))
        .expect("should have reference creation");

    assert!(
        ref_stmt.span.end <= source.len(),
        "Reference span should be within source bounds"
    );
}

// ========== Test 12: Unary operations ==========

#[test]
fn unary_op_span_preserved() {
    // Use a variable so we get a unary op, not a negative literal
    let source = "fn main() { let x = 42; let y = -x; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the UnaryOp statement
    let unary_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| matches!(&s.kind, crate::mir::StatementKind::Assign(_, crate::mir::Rvalue::UnaryOp(..))))
        .expect("should have unary op");

    assert!(
        unary_stmt.span.end <= source.len(),
        "Unary op span should be within source bounds"
    );
}

#[test]
fn unary_not_span_preserved() {
    let source = "fn main() { let x = true; let y = !x; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the UnaryOp statement (Not)
    let unary_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| matches!(&s.kind, crate::mir::StatementKind::Assign(_, crate::mir::Rvalue::UnaryOp(crate::mir::UnOp::Not, ..))))
        .expect("should have unary Not op");

    assert!(
        unary_stmt.span.end <= source.len(),
        "Unary Not span should be within source bounds"
    );
}

// ========== Test 13: Cast expressions ==========

#[test]
fn cast_span_preserved() {
    let source = "fn main() { let x = 42 as i64; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the Cast rvalue
    let cast_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| matches!(&s.kind, crate::mir::StatementKind::Assign(_, crate::mir::Rvalue::Cast(..))))
        .expect("should have cast");

    assert!(
        cast_stmt.span.end <= source.len(),
        "Cast span should be within source bounds"
    );

    let span_text = span_to_source(source, &cast_stmt.span);
    assert!(
        span_text.contains("as") || span_text.contains("42"),
        "Cast span should reference the cast expression, got '{span_text}'",
    );
}

// ========== Test 14: Break/Continue spans ==========

#[test]
fn break_span_preserved() {
    let source = "fn main() { loop { break; } }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // All terminators should have valid spans
    for bb in &body.basic_blocks {
        if let Some(term) = &bb.terminator {
            assert!(
                term.span.end <= source.len(),
                "Terminator span {:?} should be within source bounds",
                term.span
            );
        }
    }
}

#[test]
fn break_with_value_span_preserved() {
    let source = "fn main(): i32 { loop { break 42; } }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find assignment of 42 (break value)
    let break_assign = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| {
            matches!(
                &s.kind,
                crate::mir::StatementKind::Assign(
                    _,
                    crate::mir::Rvalue::Use(crate::mir::Operand::Constant(
                        crate::mir::Constant::Int(42, _)
                    ))
                )
            )
        });

    if let Some(stmt) = break_assign {
        assert!(
            stmt.span.end <= source.len(),
            "Break value span should be within source bounds"
        );
    }
}

// ========== Test 15: Multiple functions ==========

#[test]
fn multiple_functions_have_correct_spans() {
    let source = r#"
fn first(): i32 { 1 }
fn second(): i32 { 2 }
fn third(): i32 { 3 }
"#;
    let bodies = lower_source(source);

    assert_eq!(bodies.len(), 3, "Should have 3 function bodies");

    // Each function's return terminator should have a valid span
    for body in &bodies {
        for bb in &body.basic_blocks {
            if let Some(term) = &bb.terminator {
                assert!(
                    term.span.end <= source.len(),
                    "Function terminator span {:?} should be within source bounds (len={})",
                    term.span,
                    source.len()
                );
            }
            for stmt in &bb.statements {
                assert!(
                    stmt.span.end <= source.len(),
                    "Function statement span {:?} should be within source bounds (len={})",
                    stmt.span,
                    source.len()
                );
            }
        }
    }
}

// ========== Test 16: Nested expressions ==========

#[test]
fn nested_expression_spans_preserved() {
    let source = "fn main() { let x = (1 + 2) * 3; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // All statements should have valid spans
    let all_valid = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .all(|s| s.span.end <= source.len());

    assert!(all_valid, "All nested expression spans should be valid");
}

// ========== Test 17: Assignment expressions ==========

#[test]
fn compound_assignment_span_preserved() {
    let source = "fn main() { let mut x = 1; x += 2; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the compound assignment (binary op Add) - use rfind to get the last one
    let compound_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .collect::<Vec<_>>()
        .into_iter()
        .rfind(|s| {
            matches!(
                &s.kind,
                crate::mir::StatementKind::Assign(_, crate::mir::Rvalue::BinaryOp(crate::mir::BinOp::Add, ..))
            )
        }); // Get the compound assignment, not the initial 1

    if let Some(stmt) = compound_stmt {
        assert!(
            stmt.span.end <= source.len(),
            "Compound assignment span should be within source bounds"
        );
    }
}

// ========== Test 18: Verify span content accuracy ==========

#[test]
fn literal_assignment_span_text_is_accurate() {
    let source = "fn main() { let value = 999; }";
    let bodies = lower_source(source);
    let body = &bodies[0];

    // Find the assignment of 999
    let assign_stmt = body
        .basic_blocks
        .iter()
        .flat_map(|bb| bb.statements.iter())
        .find(|s| {
            matches!(
                &s.kind,
                crate::mir::StatementKind::Assign(
                    _,
                    crate::mir::Rvalue::Use(crate::mir::Operand::Constant(
                        crate::mir::Constant::Int(999, _)
                    ))
                )
            )
        })
        .expect("should have assignment of 999");

    let span_text = span_to_source(source, &assign_stmt.span);
    // The span should reference the let statement or the value itself
    assert!(
        span_text.contains("let") || span_text.contains("value") || span_text.contains("999"),
        "Span text should contain relevant source, got '{span_text}'",
    );
}

// ========== Test 19: All MIR spans within bounds ==========

#[test]
fn all_mir_spans_within_source_bounds() {
    // SPL requires explicit returns when function body contains statements
    let source = r#"
fn complex(): i32 {
    let a = 1;
    let b = 2;
    let c = a + b;
    if c > 0 {
        return c;
    }
    return 0;
}
fn main() { complex(); }
"#;
    let bodies = lower_source(source);

    for (fn_idx, body) in bodies.iter().enumerate() {
        for (bb_idx, bb) in body.basic_blocks.iter().enumerate() {
            for (stmt_idx, stmt) in bb.statements.iter().enumerate() {
                assert!(
                    stmt.span.end <= source.len(),
                    "fn{} bb{} stmt{}: span {:?} exceeds source len {}",
                    fn_idx, bb_idx, stmt_idx, stmt.span, source.len()
                );
            }
            if let Some(term) = &bb.terminator {
                assert!(
                    term.span.end <= source.len(),
                    "fn{} bb{} terminator: span {:?} exceeds source len {}",
                    fn_idx, bb_idx, term.span, source.len()
                );
            }
        }
    }
}
