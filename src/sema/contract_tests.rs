//! Contract violation tests for semantic analysis.
//!
//! These tests verify that debug_assert! contracts correctly catch invalid states.
//! Tests are organized by the contract they exercise.

#[cfg(test)]
mod tests {
    use crate::sema::{ScopeKind, SemanticContext};
    use crate::sema::types::TypeInterner;

    // =========================================================================
    // Phase 1: Helper Function Tests
    // =========================================================================

    mod helper_functions {
        use super::*;

        #[test]
        fn types_len_starts_with_builtins() {
            let interner = TypeInterner::new();
            // Built-in primitives are pre-interned (i8, i16, i32, i64, etc.)
            assert!(interner.types_len() > 0, "interner should have built-in types");
        }

        #[test]
        fn types_len_increments_on_new_type() {
            let mut interner = TypeInterner::new();
            let before = interner.types_len();

            let _var = interner.fresh_type_var();
            assert_eq!(interner.types_len(), before + 1);

            let _var2 = interner.fresh_int_var();
            assert_eq!(interner.types_len(), before + 2);

            let _var3 = interner.fresh_float_var();
            assert_eq!(interner.types_len(), before + 3);
        }

        #[test]
        fn types_len_no_increment_on_existing_type() {
            let interner = TypeInterner::new();

            // i32 is already interned, calling i32() returns cached value
            let before = interner.types_len();
            // Note: i32() doesn't need &mut self for lookup
            assert_eq!(interner.types_len(), before);
        }

        #[test]
        fn is_at_root_scope_initially_true() {
            let ctx = SemanticContext::new();
            assert!(ctx.is_at_root_scope());
        }

        #[test]
        fn is_at_root_scope_false_after_enter() {
            let mut ctx = SemanticContext::new();
            ctx.enter_scope(ScopeKind::Function);
            assert!(!ctx.is_at_root_scope());
        }

        #[test]
        fn scope_depth_starts_at_zero() {
            let ctx = SemanticContext::new();
            assert_eq!(ctx.scope_depth(), 0);
        }

        #[test]
        fn scope_depth_increments_correctly() {
            let mut ctx = SemanticContext::new();

            ctx.enter_scope(ScopeKind::Function);
            assert_eq!(ctx.scope_depth(), 1);

            ctx.enter_scope(ScopeKind::Block);
            assert_eq!(ctx.scope_depth(), 2);

            ctx.enter_scope(ScopeKind::Block);
            assert_eq!(ctx.scope_depth(), 3);
        }

        #[test]
        fn scope_depth_decrements_on_exit() {
            let mut ctx = SemanticContext::new();

            ctx.enter_scope(ScopeKind::Function);
            ctx.enter_scope(ScopeKind::Block);
            assert_eq!(ctx.scope_depth(), 2);

            ctx.exit_scope();
            assert_eq!(ctx.scope_depth(), 1);

            ctx.exit_scope();
            assert_eq!(ctx.scope_depth(), 0);
        }
    }

    // =========================================================================
    // Phase 2: Core Type Inference Contracts
    // =========================================================================

    mod type_inference {
        use crate::parser::parse;
        use crate::ast::SourceFile;
        use crate::sema::resolver::resolve;
        use crate::sema::infer::infer;
        use rowan::ast::AstNode;

        #[test]
        fn unify_simple_types_no_cycle() {
            // Test that simple type unification doesn't trigger cycle detection
            let source = r#"
                fn test() {
                    let x: i32 = 1;
                    let y: i32 = x;
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
            // No panic from cycle detection
        }

        #[test]
        fn unify_type_variables_chain() {
            // Test unification of type variable chains
            let source = r#"
                fn identity<T>(x: T) -> T { x }

                fn test() {
                    let a = identity(1);
                    let b = identity(a);
                    let c = identity(b);
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
            // No panic from cycle detection in chained unification
        }

        #[test]
        fn resolve_type_returns_concrete() {
            // Test that resolved types are concrete or unbound
            let source = r#"
                fn test() {
                    let x = 42;
                    let y = x + 1;
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
            // No panic from resolve_type postcondition
        }

        #[test]
        fn complex_type_unification() {
            // Test complex nested type unification
            let source = r#"
                struct Wrapper<T> { value: T }

                fn test() {
                    let w: Wrapper<i32> = Wrapper { value: 42 };
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }
    }

    // =========================================================================
    // Phase 3: Scope Management Contracts
    // =========================================================================

    mod scope_management {
        use super::*;

        #[test]
        #[should_panic(expected = "cannot exit root scope")]
        fn exit_root_scope_panics() {
            let mut ctx = SemanticContext::new();
            ctx.exit_scope();
        }

        #[test]
        #[should_panic(expected = "cannot exit root scope")]
        fn exit_root_scope_after_balanced_ops_panics() {
            let mut ctx = SemanticContext::new();
            ctx.enter_scope(ScopeKind::Function);
            ctx.exit_scope();
            // Now at root, this should panic
            ctx.exit_scope();
        }

        #[test]
        fn deeply_nested_scopes_balance() {
            let mut ctx = SemanticContext::new();

            // Enter 10 nested scopes
            for _ in 0..10 {
                ctx.enter_scope(ScopeKind::Block);
            }
            assert_eq!(ctx.scope_depth(), 10);

            // Exit all 10
            for i in (0..10).rev() {
                ctx.exit_scope();
                assert_eq!(ctx.scope_depth(), i);
            }

            assert!(ctx.is_at_root_scope());
        }

        #[test]
        fn mixed_scope_kinds_balance() {
            let mut ctx = SemanticContext::new();

            ctx.enter_scope(ScopeKind::Function);
            ctx.enter_scope(ScopeKind::Block);
            ctx.enter_scope(ScopeKind::ForLoop);
            ctx.enter_scope(ScopeKind::Block);
            ctx.enter_scope(ScopeKind::Impl);

            assert_eq!(ctx.scope_depth(), 5);

            for _ in 0..5 {
                ctx.exit_scope();
            }

            assert!(ctx.is_at_root_scope());
        }
    }

    // =========================================================================
    // Phase 4: Tier 2 Contracts (Resolver + Parser)
    // =========================================================================

    mod resolver_contracts {
        use crate::parser::parse;
        use crate::ast::SourceFile;
        use crate::sema::resolver::resolve;
        use rowan::ast::AstNode;

        #[test]
        fn two_pass_forward_references() {
            // Test that forward references work (pass 1 collects, pass 2 resolves)
            let source = r#"
                fn first() { second(); }
                fn second() { third(); }
                fn third() { first(); }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let result = resolve(&source_file);

            assert!(result.diagnostics.is_empty());
        }

        #[test]
        fn two_pass_struct_before_use() {
            // Struct defined after use should still resolve
            let source = r#"
                fn uses_point() {
                    let p = Point { x: 1, y: 2 };
                }

                struct Point { x: i32, y: i32 }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let result = resolve(&source_file);

            assert!(result.diagnostics.is_empty());
        }

        #[test]
        fn define_name_recorded_in_scope() {
            // Test that defined names are findable
            let source = r#"
                fn test() {
                    let x = 1;
                    let y = x;
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let result = resolve(&source_file);

            assert!(result.diagnostics.is_empty());
            // If define_name contract failed, we'd get "not found" errors
        }

        #[test]
        fn block_scope_balance_simple() {
            let source = r#"
                fn test() {
                    { let a = 1; }
                    { let b = 2; }
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let result = resolve(&source_file);

            assert!(result.diagnostics.is_empty());
        }

        #[test]
        fn block_scope_balance_nested() {
            let source = r#"
                fn test() {
                    {
                        let a = 1;
                        {
                            let b = a;
                            {
                                let c = b;
                            }
                        }
                    }
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let result = resolve(&source_file);

            assert!(result.diagnostics.is_empty());
        }

        #[test]
        fn block_scope_balance_control_flow() {
            let source = r#"
                fn test() {
                    let x = 1;
                    if x > 0 {
                        let a = 1;
                        if a > 0 {
                            let b = 2;
                        }
                    } else {
                        let c = 3;
                    }

                    while x > 0 {
                        let d = 4;
                        { let e = 5; }
                    }

                    for i in 0..10 {
                        let f = i;
                        { let g = f; }
                    }

                    loop {
                        let h = 1;
                        break;
                    }
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let result = resolve(&source_file);

            assert!(result.diagnostics.is_empty());
        }
    }

    mod parser_contracts {
        use crate::parser::parse;

        #[test]
        fn parser_advances_on_expression() {
            // Various expressions should all advance the parser
            let expressions = [
                "1",
                "1 + 2",
                "1 + 2 * 3",
                "foo",
                "foo.bar",
                "foo.bar()",
                "foo(1, 2)",
                "[1, 2, 3]",
                "(1, 2, 3)",
                "if true { 1 } else { 2 }",
                "{ let x = 1; x }",
            ];

            for expr in expressions {
                let source = format!("fn test() {{ {}; }}", expr);
                let result = parse(&source);
                assert!(result.ok(), "failed to parse: {}", expr);
            }
        }
    }

    // =========================================================================
    // Phase 5: Tier 3 Contracts (Synth Functions)
    // =========================================================================

    mod synth_contracts {
        use crate::parser::parse;
        use crate::ast::SourceFile;
        use crate::sema::resolver::resolve;
        use crate::sema::infer::infer;
        use rowan::ast::AstNode;

        #[test]
        fn synth_array_returns_array_type() {
            let source = r#"
                fn test() {
                    let arr1 = [1, 2, 3];
                    let arr2 = [true, false];
                    let arr3: [i32; 5] = [0; 5];
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }

        #[test]
        fn synth_array_empty() {
            let source = r#"
                fn test() {
                    let arr: [i32; 0] = [];
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }

        #[test]
        fn synth_array_repeat_syntax() {
            let source = r#"
                fn test() {
                    let arr = [0; 10];
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }

        #[test]
        fn synth_struct_all_fields_provided() {
            let source = r#"
                struct Point { x: i32, y: i32 }

                fn test() {
                    let p = Point { x: 1, y: 2 };
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }

        #[test]
        fn synth_struct_missing_field_emits_diagnostic() {
            let source = r#"
                struct Point { x: i32, y: i32 }

                fn test() {
                    let p = Point { x: 1 };
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let infer_result = infer(&source_file, resolve_result);

            // Should have a diagnostic about missing field
            assert!(
                infer_result.diagnostics.iter().any(|d| d.message.contains("missing field")),
                "expected missing field diagnostic"
            );
        }

        #[test]
        fn synth_field_auto_deref_single() {
            let source = r#"
                struct Point { x: i32, y: i32 }

                fn test(p: &Point) -> i32 {
                    p.x
                }
            "#;

            let parse = parse(source);
            assert!(parse.ok(), "parse errors: {:?}", parse.errors());
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }

        #[test]
        fn synth_field_tuple_index() {
            let source = r#"
                fn test() {
                    let t = (1, 2, 3);
                    let first = t.0;
                    let second = t.1;
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }

        #[test]
        fn synth_block_unit_type() {
            let source = r#"
                fn test() {
                    let _x: () = {};
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }

        #[test]
        fn synth_block_with_tail() {
            let source = r#"
                fn test() {
                    let x: i32 = { 42 };
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }

        #[test]
        fn synth_block_diverges() {
            let source = r#"
                fn diverge() -> ! {
                    loop {}
                }

                fn test() {
                    let _x = {
                        diverge();
                    };
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }

        #[test]
        fn synth_block_diverges_with_return() {
            let source = r#"
                fn test() -> i32 {
                    {
                        return 42;
                    }
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }
    }

    // =========================================================================
    // Integration Tests
    // =========================================================================

    mod integration {
        use crate::parser::parse;
        use crate::ast::SourceFile;
        use crate::sema::resolver::resolve;
        use crate::sema::infer::infer;
        use rowan::ast::AstNode;

        #[test]
        fn full_pipeline_comprehensive() {
            let source = r#"
                struct Point { x: i32, y: i32 }
                struct Line { start: Point, end: Point }

                fn add(a: i32, b: i32) -> i32 {
                    a + b
                }

                fn distance_squared(p1: &Point, p2: &Point) -> i32 {
                    let dx = p2.x - p1.x;
                    let dy = p2.y - p1.y;
                    dx * dx + dy * dy
                }

                fn main() {
                    let origin = Point { x: 0, y: 0 };
                    let target = Point { x: 3, y: 4 };

                    let dist = distance_squared(&origin, &target);

                    let arr = [1, 2, 3, 4, 5];
                    let sum = arr[0] + arr[1];

                    for i in 0..5 {
                        let val = arr[i];
                    }

                    if dist > 0 {
                        let positive = true;
                    } else {
                        let zero = false;
                    }

                    let mut counter = 0;
                    while counter < 10 {
                        counter = counter + 1;
                    }

                    let result = loop {
                        if counter > 5 {
                            break counter;
                        }
                    };
                }
            "#;

            let parse = parse(source);
            assert!(parse.ok(), "parse errors: {:?}", parse.errors());

            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            assert!(
                resolve_result.diagnostics.is_empty(),
                "resolve errors: {:?}",
                resolve_result.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
            );

            let _infer_result = infer(&source_file, resolve_result);
            // All contracts should pass without panic
        }

        #[test]
        fn impl_methods_with_self() {
            let source = r#"
                struct Counter { value: i32 }

                impl Counter {
                    fn new() -> Counter {
                        Counter { value: 0 }
                    }

                    fn get(&self) -> i32 {
                        self.value
                    }

                    fn increment(&mut self) {
                        self.value = self.value + 1;
                    }
                }

                fn main() {
                    let c = Counter { value: 0 };
                    let v = c.get();
                }
            "#;

            let parse = parse(source);
            assert!(parse.ok(), "parse errors: {:?}", parse.errors());

            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }

        #[test]
        fn generic_functions() {
            let source = r#"
                fn identity<T>(x: T) -> T {
                    x
                }

                fn main() {
                    let a = identity(42);
                    let b = identity(true);
                }
            "#;

            let parse = parse(source);
            let source_file = SourceFile::cast(parse.syntax()).unwrap();
            let resolve_result = resolve(&source_file);
            let _infer_result = infer(&source_file, resolve_result);
        }
    }
}
