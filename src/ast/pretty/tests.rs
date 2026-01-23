use super::*;
use crate::parser::parse;
use expect_test::{Expect, expect};

fn check(source: &str, expected: &Expect) {
    let parsed = parse(source);
    let source_file = SourceFile::cast(parsed.syntax()).unwrap();
    let output = pretty_print(&source_file);
    expected.assert_eq(&output);
}

mod items {
    use super::*;

    #[test]
    fn function_empty() {
        check(
            "fn foo() {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "foo"
                    ParamList
                    Block
            "#]],
        );
    }

    #[test]
    fn function_with_params_colon() {
        check(
            "fn add(x: i32, y: i32): i32 { x + y }",
            &expect![[r#"
                SourceFile
                  FunctionDef "add"
                    ParamList
                      Param "x"
                        Path "i32"
                      Param "y"
                        Path "i32"
                    ReturnType
                      Path "i32"
                    Block
                      TailExpr
                        BinaryExpr "+"
                          Path "x"
                          Path "y"
            "#]],
        );
    }

    #[test]
    fn function_with_self() {
        check(
            "fn method(&self) {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "method"
                    ParamList
                      SelfParam "&self"
                    Block
            "#]],
        );
    }

    #[test]
    fn function_with_mut_self() {
        check(
            "fn method(&mut self) {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "method"
                    ParamList
                      SelfParam "&mut self"
                    Block
            "#]],
        );
    }

    #[test]
    fn function_generic_where() {
        check(
            "fn id(x: T): T where T { x }",
            &expect![[r#"
                SourceFile
                  FunctionDef "id"
                    ParamList
                      Param "x"
                        Path "T"
                    ReturnType
                      Path "T"
                    Block
                      TailExpr
                        Path "x"
            "#]],
        );
    }

    #[test]
    fn function_visibility() {
        check(
            "pub fn public() {}",
            &expect![[r#"
                SourceFile
                  pub FunctionDef "public"
                    ParamList
                    Block
            "#]],
        );
    }

    #[test]
    fn struct_empty_parens() {
        check(
            "struct Empty()",
            &expect![[r#"
                SourceFile
                  StructDef "Empty"
                    FieldList
            "#]],
        );
    }

    /// Unit struct syntax: `struct Empty;`
    #[test]
    fn struct_unit() {
        check(
            "struct Empty;",
            &expect![[r#"
                SourceFile
                  StructDef "Empty"
            "#]],
        );
    }

    #[test]
    fn struct_with_fields_parens() {
        check(
            "struct Point(x: i32, y: i32)",
            &expect![[r#"
                SourceFile
                  StructDef "Point"
                    FieldList
                      FieldDef "x"
                        Path "i32"
                      FieldDef "y"
                        Path "i32"
            "#]],
        );
    }

    /// Tuple struct syntax: `struct Pair(i32, i32);`
    #[test]
    fn struct_tuple() {
        check(
            "struct Pair(i32, i32);",
            &expect![[r#"
                SourceFile
                  StructDef "Pair"
                    FieldList
                      FieldDef "0"
                        Path "i32"
                      FieldDef "1"
                        Path "i32"
            "#]],
        );
    }

    #[test]
    fn struct_generic_where() {
        check(
            "struct Box(value: T) where T",
            &expect![[r#"
                SourceFile
                  StructDef "Box"
                    FieldList
                      FieldDef "value"
                        Path "T"
            "#]],
        );
    }

    #[test]
    fn impl_block() {
        check(
            "impl Foo { fn bar() {} }",
            &expect![[r#"
                SourceFile
                  ImplBlock
                    SelfType
                      Path "Foo"
                    FunctionDef "bar"
                      ParamList
                      Block
            "#]],
        );
    }

    #[test]
    fn impl_with_self() {
        check(
            "impl Foo { fn method(&self) {} }",
            &expect![[r#"
                SourceFile
                  ImplBlock
                    SelfType
                      Path "Foo"
                    FunctionDef "method"
                      ParamList
                        SelfParam "&self"
                      Block
            "#]],
        );
    }

    #[test]
    fn type_alias() {
        check(
            "type Int = i32;",
            &expect![[r#"
                SourceFile
                  TypeAlias "Int"
                    Path "i32"
            "#]],
        );
    }

    #[test]
    fn type_alias_with_generic_target() {
        check(
            "type OptInt = Option<i32>;",
            &expect![[r#"
                SourceFile
                  TypeAlias "OptInt"
                    Path "Option"
                      GenericArgs
                        Path "i32"
            "#]],
        );
    }

    /// Generic type alias: `type Result = Option<T> where T;`
    #[test]
    fn type_alias_generic_where() {
        check(
            "type Result = Option<T> where T;",
            &expect![[r#"
                SourceFile
                  TypeAlias "Result"
                    Path "Option"
                      GenericArgs
                        Path "T"
            "#]],
        );
    }
}

mod statements {
    use super::*;

    #[test]
    fn let_simple() {
        check(
            "fn main() { let x = 1; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt
                        IdentPat "x"
                        Initializer
                          Literal 1
            "#]],
        );
    }

    #[test]
    fn let_with_type() {
        check(
            "fn main() { let x: i32 = 1; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt
                        IdentPat "x"
                        TypeAnnotation
                          Path "i32"
                        Initializer
                          Literal 1
            "#]],
        );
    }

    #[test]
    fn let_mut() {
        check(
            "fn main() { let mut x = 1; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt mut
                        IdentPat "x"
                        Initializer
                          Literal 1
            "#]],
        );
    }

    #[test]
    fn let_pattern() {
        check(
            "fn main() { let (a, b) = pair; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt
                        TuplePat
                          IdentPat "a"
                          IdentPat "b"
                        Initializer
                          Path "pair"
            "#]],
        );
    }

    #[test]
    fn expr_stmt() {
        check(
            "fn main() { foo(); }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      ExprStmt
                        CallExpr
                          Callee
                            Path "foo"
                          ArgList
            "#]],
        );
    }

    #[test]
    fn expr_stmt_no_semi() {
        check(
            "fn main() { x }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        Path "x"
            "#]],
        );
    }
}

mod expressions {
    use super::*;

    #[test]
    fn literal_int() {
        check(
            "fn main() { 42 }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        Literal 42
            "#]],
        );
    }

    #[test]
    fn literal_float() {
        check(
            "fn main() { 3.14 }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        Literal 3.14
            "#]],
        );
    }

    #[test]
    fn literal_string() {
        check(
            r#"fn main() { "hello" }"#,
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        Literal "hello"
            "#]],
        );
    }

    #[test]
    fn literal_char() {
        check(
            "fn main() { 'a' }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        Literal 'a'
            "#]],
        );
    }

    #[test]
    fn literal_bool() {
        check(
            "fn main() { true }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        Literal true
            "#]],
        );
    }

    #[test]
    fn path_simple() {
        check(
            "fn main() { foo }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        Path "foo"
            "#]],
        );
    }

    #[test]
    fn path_qualified() {
        check(
            "fn main() { foo::bar::baz }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        Path "foo::bar::baz"
            "#]],
        );
    }

    #[test]
    fn paren_expr() {
        check(
            "fn main() { (1 + 2) }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        ParenExpr
                          BinaryExpr "+"
                            Literal 1
                            Literal 2
            "#]],
        );
    }

    #[test]
    fn tuple_expr() {
        check(
            "fn main() { (1, 2, 3) }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        TupleExpr
                          Literal 1
                          Literal 2
                          Literal 3
            "#]],
        );
    }

    #[test]
    fn array_expr() {
        check(
            "fn main() { [1, 2, 3] }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        ArrayExpr
                          Literal 1
                          Literal 2
                          Literal 3
            "#]],
        );
    }

    #[test]
    fn array_repeat_expr() {
        check(
            "fn main() { [0; 10] }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        ArrayRepeatExpr
                          Literal 0
                          Literal 10
            "#]],
        );
    }

    #[test]
    fn struct_expr() {
        check(
            "fn main() { Point { x: 1, y: 2 } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        StructExpr "Point"
                          Field "x"
                            Literal 1
                          Field "y"
                            Literal 2
            "#]],
        );
    }

    #[test]
    fn struct_expr_update() {
        check(
            "fn main() { Point { x: 1, ..other } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        StructExpr "Point"
                          Field "x"
                            Literal 1
                          UpdateBase
                            Path "other"
            "#]],
        );
    }

    #[test]
    fn binary_arithmetic() {
        check(
            "fn main() { 1 + 2 * 3 }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        BinaryExpr "+"
                          Literal 1
                          BinaryExpr "*"
                            Literal 2
                            Literal 3
            "#]],
        );
    }

    #[test]
    fn binary_comparison() {
        check(
            "fn main() { a < b }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        BinaryExpr "<"
                          Path "a"
                          Path "b"
            "#]],
        );
    }

    #[test]
    fn binary_logical() {
        check(
            "fn main() { a && b || c }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        BinaryExpr "||"
                          BinaryExpr "&&"
                            Path "a"
                            Path "b"
                          Path "c"
            "#]],
        );
    }

    #[test]
    fn prefix_neg() {
        check(
            "fn main() { -x }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        PrefixExpr "-"
                          Path "x"
            "#]],
        );
    }

    #[test]
    fn prefix_not() {
        check(
            "fn main() { !x }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        PrefixExpr "!"
                          Path "x"
            "#]],
        );
    }

    #[test]
    fn prefix_deref() {
        check(
            "fn main() { *x }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        PrefixExpr "*"
                          Path "x"
            "#]],
        );
    }

    #[test]
    fn ref_expr() {
        check(
            "fn main() { &x }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        RefExpr "&"
                          Path "x"
            "#]],
        );
    }

    #[test]
    fn ref_mut_expr() {
        check(
            "fn main() { &mut x }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        RefExpr "&mut "
                          Path "x"
            "#]],
        );
    }

    #[test]
    fn field_access() {
        check(
            "fn main() { point.x }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        FieldExpr ".x"
                          Path "point"
            "#]],
        );
    }

    #[test]
    fn tuple_field_access() {
        check(
            "fn main() { tuple.0 }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        FieldExpr ".0"
                          Path "tuple"
            "#]],
        );
    }

    #[test]
    fn method_call() {
        check(
            "fn main() { x.foo() }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        MethodCallExpr ".foo()"
                          Receiver
                            Path "x"
                          ArgList
            "#]],
        );
    }

    #[test]
    fn method_call_with_args() {
        check(
            "fn main() { x.foo(1, 2) }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        MethodCallExpr ".foo()"
                          Receiver
                            Path "x"
                          ArgList
                            Literal 1
                            Literal 2
            "#]],
        );
    }

    #[test]
    fn call_expr() {
        check(
            "fn main() { foo(1, 2) }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        CallExpr
                          Callee
                            Path "foo"
                          ArgList
                            Literal 1
                            Literal 2
            "#]],
        );
    }

    #[test]
    fn index_expr() {
        check(
            "fn main() { arr[0] }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        IndexExpr
                          Base
                            Path "arr"
                          Index
                            Literal 0
            "#]],
        );
    }

    #[test]
    fn if_expr() {
        check(
            "fn main() { if x { 1 } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        IfExpr
                          Condition
                            Path "x"
                          Then
                            Block
                              TailExpr
                                Literal 1
            "#]],
        );
    }

    #[test]
    fn if_else() {
        check(
            "fn main() { if x { 1 } else { 2 } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        IfExpr
                          Condition
                            Path "x"
                          Then
                            Block
                              TailExpr
                                Literal 1
                          Else
                            Block
                              TailExpr
                                Literal 2
            "#]],
        );
    }

    #[test]
    fn if_else_if() {
        check(
            "fn main() { if x { 1 } else if y { 2 } else { 3 } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        IfExpr
                          Condition
                            Path "x"
                          Then
                            Block
                              TailExpr
                                Literal 1
                          Else
                            IfExpr
                              Condition
                                Path "y"
                              Then
                                Block
                                  TailExpr
                                    Literal 2
                              Else
                                Block
                                  TailExpr
                                    Literal 3
            "#]],
        );
    }

    #[test]
    fn while_expr() {
        check(
            "fn main() { while x { y; } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        WhileExpr
                          Condition
                            Path "x"
                          Block
                            ExprStmt
                              Path "y"
            "#]],
        );
    }

    #[test]
    fn for_expr() {
        check(
            "fn main() { for i in items { i; } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        ForExpr
                          Pattern
                            IdentPat "i"
                          Iterable
                            Path "items"
                          Block
                            ExprStmt
                              Path "i"
            "#]],
        );
    }

    #[test]
    fn loop_expr() {
        check(
            "fn main() { loop { break; } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        LoopExpr
                          Block
                            ExprStmt
                              BreakExpr
            "#]],
        );
    }

    #[test]
    fn break_expr() {
        check(
            "fn main() { loop { break; } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        LoopExpr
                          Block
                            ExprStmt
                              BreakExpr
            "#]],
        );
    }

    #[test]
    fn break_with_value() {
        check(
            "fn main() { loop { break 42; } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        LoopExpr
                          Block
                            ExprStmt
                              BreakExpr
                                Literal 42
            "#]],
        );
    }

    #[test]
    fn continue_expr() {
        check(
            "fn main() { loop { continue; } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        LoopExpr
                          Block
                            ExprStmt
                              ContinueExpr
            "#]],
        );
    }

    #[test]
    fn return_expr() {
        check(
            "fn main() { return 42; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      ExprStmt
                        ReturnExpr
                          Literal 42
            "#]],
        );
    }

    #[test]
    fn return_void() {
        check(
            "fn main() { return; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      ExprStmt
                        ReturnExpr
            "#]],
        );
    }

    #[test]
    fn block_expr() {
        check(
            "fn main() { { 1 } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        BlockExpr
                          Block
                            TailExpr
                              Literal 1
            "#]],
        );
    }

    #[test]
    fn block_with_tail() {
        check(
            "fn main() { { let x = 1; x } }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        BlockExpr
                          Block
                            LetStmt
                              IdentPat "x"
                              Initializer
                                Literal 1
                            TailExpr
                              Path "x"
            "#]],
        );
    }

    #[test]
    fn cast_expr() {
        check(
            "fn main() { x as i64 }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        CastExpr
                          Path "x"
                          TargetType
                            Path "i64"
            "#]],
        );
    }

    /// RangeFull expression: `..`
    #[test]
    fn range_full() {
        check(
            "fn main() { .. }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        RangeFullExpr
            "#]],
        );
    }

    #[test]
    fn range_from() {
        check(
            "fn main() { 1.. }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        RangeFromExpr
                          Start
                            Literal 1
            "#]],
        );
    }

    /// RangeTo expression: `..10`
    #[test]
    fn range_to() {
        check(
            "fn main() { ..10 }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        RangeToExpr
                          End
                            Literal 10
            "#]],
        );
    }

    #[test]
    fn range_expr() {
        check(
            "fn main() { 1..10 }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        RangeExpr
                          Start
                            Literal 1
                          End
                            Literal 10
            "#]],
        );
    }
}

mod types {
    use super::*;

    #[test]
    fn type_path_colon() {
        check(
            "fn foo(): i32 {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "foo"
                    ParamList
                    ReturnType
                      Path "i32"
                    Block
            "#]],
        );
    }

    #[test]
    fn type_path_generic_colon() {
        check(
            "fn foo(): Vec<i32> {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "foo"
                    ParamList
                    ReturnType
                      Path "Vec"
                        GenericArgs
                          Path "i32"
                    Block
            "#]],
        );
    }

    #[test]
    fn type_ref() {
        check(
            "fn foo(x: &i32) {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "foo"
                    ParamList
                      Param "x"
                        RefType "&"
                          Path "i32"
                    Block
            "#]],
        );
    }

    #[test]
    fn type_ref_mut() {
        check(
            "fn foo(x: &mut i32) {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "foo"
                    ParamList
                      Param "x"
                        RefType "&mut "
                          Path "i32"
                    Block
            "#]],
        );
    }

    #[test]
    fn type_array() {
        check(
            "fn foo(x: [i32; 5]) {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "foo"
                    ParamList
                      Param "x"
                        ArrayType
                          ElementType
                            Path "i32"
                          Length
                            Literal 5
                    Block
            "#]],
        );
    }

    #[test]
    fn type_slice() {
        check(
            "fn foo(x: &[i32]) {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "foo"
                    ParamList
                      Param "x"
                        RefType "&"
                          SliceType
                            Path "i32"
                    Block
            "#]],
        );
    }

    #[test]
    fn type_tuple() {
        check(
            "fn foo(x: (i32, bool)) {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "foo"
                    ParamList
                      Param "x"
                        TupleType
                          Path "i32"
                          Path "bool"
                    Block
            "#]],
        );
    }

    #[test]
    fn type_fn_ptr() {
        check(
            "fn foo(f: fn(i32) -> bool) {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "foo"
                    ParamList
                      Param "f"
                        FnPtrType
                          Params
                            Path "i32"
                          ReturnType
                            Path "bool"
                    Block
            "#]],
        );
    }

    #[test]
    fn type_never_colon() {
        check(
            "fn foo(): ! {}",
            &expect![[r#"
                SourceFile
                  FunctionDef "foo"
                    ParamList
                    ReturnType
                      NeverType "!"
                    Block
            "#]],
        );
    }
}

mod patterns {
    use super::*;

    #[test]
    fn pat_ident() {
        check(
            "fn main() { let x = 1; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt
                        IdentPat "x"
                        Initializer
                          Literal 1
            "#]],
        );
    }

    #[test]
    fn pat_ident_mut() {
        check(
            "fn main() { let mut x = 1; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt mut
                        IdentPat "x"
                        Initializer
                          Literal 1
            "#]],
        );
    }

    #[test]
    fn pat_wildcard() {
        check(
            "fn main() { let _ = 1; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt
                        WildcardPat "_"
                        Initializer
                          Literal 1
            "#]],
        );
    }

    #[test]
    fn pat_tuple() {
        check(
            "fn main() { let (a, b) = pair; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt
                        TuplePat
                          IdentPat "a"
                          IdentPat "b"
                        Initializer
                          Path "pair"
            "#]],
        );
    }

    #[test]
    fn pat_struct() {
        check(
            "fn main() { let Point { x, y } = p; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt
                        StructPat "Point"
                          Field "x" (shorthand)
                          Field "y" (shorthand)
                        Initializer
                          Path "p"
            "#]],
        );
    }

    #[test]
    fn pat_struct_with_binding() {
        check(
            "fn main() { let Point { x: a, y: b } = p; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt
                        StructPat "Point"
                          Field "x"
                            IdentPat "a"
                          Field "y"
                            IdentPat "b"
                        Initializer
                          Path "p"
            "#]],
        );
    }

    #[test]
    fn pat_ref() {
        check(
            "fn main() { let &x = r; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt
                        RefPat "&"
                          IdentPat "x"
                        Initializer
                          Path "r"
            "#]],
        );
    }

    #[test]
    fn pat_ref_mut() {
        check(
            "fn main() { let &mut x = r; }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt
                        RefPat "&mut "
                          IdentPat "x"
                        Initializer
                          Path "r"
            "#]],
        );
    }
}

mod integration {
    use super::*;

    #[test]
    fn complete_program() {
        check(
            r#"
struct Point(x: i32, y: i32)

impl Point {
fn new(x: i32, y: i32): Point {
    Point { x: x, y: y }
}

fn distance(&self): i32 {
    self.x + self.y
}
}

fn main() {
let p = Point::new(3, 4);
p.distance();
}
"#,
            &expect![[r#"
                SourceFile
                  StructDef "Point"
                    FieldList
                      FieldDef "x"
                        Path "i32"
                      FieldDef "y"
                        Path "i32"
                  ImplBlock
                    SelfType
                      Path "Point"
                    FunctionDef "new"
                      ParamList
                        Param "x"
                          Path "i32"
                        Param "y"
                          Path "i32"
                      ReturnType
                        Path "Point"
                      Block
                        TailExpr
                          StructExpr "Point"
                            Field "x"
                              Path "x"
                            Field "y"
                              Path "y"
                    FunctionDef "distance"
                      ParamList
                        SelfParam "&self"
                      ReturnType
                        Path "i32"
                      Block
                        TailExpr
                          BinaryExpr "+"
                            FieldExpr ".x"
                              Path "self"
                            FieldExpr ".y"
                              Path "self"
                  FunctionDef "main"
                    ParamList
                    Block
                      LetStmt
                        IdentPat "p"
                        Initializer
                          CallExpr
                            Callee
                              Path "Point::new"
                            ArgList
                              Literal 3
                              Literal 4
                      ExprStmt
                        MethodCallExpr ".distance()"
                          Receiver
                            Path "p"
                          ArgList
            "#]],
        );
    }

    #[test]
    fn nested_expressions() {
        check(
            "fn main() { ((1 + 2) * (3 - 4)) / 5 }",
            &expect![[r#"
                SourceFile
                  FunctionDef "main"
                    ParamList
                    Block
                      TailExpr
                        BinaryExpr "/"
                          ParenExpr
                            BinaryExpr "*"
                              ParenExpr
                                BinaryExpr "+"
                                  Literal 1
                                  Literal 2
                              ParenExpr
                                BinaryExpr "-"
                                  Literal 3
                                  Literal 4
                          Literal 5
            "#]],
        );
    }

    #[test]
    fn complex_function() {
        check(
            r#"
fn process(items: &[T], filter: fn(T) -> bool): i32 where T {
let mut count = 0;
for item in items {
    if filter(item) {
        count = count + 1;
    }
}
count
}
"#,
            &expect![[r#"
                SourceFile
                  FunctionDef "process"
                    ParamList
                      Param "items"
                        RefType "&"
                          SliceType
                            Path "T"
                      Param "filter"
                        FnPtrType
                          Params
                            Path "T"
                          ReturnType
                            Path "bool"
                    ReturnType
                      Path "i32"
                    Block
                      LetStmt mut
                        IdentPat "count"
                        Initializer
                          Literal 0
                      ExprStmt (no semi)
                        ForExpr
                          Pattern
                            IdentPat "item"
                          Iterable
                            Path "items"
                          Block
                            TailExpr
                              IfExpr
                                Condition
                                  CallExpr
                                    Callee
                                      Path "filter"
                                    ArgList
                                      Path "item"
                                Then
                                  Block
                                    ExprStmt
                                      BinaryExpr "="
                                        Path "count"
                                        BinaryExpr "+"
                                          Path "count"
                                          Literal 1
                      TailExpr
                        Path "count"
            "#]],
        );
    }
}
