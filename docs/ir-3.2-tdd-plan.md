# IR-3.2: Lower Variables and Arithmetic Expressions - TDD Plan

## Overview

This document describes the test-driven development plan for implementing MIR lowering of variable references, binary operations, and unary operations.

## Dependencies

- **Completed**: IR-3.1 (MIR lowering infrastructure and literals)
- **Blocked by this**: IR-3.3 (control flow), IR-3.6 (aggregates), NATIVE-2 (CLIF lowering)

## Scope

### In Scope

1. **Variable References** (`HirExprKind::Var`)
   - Look up DefId in local_map to get MIR Local
   - Return Operand::Copy(Place::from_local(local))

2. **Binary Operations** (`HirExprKind::Binary`)
   - Arithmetic: Add, Sub, Mul, Div, Rem
   - Comparison: Eq, Ne, Lt, Le, Gt, Ge
   - Map HIR BinOp → MIR BinOp
   - Generate: lower LHS, lower RHS, allocate temp, assign BinaryOp

3. **Unary Operations** (`HirExprKind::Unary`)
   - Not (logical/bitwise negation)
   - Neg (arithmetic negation)
   - Map HIR UnaryOp → MIR UnOp

### Out of Scope (Later Issues)

- `BinOp::And/Or` - Short-circuit evaluation (IR-3.3: control flow)
- `BinOp::Assign/*Assign` - Assignment statements (IR-3.4: statements)
- `UnaryOp::Deref` - Dereference to place (IR-3.7: place expressions)

## Implementation Plan

### Phase 1: Operator Mapping Functions

Add conversion functions in `src/mir/lower.rs`:

```rust
fn hir_binop_to_mir(op: crate::hir::BinOp) -> Option<BinOp>
fn hir_unop_to_mir(op: crate::hir::UnaryOp) -> Option<UnOp>
```

### Phase 2: Variable Lowering

Extend `lower_expr_to_place` and `lower_expr_as_operand` to handle `HirExprKind::Var`:
- Look up DefId in `self.local_map`
- Return Copy of that local

### Phase 3: Binary Expression Lowering

Extend `lower_expr_to_place` to handle `HirExprKind::Binary`:
1. Recursively lower LHS as operand
2. Recursively lower RHS as operand
3. Allocate temp for result
4. Emit `Assign(temp, BinaryOp(op, lhs, rhs))`
5. Return temp place

### Phase 4: Unary Expression Lowering

Extend `lower_expr_to_place` to handle `HirExprKind::Unary`:
1. Recursively lower operand
2. Allocate temp for result
3. Emit `Assign(temp, UnaryOp(op, operand))`
4. Return temp place

## Test Plan

### Unit Tests: Operator Mapping

```rust
#[test] fn test_hir_binop_add_to_mir()
#[test] fn test_hir_binop_sub_to_mir()
#[test] fn test_hir_binop_mul_to_mir()
#[test] fn test_hir_binop_div_to_mir()
#[test] fn test_hir_binop_rem_to_mir()
#[test] fn test_hir_binop_eq_to_mir()
#[test] fn test_hir_binop_ne_to_mir()
#[test] fn test_hir_binop_lt_to_mir()
#[test] fn test_hir_binop_le_to_mir()
#[test] fn test_hir_binop_gt_to_mir()
#[test] fn test_hir_binop_ge_to_mir()
#[test] fn test_hir_binop_and_returns_none()  // Short-circuit: not handled here
#[test] fn test_hir_binop_or_returns_none()   // Short-circuit: not handled here
#[test] fn test_hir_binop_assign_returns_none() // Assignment: not handled here
#[test] fn test_hir_unop_not_to_mir()
#[test] fn test_hir_unop_neg_to_mir()
#[test] fn test_hir_unop_deref_returns_none()  // Deref: not handled here
```

### Integration Tests: Variable Lowering

```rust
#[test] fn test_lower_var_reference()
// fn use_x(x: i32) -> i32 { x }
// Should produce: _0 = _1; return

#[test] fn test_lower_multiple_var_refs()
// fn add_params(a: i32, b: i32) -> i32 { a }
// Verify both params mapped correctly

#[test] fn test_lower_var_in_expression()
// Variable used as part of binary expression
```

### Integration Tests: Binary Expressions

```rust
#[test] fn test_lower_binary_add()
// fn add() -> i32 { 1 + 2 }
// Should produce: _1 = 1; _2 = 2; _3 = Add(_1, _2); _0 = _3; return
// Or optimized: _1 = Add(Const(1), Const(2)); _0 = _1; return

#[test] fn test_lower_binary_sub()
#[test] fn test_lower_binary_mul()
#[test] fn test_lower_binary_div()
#[test] fn test_lower_binary_rem()
#[test] fn test_lower_binary_comparison_eq()
#[test] fn test_lower_binary_comparison_lt()
#[test] fn test_lower_nested_binary()
// fn nested() -> i32 { (1 + 2) * 3 }
// Tests correct evaluation order

#[test] fn test_lower_binary_with_vars()
// fn add(a: i32, b: i32) -> i32 { a + b }
```

### Integration Tests: Unary Expressions

```rust
#[test] fn test_lower_unary_neg()
// fn neg(x: i32) -> i32 { -x }

#[test] fn test_lower_unary_not()
// fn flip(b: bool) -> bool { !b }

#[test] fn test_lower_nested_unary()
// fn double_neg(x: i32) -> i32 { --x }
```

### Edge Cases

```rust
#[test] fn test_lower_complex_expression()
// fn complex(a: i32, b: i32) -> i32 { -(a + b) * 2 }

#[test] fn test_lower_comparison_chain()
// fn in_range(x: i32) -> bool { x > 0 }
// Note: `0 < x < 10` would need && which is control flow
```

## File Changes

- `src/mir/lower.rs`: Add operator mapping + extend lowering functions
- `src/mir/mod.rs`: Re-export new functions if public

## Success Criteria

1. All tests pass
2. `cargo clippy --all-targets -- -D warnings` passes
3. Variable references correctly map DefId → Local
4. All arithmetic BinOps correctly lowered
5. All comparison BinOps correctly lowered
6. Not and Neg UnOps correctly lowered
7. Nested expressions produce correct temp allocation
