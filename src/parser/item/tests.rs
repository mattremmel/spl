use crate::parser::tests::check_item;
use expect_test::expect;

#[test]
fn function_minimal() {
    check_item(
        "fn foo() {}",
        &expect![[r#"
            FunctionDef@0..11
              FN_KW@0..2 "fn"
              Name@2..6
                WHITESPACE@2..3 " "
                IDENT@3..6 "foo"
              ParamList@6..8
                L_PAREN@6..7 "("
                R_PAREN@7..8 ")"
              Block@8..11
                WHITESPACE@8..9 " "
                L_BRACE@9..10 "{"
                R_BRACE@10..11 "}"
        "#]],
    );
}

#[test]
fn function_with_return_type_colon() {
    check_item(
        "fn foo(): i32 {}",
        &expect![[r#"
            FunctionDef@0..16
              FN_KW@0..2 "fn"
              Name@2..6
                WHITESPACE@2..3 " "
                IDENT@3..6 "foo"
              ParamList@6..8
                L_PAREN@6..7 "("
                R_PAREN@7..8 ")"
              COLON@8..9 ":"
              PathType@9..13
                Path@9..13
                  PathSegment@9..13
                    NameRef@9..13
                      WHITESPACE@9..10 " "
                      IDENT@10..13 "i32"
              Block@13..16
                WHITESPACE@13..14 " "
                L_BRACE@14..15 "{"
                R_BRACE@15..16 "}"
        "#]],
    );
}

#[test]
fn function_with_params_colon() {
    check_item(
        "fn add(x: i32, y: i32): i32 {}",
        &expect![[r#"
            FunctionDef@0..30
              FN_KW@0..2 "fn"
              Name@2..6
                WHITESPACE@2..3 " "
                IDENT@3..6 "add"
              ParamList@6..22
                L_PAREN@6..7 "("
                Param@7..13
                  Name@7..8
                    IDENT@7..8 "x"
                  COLON@8..9 ":"
                  PathType@9..13
                    Path@9..13
                      PathSegment@9..13
                        NameRef@9..13
                          WHITESPACE@9..10 " "
                          IDENT@10..13 "i32"
                COMMA@13..14 ","
                Param@14..21
                  Name@14..16
                    WHITESPACE@14..15 " "
                    IDENT@15..16 "y"
                  COLON@16..17 ":"
                  PathType@17..21
                    Path@17..21
                      PathSegment@17..21
                        NameRef@17..21
                          WHITESPACE@17..18 " "
                          IDENT@18..21 "i32"
                R_PAREN@21..22 ")"
              COLON@22..23 ":"
              PathType@23..27
                Path@23..27
                  PathSegment@23..27
                    NameRef@23..27
                      WHITESPACE@23..24 " "
                      IDENT@24..27 "i32"
              Block@27..30
                WHITESPACE@27..28 " "
                L_BRACE@28..29 "{"
                R_BRACE@29..30 "}"
        "#]],
    );
}

#[test]
fn function_with_body_colon() {
    check_item(
        "fn answer(): i32 { 42 }",
        &expect![[r#"
            FunctionDef@0..23
              FN_KW@0..2 "fn"
              Name@2..9
                WHITESPACE@2..3 " "
                IDENT@3..9 "answer"
              ParamList@9..11
                L_PAREN@9..10 "("
                R_PAREN@10..11 ")"
              COLON@11..12 ":"
              PathType@12..16
                Path@12..16
                  PathSegment@12..16
                    NameRef@12..16
                      WHITESPACE@12..13 " "
                      IDENT@13..16 "i32"
              Block@16..23
                WHITESPACE@16..17 " "
                L_BRACE@17..18 "{"
                LiteralExpr@18..21
                  WHITESPACE@18..19 " "
                  INT_LITERAL@19..21 "42"
                WHITESPACE@21..22 " "
                R_BRACE@22..23 "}"
        "#]],
    );
}

#[test]
fn function_pub() {
    check_item(
        "pub fn foo() {}",
        &expect![[r#"
            FunctionDef@0..15
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              FN_KW@4..6 "fn"
              Name@6..10
                WHITESPACE@6..7 " "
                IDENT@7..10 "foo"
              ParamList@10..12
                L_PAREN@10..11 "("
                R_PAREN@11..12 ")"
              Block@12..15
                WHITESPACE@12..13 " "
                L_BRACE@13..14 "{"
                R_BRACE@14..15 "}"
        "#]],
    );
}

#[test]
fn visibility_pub_crate() {
    check_item(
        "pub(crate) fn foo() {}",
        &expect![[r#"
            FunctionDef@0..22
              Visibility@0..10
                PUB_KW@0..3 "pub"
                L_PAREN@3..4 "("
                CRATE_KW@4..9 "crate"
                R_PAREN@9..10 ")"
              WHITESPACE@10..11 " "
              FN_KW@11..13 "fn"
              Name@13..17
                WHITESPACE@13..14 " "
                IDENT@14..17 "foo"
              ParamList@17..19
                L_PAREN@17..18 "("
                R_PAREN@18..19 ")"
              Block@19..22
                WHITESPACE@19..20 " "
                L_BRACE@20..21 "{"
                R_BRACE@21..22 "}"
        "#]],
    );
}

#[test]
fn visibility_pub_super() {
    check_item(
        "pub(super) fn foo() {}",
        &expect![[r#"
            FunctionDef@0..22
              Visibility@0..10
                PUB_KW@0..3 "pub"
                L_PAREN@3..4 "("
                SUPER_KW@4..9 "super"
                R_PAREN@9..10 ")"
              WHITESPACE@10..11 " "
              FN_KW@11..13 "fn"
              Name@13..17
                WHITESPACE@13..14 " "
                IDENT@14..17 "foo"
              ParamList@17..19
                L_PAREN@17..18 "("
                R_PAREN@18..19 ")"
              Block@19..22
                WHITESPACE@19..20 " "
                L_BRACE@20..21 "{"
                R_BRACE@21..22 "}"
        "#]],
    );
}

#[test]
fn visibility_pub_self() {
    check_item(
        "pub(self) fn foo() {}",
        &expect![[r#"
            FunctionDef@0..21
              Visibility@0..9
                PUB_KW@0..3 "pub"
                L_PAREN@3..4 "("
                SELF_VALUE_KW@4..8 "self"
                R_PAREN@8..9 ")"
              WHITESPACE@9..10 " "
              FN_KW@10..12 "fn"
              Name@12..16
                WHITESPACE@12..13 " "
                IDENT@13..16 "foo"
              ParamList@16..18
                L_PAREN@16..17 "("
                R_PAREN@17..18 ")"
              Block@18..21
                WHITESPACE@18..19 " "
                L_BRACE@19..20 "{"
                R_BRACE@20..21 "}"
        "#]],
    );
}

#[test]
fn visibility_pub_in_path() {
    check_item(
        "pub(in crate.foo) fn bar() {}",
        &expect![[r#"
            FunctionDef@0..29
              Visibility@0..17
                PUB_KW@0..3 "pub"
                L_PAREN@3..4 "("
                IN_KW@4..6 "in"
                Path@6..16
                  PathSegment@6..12
                    NameRef@6..12
                      WHITESPACE@6..7 " "
                      CRATE_KW@7..12 "crate"
                  DOT@12..13 "."
                  PathSegment@13..16
                    NameRef@13..16
                      IDENT@13..16 "foo"
                R_PAREN@16..17 ")"
              WHITESPACE@17..18 " "
              FN_KW@18..20 "fn"
              Name@20..24
                WHITESPACE@20..21 " "
                IDENT@21..24 "bar"
              ParamList@24..26
                L_PAREN@24..25 "("
                R_PAREN@25..26 ")"
              Block@26..29
                WHITESPACE@26..27 " "
                L_BRACE@27..28 "{"
                R_BRACE@28..29 "}"
        "#]],
    );
}

#[test]
fn struct_pub_crate_paren() {
    check_item(
        "pub(crate) struct Foo()",
        &expect![[r#"
            StructDef@0..23
              Visibility@0..10
                PUB_KW@0..3 "pub"
                L_PAREN@3..4 "("
                CRATE_KW@4..9 "crate"
                R_PAREN@9..10 ")"
              WHITESPACE@10..11 " "
              STRUCT_KW@11..17 "struct"
              Name@17..21
                WHITESPACE@17..18 " "
                IDENT@18..21 "Foo"
              FieldList@21..23
                L_PAREN@21..22 "("
                R_PAREN@22..23 ")"
        "#]],
    );
}

#[test]
fn field_pub_crate_paren() {
    check_item(
        "struct Foo(pub(crate) x: i32)",
        &expect![[r#"
            StructDef@0..29
              STRUCT_KW@0..6 "struct"
              Name@6..10
                WHITESPACE@6..7 " "
                IDENT@7..10 "Foo"
              FieldList@10..29
                L_PAREN@10..11 "("
                FieldDef@11..28
                  Visibility@11..21
                    PUB_KW@11..14 "pub"
                    L_PAREN@14..15 "("
                    CRATE_KW@15..20 "crate"
                    R_PAREN@20..21 ")"
                  Name@21..23
                    WHITESPACE@21..22 " "
                    IDENT@22..23 "x"
                  COLON@23..24 ":"
                  PathType@24..28
                    Path@24..28
                      PathSegment@24..28
                        NameRef@24..28
                          WHITESPACE@24..25 " "
                          IDENT@25..28 "i32"
                R_PAREN@28..29 ")"
        "#]],
    );
}

#[test]
fn function_with_self() {
    check_item(
        "fn method(&self) {}",
        &expect![[r#"
            FunctionDef@0..19
              FN_KW@0..2 "fn"
              Name@2..9
                WHITESPACE@2..3 " "
                IDENT@3..9 "method"
              ParamList@9..16
                L_PAREN@9..10 "("
                SelfParam@10..15
                  AMP@10..11 "&"
                  SELF_VALUE_KW@11..15 "self"
                R_PAREN@15..16 ")"
              Block@16..19
                WHITESPACE@16..17 " "
                L_BRACE@17..18 "{"
                R_BRACE@18..19 "}"
        "#]],
    );
}

#[test]
fn function_with_mut_self() {
    check_item(
        "fn method(&mut self) {}",
        &expect![[r#"
            FunctionDef@0..23
              FN_KW@0..2 "fn"
              Name@2..9
                WHITESPACE@2..3 " "
                IDENT@3..9 "method"
              ParamList@9..20
                L_PAREN@9..10 "("
                SelfParam@10..19
                  AMP@10..11 "&"
                  MUT_KW@11..14 "mut"
                  WHITESPACE@14..15 " "
                  SELF_VALUE_KW@15..19 "self"
                R_PAREN@19..20 ")"
              Block@20..23
                WHITESPACE@20..21 " "
                L_BRACE@21..22 "{"
                R_BRACE@22..23 "}"
        "#]],
    );
}

#[test]
fn function_with_self_and_params() {
    check_item(
        "fn method(&self, x: i32) {}",
        &expect![[r#"
            FunctionDef@0..27
              FN_KW@0..2 "fn"
              Name@2..9
                WHITESPACE@2..3 " "
                IDENT@3..9 "method"
              ParamList@9..24
                L_PAREN@9..10 "("
                SelfParam@10..15
                  AMP@10..11 "&"
                  SELF_VALUE_KW@11..15 "self"
                COMMA@15..16 ","
                Param@16..23
                  Name@16..18
                    WHITESPACE@16..17 " "
                    IDENT@17..18 "x"
                  COLON@18..19 ":"
                  PathType@19..23
                    Path@19..23
                      PathSegment@19..23
                        NameRef@19..23
                          WHITESPACE@19..20 " "
                          IDENT@20..23 "i32"
                R_PAREN@23..24 ")"
              Block@24..27
                WHITESPACE@24..25 " "
                L_BRACE@25..26 "{"
                R_BRACE@26..27 "}"
        "#]],
    );
}

#[test]
fn function_with_generics_where() {
    check_item(
        "fn identity(x: T): T where T {}",
        &expect![[r#"
            FunctionDef@0..31
              FN_KW@0..2 "fn"
              Name@2..11
                WHITESPACE@2..3 " "
                IDENT@3..11 "identity"
              ParamList@11..17
                L_PAREN@11..12 "("
                Param@12..16
                  Name@12..13
                    IDENT@12..13 "x"
                  COLON@13..14 ":"
                  PathType@14..16
                    Path@14..16
                      PathSegment@14..16
                        NameRef@14..16
                          WHITESPACE@14..15 " "
                          IDENT@15..16 "T"
                R_PAREN@16..17 ")"
              COLON@17..18 ":"
              PathType@18..20
                Path@18..20
                  PathSegment@18..20
                    NameRef@18..20
                      WHITESPACE@18..19 " "
                      IDENT@19..20 "T"
              WhereClause@20..28
                WHITESPACE@20..21 " "
                WHERE_KW@21..26 "where"
                GenericParam@26..28
                  Name@26..28
                    WHITESPACE@26..27 " "
                    IDENT@27..28 "T"
              Block@28..31
                WHITESPACE@28..29 " "
                L_BRACE@29..30 "{"
                R_BRACE@30..31 "}"
        "#]],
    );
}

#[test]
fn function_with_multiple_generics_where() {
    check_item(
        "fn pair(a: T, b: U) where T, U {}",
        &expect![[r#"
            FunctionDef@0..33
              FN_KW@0..2 "fn"
              Name@2..7
                WHITESPACE@2..3 " "
                IDENT@3..7 "pair"
              ParamList@7..19
                L_PAREN@7..8 "("
                Param@8..12
                  Name@8..9
                    IDENT@8..9 "a"
                  COLON@9..10 ":"
                  PathType@10..12
                    Path@10..12
                      PathSegment@10..12
                        NameRef@10..12
                          WHITESPACE@10..11 " "
                          IDENT@11..12 "T"
                COMMA@12..13 ","
                Param@13..18
                  Name@13..15
                    WHITESPACE@13..14 " "
                    IDENT@14..15 "b"
                  COLON@15..16 ":"
                  PathType@16..18
                    Path@16..18
                      PathSegment@16..18
                        NameRef@16..18
                          WHITESPACE@16..17 " "
                          IDENT@17..18 "U"
                R_PAREN@18..19 ")"
              WhereClause@19..30
                WHITESPACE@19..20 " "
                WHERE_KW@20..25 "where"
                GenericParam@25..27
                  Name@25..27
                    WHITESPACE@25..26 " "
                    IDENT@26..27 "T"
                COMMA@27..28 ","
                GenericParam@28..30
                  Name@28..30
                    WHITESPACE@28..29 " "
                    IDENT@29..30 "U"
              Block@30..33
                WHITESPACE@30..31 " "
                L_BRACE@31..32 "{"
                R_BRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn function_owned_self() {
    check_item(
        "fn consume(self) {}",
        &expect![[r#"
            FunctionDef@0..19
              FN_KW@0..2 "fn"
              Name@2..10
                WHITESPACE@2..3 " "
                IDENT@3..10 "consume"
              ParamList@10..16
                L_PAREN@10..11 "("
                SelfParam@11..15
                  SELF_VALUE_KW@11..15 "self"
                R_PAREN@15..16 ")"
              Block@16..19
                WHITESPACE@16..17 " "
                L_BRACE@17..18 "{"
                R_BRACE@18..19 "}"
        "#]],
    );
}

// === Struct tests (paren syntax) ===

#[test]
fn struct_empty_paren() {
    check_item(
        "struct Point()",
        &expect![[r#"
            StructDef@0..14
              STRUCT_KW@0..6 "struct"
              Name@6..12
                WHITESPACE@6..7 " "
                IDENT@7..12 "Point"
              FieldList@12..14
                L_PAREN@12..13 "("
                R_PAREN@13..14 ")"
        "#]],
    );
}

#[test]
fn struct_with_fields_paren() {
    check_item(
        "struct Point(x: i32, y: i32)",
        &expect![[r#"
            StructDef@0..28
              STRUCT_KW@0..6 "struct"
              Name@6..12
                WHITESPACE@6..7 " "
                IDENT@7..12 "Point"
              FieldList@12..28
                L_PAREN@12..13 "("
                FieldDef@13..19
                  Name@13..14
                    IDENT@13..14 "x"
                  COLON@14..15 ":"
                  PathType@15..19
                    Path@15..19
                      PathSegment@15..19
                        NameRef@15..19
                          WHITESPACE@15..16 " "
                          IDENT@16..19 "i32"
                COMMA@19..20 ","
                FieldDef@20..27
                  Name@20..22
                    WHITESPACE@20..21 " "
                    IDENT@21..22 "y"
                  COLON@22..23 ":"
                  PathType@23..27
                    Path@23..27
                      PathSegment@23..27
                        NameRef@23..27
                          WHITESPACE@23..24 " "
                          IDENT@24..27 "i32"
                R_PAREN@27..28 ")"
        "#]],
    );
}

#[test]
fn struct_pub_paren() {
    check_item(
        "pub struct Foo()",
        &expect![[r#"
            StructDef@0..16
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              STRUCT_KW@4..10 "struct"
              Name@10..14
                WHITESPACE@10..11 " "
                IDENT@11..14 "Foo"
              FieldList@14..16
                L_PAREN@14..15 "("
                R_PAREN@15..16 ")"
        "#]],
    );
}

#[test]
fn struct_with_pub_field_paren() {
    check_item(
        "struct Foo(pub x: i32)",
        &expect![[r#"
            StructDef@0..22
              STRUCT_KW@0..6 "struct"
              Name@6..10
                WHITESPACE@6..7 " "
                IDENT@7..10 "Foo"
              FieldList@10..22
                L_PAREN@10..11 "("
                FieldDef@11..21
                  Visibility@11..14
                    PUB_KW@11..14 "pub"
                  Name@14..16
                    WHITESPACE@14..15 " "
                    IDENT@15..16 "x"
                  COLON@16..17 ":"
                  PathType@17..21
                    Path@17..21
                      PathSegment@17..21
                        NameRef@17..21
                          WHITESPACE@17..18 " "
                          IDENT@18..21 "i32"
                R_PAREN@21..22 ")"
        "#]],
    );
}

#[test]
fn struct_with_generics_where() {
    check_item(
        "struct Pair(first: T, second: U) where T, U",
        &expect![[r#"
            StructDef@0..43
              STRUCT_KW@0..6 "struct"
              Name@6..11
                WHITESPACE@6..7 " "
                IDENT@7..11 "Pair"
              FieldList@11..32
                L_PAREN@11..12 "("
                FieldDef@12..20
                  Name@12..17
                    IDENT@12..17 "first"
                  COLON@17..18 ":"
                  PathType@18..20
                    Path@18..20
                      PathSegment@18..20
                        NameRef@18..20
                          WHITESPACE@18..19 " "
                          IDENT@19..20 "T"
                COMMA@20..21 ","
                FieldDef@21..31
                  Name@21..28
                    WHITESPACE@21..22 " "
                    IDENT@22..28 "second"
                  COLON@28..29 ":"
                  PathType@29..31
                    Path@29..31
                      PathSegment@29..31
                        NameRef@29..31
                          WHITESPACE@29..30 " "
                          IDENT@30..31 "U"
                R_PAREN@31..32 ")"
              WhereClause@32..43
                WHITESPACE@32..33 " "
                WHERE_KW@33..38 "where"
                GenericParam@38..40
                  Name@38..40
                    WHITESPACE@38..39 " "
                    IDENT@39..40 "T"
                COMMA@40..41 ","
                GenericParam@41..43
                  Name@41..43
                    WHITESPACE@41..42 " "
                    IDENT@42..43 "U"
        "#]],
    );
}

// === Type alias tests ===

#[test]
fn type_alias_simple() {
    check_item(
        "type Int = i32;",
        &expect![[r#"
            TypeAlias@0..15
              TYPE_KW@0..4 "type"
              Name@4..8
                WHITESPACE@4..5 " "
                IDENT@5..8 "Int"
              WHITESPACE@8..9 " "
              EQ@9..10 "="
              PathType@10..14
                Path@10..14
                  PathSegment@10..14
                    NameRef@10..14
                      WHITESPACE@10..11 " "
                      IDENT@11..14 "i32"
              SEMI@14..15 ";"
        "#]],
    );
}

#[test]
fn type_alias_pub_arrow() {
    // Note: fn pointer types still use -> syntax
    check_item(
        "pub type Callback = fn(i32) -> bool;",
        &expect![[r#"
            TypeAlias@0..36
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              TYPE_KW@4..8 "type"
              Name@8..17
                WHITESPACE@8..9 " "
                IDENT@9..17 "Callback"
              WHITESPACE@17..18 " "
              EQ@18..19 "="
              FnPtrType@19..35
                WHITESPACE@19..20 " "
                FN_KW@20..22 "fn"
                L_PAREN@22..23 "("
                PathType@23..26
                  Path@23..26
                    PathSegment@23..26
                      NameRef@23..26
                        IDENT@23..26 "i32"
                R_PAREN@26..27 ")"
                WHITESPACE@27..28 " "
                ARROW@28..30 "->"
                PathType@30..35
                  Path@30..35
                    PathSegment@30..35
                      NameRef@30..35
                        WHITESPACE@30..31 " "
                        IDENT@31..35 "bool"
              SEMI@35..36 ";"
        "#]],
    );
}

// === Impl block tests ===

#[test]
fn impl_empty() {
    check_item(
        "impl Point {}",
        &expect![[r#"
            ImplBlock@0..13
              IMPL_KW@0..4 "impl"
              PathType@4..10
                Path@4..10
                  PathSegment@4..10
                    NameRef@4..10
                      WHITESPACE@4..5 " "
                      IDENT@5..10 "Point"
              WHITESPACE@10..11 " "
              L_BRACE@11..12 "{"
              R_BRACE@12..13 "}"
        "#]],
    );
}

#[test]
fn impl_with_method_colon() {
    check_item(
        "impl Point { fn new(): Point {} }",
        &expect![[r#"
            ImplBlock@0..33
              IMPL_KW@0..4 "impl"
              PathType@4..10
                Path@4..10
                  PathSegment@4..10
                    NameRef@4..10
                      WHITESPACE@4..5 " "
                      IDENT@5..10 "Point"
              WHITESPACE@10..11 " "
              L_BRACE@11..12 "{"
              FunctionDef@12..31
                WHITESPACE@12..13 " "
                FN_KW@13..15 "fn"
                Name@15..19
                  WHITESPACE@15..16 " "
                  IDENT@16..19 "new"
                ParamList@19..21
                  L_PAREN@19..20 "("
                  R_PAREN@20..21 ")"
                COLON@21..22 ":"
                PathType@22..28
                  Path@22..28
                    PathSegment@22..28
                      NameRef@22..28
                        WHITESPACE@22..23 " "
                        IDENT@23..28 "Point"
                Block@28..31
                  WHITESPACE@28..29 " "
                  L_BRACE@29..30 "{"
                  R_BRACE@30..31 "}"
              WHITESPACE@31..32 " "
              R_BRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn impl_with_generics_where() {
    check_item(
        "impl Vec(T) where T {}",
        &expect![[r#"
            ImplBlock@0..22
              IMPL_KW@0..4 "impl"
              PathType@4..11
                Path@4..11
                  PathSegment@4..11
                    NameRef@4..8
                      WHITESPACE@4..5 " "
                      IDENT@5..8 "Vec"
                    GenericArgs@8..11
                      L_PAREN@8..9 "("
                      PathType@9..10
                        Path@9..10
                          PathSegment@9..10
                            NameRef@9..10
                              IDENT@9..10 "T"
                      R_PAREN@10..11 ")"
              WhereClause@11..19
                WHITESPACE@11..12 " "
                WHERE_KW@12..17 "where"
                GenericParam@17..19
                  Name@17..19
                    WHITESPACE@17..18 " "
                    IDENT@18..19 "T"
              WHITESPACE@19..20 " "
              L_BRACE@20..21 "{"
              R_BRACE@21..22 "}"
        "#]],
    );
}

// === Source file tests ===

#[test]
fn source_file_empty() {
    use crate::parser::tests::check_source_file;
    check_source_file(
        "",
        &expect![[r#"
            SourceFile@0..0
        "#]],
    );
}

#[test]
fn source_file_single_function() {
    use crate::parser::tests::check_source_file;
    check_source_file(
        "fn main() {}",
        &expect![[r#"
            SourceFile@0..12
              FunctionDef@0..12
                FN_KW@0..2 "fn"
                Name@2..7
                  WHITESPACE@2..3 " "
                  IDENT@3..7 "main"
                ParamList@7..9
                  L_PAREN@7..8 "("
                  R_PAREN@8..9 ")"
                Block@9..12
                  WHITESPACE@9..10 " "
                  L_BRACE@10..11 "{"
                  R_BRACE@11..12 "}"
        "#]],
    );
}

#[test]
fn source_file_multiple_items_paren() {
    use crate::parser::tests::check_source_file;
    check_source_file(
        "struct Point(x: i32)\nfn main() {}",
        &expect![[r#"
            SourceFile@0..33
              StructDef@0..20
                STRUCT_KW@0..6 "struct"
                Name@6..12
                  WHITESPACE@6..7 " "
                  IDENT@7..12 "Point"
                FieldList@12..20
                  L_PAREN@12..13 "("
                  FieldDef@13..19
                    Name@13..14
                      IDENT@13..14 "x"
                    COLON@14..15 ":"
                    PathType@15..19
                      Path@15..19
                        PathSegment@15..19
                          NameRef@15..19
                            WHITESPACE@15..16 " "
                            IDENT@16..19 "i32"
                  R_PAREN@19..20 ")"
              FunctionDef@20..33
                WHITESPACE@20..21 "\n"
                FN_KW@21..23 "fn"
                Name@23..28
                  WHITESPACE@23..24 " "
                  IDENT@24..28 "main"
                ParamList@28..30
                  L_PAREN@28..29 "("
                  R_PAREN@29..30 ")"
                Block@30..33
                  WHITESPACE@30..31 " "
                  L_BRACE@31..32 "{"
                  R_BRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn source_file_with_impl_paren() {
    use crate::parser::tests::check_source_file;
    check_source_file(
        "struct Foo()\nimpl Foo { fn bar(&self) {} }",
        &expect![[r#"
            SourceFile@0..42
              StructDef@0..12
                STRUCT_KW@0..6 "struct"
                Name@6..10
                  WHITESPACE@6..7 " "
                  IDENT@7..10 "Foo"
                FieldList@10..12
                  L_PAREN@10..11 "("
                  R_PAREN@11..12 ")"
              ImplBlock@12..42
                WHITESPACE@12..13 "\n"
                IMPL_KW@13..17 "impl"
                PathType@17..21
                  Path@17..21
                    PathSegment@17..21
                      NameRef@17..21
                        WHITESPACE@17..18 " "
                        IDENT@18..21 "Foo"
                WHITESPACE@21..22 " "
                L_BRACE@22..23 "{"
                FunctionDef@23..40
                  WHITESPACE@23..24 " "
                  FN_KW@24..26 "fn"
                  Name@26..30
                    WHITESPACE@26..27 " "
                    IDENT@27..30 "bar"
                  ParamList@30..37
                    L_PAREN@30..31 "("
                    SelfParam@31..36
                      AMP@31..32 "&"
                      SELF_VALUE_KW@32..36 "self"
                    R_PAREN@36..37 ")"
                  Block@37..40
                    WHITESPACE@37..38 " "
                    L_BRACE@38..39 "{"
                    R_BRACE@39..40 "}"
                WHITESPACE@40..41 " "
                R_BRACE@41..42 "}"
        "#]],
    );
}

// === Phase 6: Item Edge Cases ===

#[test]
fn fn_many_generics_where() {
    check_item(
        "fn foo(a: T, b: U): V where T, U, V {}",
        &expect![[r#"
            FunctionDef@0..38
              FN_KW@0..2 "fn"
              Name@2..6
                WHITESPACE@2..3 " "
                IDENT@3..6 "foo"
              ParamList@6..18
                L_PAREN@6..7 "("
                Param@7..11
                  Name@7..8
                    IDENT@7..8 "a"
                  COLON@8..9 ":"
                  PathType@9..11
                    Path@9..11
                      PathSegment@9..11
                        NameRef@9..11
                          WHITESPACE@9..10 " "
                          IDENT@10..11 "T"
                COMMA@11..12 ","
                Param@12..17
                  Name@12..14
                    WHITESPACE@12..13 " "
                    IDENT@13..14 "b"
                  COLON@14..15 ":"
                  PathType@15..17
                    Path@15..17
                      PathSegment@15..17
                        NameRef@15..17
                          WHITESPACE@15..16 " "
                          IDENT@16..17 "U"
                R_PAREN@17..18 ")"
              COLON@18..19 ":"
              PathType@19..21
                Path@19..21
                  PathSegment@19..21
                    NameRef@19..21
                      WHITESPACE@19..20 " "
                      IDENT@20..21 "V"
              WhereClause@21..35
                WHITESPACE@21..22 " "
                WHERE_KW@22..27 "where"
                GenericParam@27..29
                  Name@27..29
                    WHITESPACE@27..28 " "
                    IDENT@28..29 "T"
                COMMA@29..30 ","
                GenericParam@30..32
                  Name@30..32
                    WHITESPACE@30..31 " "
                    IDENT@31..32 "U"
                COMMA@32..33 ","
                GenericParam@33..35
                  Name@33..35
                    WHITESPACE@33..34 " "
                    IDENT@34..35 "V"
              Block@35..38
                WHITESPACE@35..36 " "
                L_BRACE@36..37 "{"
                R_BRACE@37..38 "}"
        "#]],
    );
}

#[test]
fn fn_taking_fn_arg_colon() {
    check_item(
        "fn apply(f: fn(i32) -> i32, x: i32): i32 {}",
        &expect![[r#"
            FunctionDef@0..43
              FN_KW@0..2 "fn"
              Name@2..8
                WHITESPACE@2..3 " "
                IDENT@3..8 "apply"
              ParamList@8..35
                L_PAREN@8..9 "("
                Param@9..26
                  Name@9..10
                    IDENT@9..10 "f"
                  COLON@10..11 ":"
                  FnPtrType@11..26
                    WHITESPACE@11..12 " "
                    FN_KW@12..14 "fn"
                    L_PAREN@14..15 "("
                    PathType@15..18
                      Path@15..18
                        PathSegment@15..18
                          NameRef@15..18
                            IDENT@15..18 "i32"
                    R_PAREN@18..19 ")"
                    WHITESPACE@19..20 " "
                    ARROW@20..22 "->"
                    PathType@22..26
                      Path@22..26
                        PathSegment@22..26
                          NameRef@22..26
                            WHITESPACE@22..23 " "
                            IDENT@23..26 "i32"
                COMMA@26..27 ","
                Param@27..34
                  Name@27..29
                    WHITESPACE@27..28 " "
                    IDENT@28..29 "x"
                  COLON@29..30 ":"
                  PathType@30..34
                    Path@30..34
                      PathSegment@30..34
                        NameRef@30..34
                          WHITESPACE@30..31 " "
                          IDENT@31..34 "i32"
                R_PAREN@34..35 ")"
              COLON@35..36 ":"
              PathType@36..40
                Path@36..40
                  PathSegment@36..40
                    NameRef@36..40
                      WHITESPACE@36..37 " "
                      IDENT@37..40 "i32"
              Block@40..43
                WHITESPACE@40..41 " "
                L_BRACE@41..42 "{"
                R_BRACE@42..43 "}"
        "#]],
    );
}

#[test]
fn struct_many_fields_paren() {
    check_item(
        "struct S(a: A, b: B, c: C, d: D)",
        &expect![[r#"
            StructDef@0..32
              STRUCT_KW@0..6 "struct"
              Name@6..8
                WHITESPACE@6..7 " "
                IDENT@7..8 "S"
              FieldList@8..32
                L_PAREN@8..9 "("
                FieldDef@9..13
                  Name@9..10
                    IDENT@9..10 "a"
                  COLON@10..11 ":"
                  PathType@11..13
                    Path@11..13
                      PathSegment@11..13
                        NameRef@11..13
                          WHITESPACE@11..12 " "
                          IDENT@12..13 "A"
                COMMA@13..14 ","
                FieldDef@14..19
                  Name@14..16
                    WHITESPACE@14..15 " "
                    IDENT@15..16 "b"
                  COLON@16..17 ":"
                  PathType@17..19
                    Path@17..19
                      PathSegment@17..19
                        NameRef@17..19
                          WHITESPACE@17..18 " "
                          IDENT@18..19 "B"
                COMMA@19..20 ","
                FieldDef@20..25
                  Name@20..22
                    WHITESPACE@20..21 " "
                    IDENT@21..22 "c"
                  COLON@22..23 ":"
                  PathType@23..25
                    Path@23..25
                      PathSegment@23..25
                        NameRef@23..25
                          WHITESPACE@23..24 " "
                          IDENT@24..25 "C"
                COMMA@25..26 ","
                FieldDef@26..31
                  Name@26..28
                    WHITESPACE@26..27 " "
                    IDENT@27..28 "d"
                  COLON@28..29 ":"
                  PathType@29..31
                    Path@29..31
                      PathSegment@29..31
                        NameRef@29..31
                          WHITESPACE@29..30 " "
                          IDENT@30..31 "D"
                R_PAREN@31..32 ")"
        "#]],
    );
}

#[test]
fn struct_mixed_visibility_paren() {
    check_item(
        "struct S(pub a: i32, pub(crate) b: i32, c: i32)",
        &expect![[r#"
            StructDef@0..47
              STRUCT_KW@0..6 "struct"
              Name@6..8
                WHITESPACE@6..7 " "
                IDENT@7..8 "S"
              FieldList@8..47
                L_PAREN@8..9 "("
                FieldDef@9..19
                  Visibility@9..12
                    PUB_KW@9..12 "pub"
                  Name@12..14
                    WHITESPACE@12..13 " "
                    IDENT@13..14 "a"
                  COLON@14..15 ":"
                  PathType@15..19
                    Path@15..19
                      PathSegment@15..19
                        NameRef@15..19
                          WHITESPACE@15..16 " "
                          IDENT@16..19 "i32"
                COMMA@19..20 ","
                FieldDef@20..38
                  Visibility@20..31
                    WHITESPACE@20..21 " "
                    PUB_KW@21..24 "pub"
                    L_PAREN@24..25 "("
                    CRATE_KW@25..30 "crate"
                    R_PAREN@30..31 ")"
                  Name@31..33
                    WHITESPACE@31..32 " "
                    IDENT@32..33 "b"
                  COLON@33..34 ":"
                  PathType@34..38
                    Path@34..38
                      PathSegment@34..38
                        NameRef@34..38
                          WHITESPACE@34..35 " "
                          IDENT@35..38 "i32"
                COMMA@38..39 ","
                FieldDef@39..46
                  Name@39..41
                    WHITESPACE@39..40 " "
                    IDENT@40..41 "c"
                  COLON@41..42 ":"
                  PathType@42..46
                    Path@42..46
                      PathSegment@42..46
                        NameRef@42..46
                          WHITESPACE@42..43 " "
                          IDENT@43..46 "i32"
                R_PAREN@46..47 ")"
        "#]],
    );
}

#[test]
fn type_alias_tuple() {
    check_item(
        "type Pair = (i32, i32);",
        &expect![[r#"
            TypeAlias@0..23
              TYPE_KW@0..4 "type"
              Name@4..9
                WHITESPACE@4..5 " "
                IDENT@5..9 "Pair"
              WHITESPACE@9..10 " "
              EQ@10..11 "="
              TupleType@11..22
                WHITESPACE@11..12 " "
                L_PAREN@12..13 "("
                PathType@13..16
                  Path@13..16
                    PathSegment@13..16
                      NameRef@13..16
                        IDENT@13..16 "i32"
                COMMA@16..17 ","
                PathType@17..21
                  Path@17..21
                    PathSegment@17..21
                      NameRef@17..21
                        WHITESPACE@17..18 " "
                        IDENT@18..21 "i32"
                R_PAREN@21..22 ")"
              SEMI@22..23 ";"
        "#]],
    );
}

#[test]
fn type_alias_array() {
    check_item(
        "type Buffer = [u8; 1024];",
        &expect![[r#"
            TypeAlias@0..25
              TYPE_KW@0..4 "type"
              Name@4..11
                WHITESPACE@4..5 " "
                IDENT@5..11 "Buffer"
              WHITESPACE@11..12 " "
              EQ@12..13 "="
              ArrayType@13..24
                WHITESPACE@13..14 " "
                L_BRACKET@14..15 "["
                PathType@15..17
                  Path@15..17
                    PathSegment@15..17
                      NameRef@15..17
                        IDENT@15..17 "u8"
                SEMI@17..18 ";"
                LiteralExpr@18..23
                  WHITESPACE@18..19 " "
                  INT_LITERAL@19..23 "1024"
                R_BRACKET@23..24 "]"
              SEMI@24..25 ";"
        "#]],
    );
}

#[test]
fn impl_multiple_methods() {
    check_item(
        "impl Foo { fn a() {} fn b() {} fn c() {} }",
        &expect![[r#"
            ImplBlock@0..42
              IMPL_KW@0..4 "impl"
              PathType@4..8
                Path@4..8
                  PathSegment@4..8
                    NameRef@4..8
                      WHITESPACE@4..5 " "
                      IDENT@5..8 "Foo"
              WHITESPACE@8..9 " "
              L_BRACE@9..10 "{"
              FunctionDef@10..20
                WHITESPACE@10..11 " "
                FN_KW@11..13 "fn"
                Name@13..15
                  WHITESPACE@13..14 " "
                  IDENT@14..15 "a"
                ParamList@15..17
                  L_PAREN@15..16 "("
                  R_PAREN@16..17 ")"
                Block@17..20
                  WHITESPACE@17..18 " "
                  L_BRACE@18..19 "{"
                  R_BRACE@19..20 "}"
              FunctionDef@20..30
                WHITESPACE@20..21 " "
                FN_KW@21..23 "fn"
                Name@23..25
                  WHITESPACE@23..24 " "
                  IDENT@24..25 "b"
                ParamList@25..27
                  L_PAREN@25..26 "("
                  R_PAREN@26..27 ")"
                Block@27..30
                  WHITESPACE@27..28 " "
                  L_BRACE@28..29 "{"
                  R_BRACE@29..30 "}"
              FunctionDef@30..40
                WHITESPACE@30..31 " "
                FN_KW@31..33 "fn"
                Name@33..35
                  WHITESPACE@33..34 " "
                  IDENT@34..35 "c"
                ParamList@35..37
                  L_PAREN@35..36 "("
                  R_PAREN@36..37 ")"
                Block@37..40
                  WHITESPACE@37..38 " "
                  L_BRACE@38..39 "{"
                  R_BRACE@39..40 "}"
              WHITESPACE@40..41 " "
              R_BRACE@41..42 "}"
        "#]],
    );
}

#[test]
fn impl_mixed_visibility() {
    check_item(
        "impl Foo { pub fn public() {} fn private() {} }",
        &expect![[r#"
            ImplBlock@0..47
              IMPL_KW@0..4 "impl"
              PathType@4..8
                Path@4..8
                  PathSegment@4..8
                    NameRef@4..8
                      WHITESPACE@4..5 " "
                      IDENT@5..8 "Foo"
              WHITESPACE@8..9 " "
              L_BRACE@9..10 "{"
              FunctionDef@10..29
                Visibility@10..14
                  WHITESPACE@10..11 " "
                  PUB_KW@11..14 "pub"
                WHITESPACE@14..15 " "
                FN_KW@15..17 "fn"
                Name@17..24
                  WHITESPACE@17..18 " "
                  IDENT@18..24 "public"
                ParamList@24..26
                  L_PAREN@24..25 "("
                  R_PAREN@25..26 ")"
                Block@26..29
                  WHITESPACE@26..27 " "
                  L_BRACE@27..28 "{"
                  R_BRACE@28..29 "}"
              FunctionDef@29..45
                WHITESPACE@29..30 " "
                FN_KW@30..32 "fn"
                Name@32..40
                  WHITESPACE@32..33 " "
                  IDENT@33..40 "private"
                ParamList@40..42
                  L_PAREN@40..41 "("
                  R_PAREN@41..42 ")"
                Block@42..45
                  WHITESPACE@42..43 " "
                  L_BRACE@43..44 "{"
                  R_BRACE@44..45 "}"
              WHITESPACE@45..46 " "
              R_BRACE@46..47 "}"
        "#]],
    );
}

#[test]
fn struct_trailing_comma_paren() {
    check_item(
        "struct S(a: i32, b: i32,)",
        &expect![[r#"
            StructDef@0..25
              STRUCT_KW@0..6 "struct"
              Name@6..8
                WHITESPACE@6..7 " "
                IDENT@7..8 "S"
              FieldList@8..25
                L_PAREN@8..9 "("
                FieldDef@9..15
                  Name@9..10
                    IDENT@9..10 "a"
                  COLON@10..11 ":"
                  PathType@11..15
                    Path@11..15
                      PathSegment@11..15
                        NameRef@11..15
                          WHITESPACE@11..12 " "
                          IDENT@12..15 "i32"
                COMMA@15..16 ","
                FieldDef@16..23
                  Name@16..18
                    WHITESPACE@16..17 " "
                    IDENT@17..18 "b"
                  COLON@18..19 ":"
                  PathType@19..23
                    Path@19..23
                      PathSegment@19..23
                        NameRef@19..23
                          WHITESPACE@19..20 " "
                          IDENT@20..23 "i32"
                COMMA@23..24 ","
                R_PAREN@24..25 ")"
        "#]],
    );
}

#[test]
fn fn_trailing_comma_params() {
    check_item(
        "fn foo(a: i32, b: i32,) {}",
        &expect![[r#"
            FunctionDef@0..26
              FN_KW@0..2 "fn"
              Name@2..6
                WHITESPACE@2..3 " "
                IDENT@3..6 "foo"
              ParamList@6..23
                L_PAREN@6..7 "("
                Param@7..13
                  Name@7..8
                    IDENT@7..8 "a"
                  COLON@8..9 ":"
                  PathType@9..13
                    Path@9..13
                      PathSegment@9..13
                        NameRef@9..13
                          WHITESPACE@9..10 " "
                          IDENT@10..13 "i32"
                COMMA@13..14 ","
                Param@14..21
                  Name@14..16
                    WHITESPACE@14..15 " "
                    IDENT@15..16 "b"
                  COLON@16..17 ":"
                  PathType@17..21
                    Path@17..21
                      PathSegment@17..21
                        NameRef@17..21
                          WHITESPACE@17..18 " "
                          IDENT@18..21 "i32"
                COMMA@21..22 ","
                R_PAREN@22..23 ")"
              Block@23..26
                WHITESPACE@23..24 " "
                L_BRACE@24..25 "{"
                R_BRACE@25..26 "}"
        "#]],
    );
}

#[test]
fn source_file_trailing_whitespace() {
    use crate::parser::tests::check_source_file;
    check_source_file(
        "fn main() {}  \n",
        &expect![[r#"
            SourceFile@0..15
              FunctionDef@0..12
                FN_KW@0..2 "fn"
                Name@2..7
                  WHITESPACE@2..3 " "
                  IDENT@3..7 "main"
                ParamList@7..9
                  L_PAREN@7..8 "("
                  R_PAREN@8..9 ")"
                Block@9..12
                  WHITESPACE@9..10 " "
                  L_BRACE@10..11 "{"
                  R_BRACE@11..12 "}"
              WHITESPACE@12..15 "  \n"
        "#]],
    );
}

// === Tuple struct tests ===

#[test]
fn struct_tuple_basic() {
    check_item(
        "struct Pair(i32, i32);",
        &expect![[r#"
            StructDef@0..24
              STRUCT_KW@0..6 "struct"
              Name@6..11
                WHITESPACE@6..7 " "
                IDENT@7..11 "Pair"
              FieldList@11..23
                L_PAREN@11..12 "("
                FieldDef@12..16
                  Name@12..13
                    INT_LITERAL@12..13 "0"
                  PathType@13..16
                    Path@13..16
                      PathSegment@13..16
                        NameRef@13..16
                          IDENT@13..16 "i32"
                COMMA@16..17 ","
                FieldDef@17..22
                  Name@17..19
                    WHITESPACE@17..18 " "
                    INT_LITERAL@18..19 "1"
                  PathType@19..22
                    Path@19..22
                      PathSegment@19..22
                        NameRef@19..22
                          IDENT@19..22 "i32"
                R_PAREN@22..23 ")"
              SEMI@23..24 ";"
        "#]],
    );
}

#[test]
fn struct_tuple_single_field() {
    check_item(
        "struct Wrapper(i32);",
        &expect![[r#"
            StructDef@0..21
              STRUCT_KW@0..6 "struct"
              Name@6..14
                WHITESPACE@6..7 " "
                IDENT@7..14 "Wrapper"
              FieldList@14..20
                L_PAREN@14..15 "("
                FieldDef@15..19
                  Name@15..16
                    INT_LITERAL@15..16 "0"
                  PathType@16..19
                    Path@16..19
                      PathSegment@16..19
                        NameRef@16..19
                          IDENT@16..19 "i32"
                R_PAREN@19..20 ")"
              SEMI@20..21 ";"
        "#]],
    );
}

#[test]
fn struct_tuple_empty() {
    check_item(
        "struct Unit();",
        &expect![[r#"
            StructDef@0..14
              STRUCT_KW@0..6 "struct"
              Name@6..11
                WHITESPACE@6..7 " "
                IDENT@7..11 "Unit"
              FieldList@11..13
                L_PAREN@11..12 "("
                R_PAREN@12..13 ")"
              SEMI@13..14 ";"
        "#]],
    );
}

#[test]
fn struct_tuple_trailing_comma() {
    check_item(
        "struct Single(i32,);",
        &expect![[r#"
            StructDef@0..21
              STRUCT_KW@0..6 "struct"
              Name@6..13
                WHITESPACE@6..7 " "
                IDENT@7..13 "Single"
              FieldList@13..20
                L_PAREN@13..14 "("
                FieldDef@14..18
                  Name@14..15
                    INT_LITERAL@14..15 "0"
                  PathType@15..18
                    Path@15..18
                      PathSegment@15..18
                        NameRef@15..18
                          IDENT@15..18 "i32"
                COMMA@18..19 ","
                R_PAREN@19..20 ")"
              SEMI@20..21 ";"
        "#]],
    );
}

#[test]
fn struct_tuple_with_visibility() {
    check_item(
        "struct Foo(pub i32, i32);",
        &expect![[r#"
            StructDef@0..27
              STRUCT_KW@0..6 "struct"
              Name@6..10
                WHITESPACE@6..7 " "
                IDENT@7..10 "Foo"
              FieldList@10..26
                L_PAREN@10..11 "("
                FieldDef@11..19
                  Visibility@11..14
                    PUB_KW@11..14 "pub"
                  Name@14..16
                    WHITESPACE@14..15 " "
                    INT_LITERAL@15..16 "0"
                  PathType@16..19
                    Path@16..19
                      PathSegment@16..19
                        NameRef@16..19
                          IDENT@16..19 "i32"
                COMMA@19..20 ","
                FieldDef@20..25
                  Name@20..22
                    WHITESPACE@20..21 " "
                    INT_LITERAL@21..22 "1"
                  PathType@22..25
                    Path@22..25
                      PathSegment@22..25
                        NameRef@22..25
                          IDENT@22..25 "i32"
                R_PAREN@25..26 ")"
              SEMI@26..27 ";"
        "#]],
    );
}

#[test]
fn struct_tuple_generic_where() {
    check_item(
        "struct Wrapper(T) where T;",
        &expect![[r#"
            StructDef@0..27
              STRUCT_KW@0..6 "struct"
              Name@6..14
                WHITESPACE@6..7 " "
                IDENT@7..14 "Wrapper"
              FieldList@14..18
                L_PAREN@14..15 "("
                FieldDef@15..17
                  Name@15..16
                    INT_LITERAL@15..16 "0"
                  PathType@16..17
                    Path@16..17
                      PathSegment@16..17
                        NameRef@16..17
                          IDENT@16..17 "T"
                R_PAREN@17..18 ")"
              WhereClause@18..26
                WHITESPACE@18..19 " "
                WHERE_KW@19..24 "where"
                GenericParam@24..26
                  Name@24..26
                    WHITESPACE@24..25 " "
                    IDENT@25..26 "T"
              SEMI@26..27 ";"
        "#]],
    );
}

#[test]
fn struct_tuple_complex_types() {
    check_item(
        "struct Complex(&i32, (u8, u8));",
        &expect![[r#"
            StructDef@0..33
              STRUCT_KW@0..6 "struct"
              Name@6..14
                WHITESPACE@6..7 " "
                IDENT@7..14 "Complex"
              FieldList@14..32
                L_PAREN@14..15 "("
                FieldDef@15..20
                  Name@15..16
                    INT_LITERAL@15..16 "0"
                  RefType@16..20
                    AMP@16..17 "&"
                    PathType@17..20
                      Path@17..20
                        PathSegment@17..20
                          NameRef@17..20
                            IDENT@17..20 "i32"
                COMMA@20..21 ","
                FieldDef@21..31
                  Name@21..23
                    WHITESPACE@21..22 " "
                    INT_LITERAL@22..23 "1"
                  TupleType@23..31
                    L_PAREN@23..24 "("
                    PathType@24..26
                      Path@24..26
                        PathSegment@24..26
                          NameRef@24..26
                            IDENT@24..26 "u8"
                    COMMA@26..27 ","
                    PathType@27..30
                      Path@27..30
                        PathSegment@27..30
                          NameRef@27..30
                            WHITESPACE@27..28 " "
                            IDENT@28..30 "u8"
                    R_PAREN@30..31 ")"
                R_PAREN@31..32 ")"
              SEMI@32..33 ";"
        "#]],
    );
}

#[test]
fn struct_tuple_pub() {
    check_item(
        "pub struct Point(i32, i32);",
        &expect![[r#"
            StructDef@0..29
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              STRUCT_KW@4..10 "struct"
              Name@10..16
                WHITESPACE@10..11 " "
                IDENT@11..16 "Point"
              FieldList@16..28
                L_PAREN@16..17 "("
                FieldDef@17..21
                  Name@17..18
                    INT_LITERAL@17..18 "0"
                  PathType@18..21
                    Path@18..21
                      PathSegment@18..21
                        NameRef@18..21
                          IDENT@18..21 "i32"
                COMMA@21..22 ","
                FieldDef@22..27
                  Name@22..24
                    WHITESPACE@22..23 " "
                    INT_LITERAL@23..24 "1"
                  PathType@24..27
                    Path@24..27
                      PathSegment@24..27
                        NameRef@24..27
                          IDENT@24..27 "i32"
                R_PAREN@27..28 ")"
              SEMI@28..29 ";"
        "#]],
    );
}

// === New Syntax Tests: Colon Return Type ===

#[test]
fn function_colon_return_type() {
    check_item(
        "fn foo(): i32 {}",
        &expect![[r#"
            FunctionDef@0..16
              FN_KW@0..2 "fn"
              Name@2..6
                WHITESPACE@2..3 " "
                IDENT@3..6 "foo"
              ParamList@6..8
                L_PAREN@6..7 "("
                R_PAREN@7..8 ")"
              COLON@8..9 ":"
              PathType@9..13
                Path@9..13
                  PathSegment@9..13
                    NameRef@9..13
                      WHITESPACE@9..10 " "
                      IDENT@10..13 "i32"
              Block@13..16
                WHITESPACE@13..14 " "
                L_BRACE@14..15 "{"
                R_BRACE@15..16 "}"
        "#]],
    );
}

// === New Syntax Tests: Where Clause ===

#[test]
fn function_with_where_clause() {
    check_item(
        "fn id(x: T): T where T {}",
        &expect![[r#"
            FunctionDef@0..25
              FN_KW@0..2 "fn"
              Name@2..5
                WHITESPACE@2..3 " "
                IDENT@3..5 "id"
              ParamList@5..11
                L_PAREN@5..6 "("
                Param@6..10
                  Name@6..7
                    IDENT@6..7 "x"
                  COLON@7..8 ":"
                  PathType@8..10
                    Path@8..10
                      PathSegment@8..10
                        NameRef@8..10
                          WHITESPACE@8..9 " "
                          IDENT@9..10 "T"
                R_PAREN@10..11 ")"
              COLON@11..12 ":"
              PathType@12..14
                Path@12..14
                  PathSegment@12..14
                    NameRef@12..14
                      WHITESPACE@12..13 " "
                      IDENT@13..14 "T"
              WhereClause@14..22
                WHITESPACE@14..15 " "
                WHERE_KW@15..20 "where"
                GenericParam@20..22
                  Name@20..22
                    WHITESPACE@20..21 " "
                    IDENT@21..22 "T"
              Block@22..25
                WHITESPACE@22..23 " "
                L_BRACE@23..24 "{"
                R_BRACE@24..25 "}"
        "#]],
    );
}

#[test]
fn function_with_where_clause_multiple_params() {
    check_item(
        "fn pair(a: T, b: U): (T, U) where T, U {}",
        &expect![[r#"
            FunctionDef@0..41
              FN_KW@0..2 "fn"
              Name@2..7
                WHITESPACE@2..3 " "
                IDENT@3..7 "pair"
              ParamList@7..19
                L_PAREN@7..8 "("
                Param@8..12
                  Name@8..9
                    IDENT@8..9 "a"
                  COLON@9..10 ":"
                  PathType@10..12
                    Path@10..12
                      PathSegment@10..12
                        NameRef@10..12
                          WHITESPACE@10..11 " "
                          IDENT@11..12 "T"
                COMMA@12..13 ","
                Param@13..18
                  Name@13..15
                    WHITESPACE@13..14 " "
                    IDENT@14..15 "b"
                  COLON@15..16 ":"
                  PathType@16..18
                    Path@16..18
                      PathSegment@16..18
                        NameRef@16..18
                          WHITESPACE@16..17 " "
                          IDENT@17..18 "U"
                R_PAREN@18..19 ")"
              COLON@19..20 ":"
              TupleType@20..27
                WHITESPACE@20..21 " "
                L_PAREN@21..22 "("
                PathType@22..23
                  Path@22..23
                    PathSegment@22..23
                      NameRef@22..23
                        IDENT@22..23 "T"
                COMMA@23..24 ","
                PathType@24..26
                  Path@24..26
                    PathSegment@24..26
                      NameRef@24..26
                        WHITESPACE@24..25 " "
                        IDENT@25..26 "U"
                R_PAREN@26..27 ")"
              WhereClause@27..38
                WHITESPACE@27..28 " "
                WHERE_KW@28..33 "where"
                GenericParam@33..35
                  Name@33..35
                    WHITESPACE@33..34 " "
                    IDENT@34..35 "T"
                COMMA@35..36 ","
                GenericParam@36..38
                  Name@36..38
                    WHITESPACE@36..37 " "
                    IDENT@37..38 "U"
              Block@38..41
                WHITESPACE@38..39 " "
                L_BRACE@39..40 "{"
                R_BRACE@40..41 "}"
        "#]],
    );
}

// === New Syntax Tests: Struct with Parentheses ===

#[test]
fn struct_parenthesized_named_fields() {
    check_item(
        "struct Point(x: i32, y: i32)",
        &expect![[r#"
            StructDef@0..28
              STRUCT_KW@0..6 "struct"
              Name@6..12
                WHITESPACE@6..7 " "
                IDENT@7..12 "Point"
              FieldList@12..28
                L_PAREN@12..13 "("
                FieldDef@13..19
                  Name@13..14
                    IDENT@13..14 "x"
                  COLON@14..15 ":"
                  PathType@15..19
                    Path@15..19
                      PathSegment@15..19
                        NameRef@15..19
                          WHITESPACE@15..16 " "
                          IDENT@16..19 "i32"
                COMMA@19..20 ","
                FieldDef@20..27
                  Name@20..22
                    WHITESPACE@20..21 " "
                    IDENT@21..22 "y"
                  COLON@22..23 ":"
                  PathType@23..27
                    Path@23..27
                      PathSegment@23..27
                        NameRef@23..27
                          WHITESPACE@23..24 " "
                          IDENT@24..27 "i32"
                R_PAREN@27..28 ")"
        "#]],
    );
}

#[test]
fn struct_parenthesized_with_where() {
    check_item(
        "struct Box(value: T) where T",
        &expect![[r#"
            StructDef@0..28
              STRUCT_KW@0..6 "struct"
              Name@6..10
                WHITESPACE@6..7 " "
                IDENT@7..10 "Box"
              FieldList@10..20
                L_PAREN@10..11 "("
                FieldDef@11..19
                  Name@11..16
                    IDENT@11..16 "value"
                  COLON@16..17 ":"
                  PathType@17..19
                    Path@17..19
                      PathSegment@17..19
                        NameRef@17..19
                          WHITESPACE@17..18 " "
                          IDENT@18..19 "T"
                R_PAREN@19..20 ")"
              WhereClause@20..28
                WHITESPACE@20..21 " "
                WHERE_KW@21..26 "where"
                GenericParam@26..28
                  Name@26..28
                    WHITESPACE@26..27 " "
                    IDENT@27..28 "T"
        "#]],
    );
}

// === New Syntax Tests: Where Clause with Type Bounds ===

#[test]
fn function_with_where_clause_and_bound() {
    check_item(
        "fn foo(x: T): T where T: Clone {}",
        &expect![[r#"
            FunctionDef@0..33
              FN_KW@0..2 "fn"
              Name@2..6
                WHITESPACE@2..3 " "
                IDENT@3..6 "foo"
              ParamList@6..12
                L_PAREN@6..7 "("
                Param@7..11
                  Name@7..8
                    IDENT@7..8 "x"
                  COLON@8..9 ":"
                  PathType@9..11
                    Path@9..11
                      PathSegment@9..11
                        NameRef@9..11
                          WHITESPACE@9..10 " "
                          IDENT@10..11 "T"
                R_PAREN@11..12 ")"
              COLON@12..13 ":"
              PathType@13..15
                Path@13..15
                  PathSegment@13..15
                    NameRef@13..15
                      WHITESPACE@13..14 " "
                      IDENT@14..15 "T"
              WhereClause@15..30
                WHITESPACE@15..16 " "
                WHERE_KW@16..21 "where"
                GenericParam@21..30
                  Name@21..23
                    WHITESPACE@21..22 " "
                    IDENT@22..23 "T"
                  COLON@23..24 ":"
                  TypeBound@24..30
                    Path@24..30
                      PathSegment@24..30
                        NameRef@24..30
                          WHITESPACE@24..25 " "
                          IDENT@25..30 "Clone"
              Block@30..33
                WHITESPACE@30..31 " "
                L_BRACE@31..32 "{"
                R_BRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn function_with_where_clause_multiple_bounds() {
    check_item(
        "fn foo(x: T) where T: Clone + Debug {}",
        &expect![[r#"
            FunctionDef@0..38
              FN_KW@0..2 "fn"
              Name@2..6
                WHITESPACE@2..3 " "
                IDENT@3..6 "foo"
              ParamList@6..12
                L_PAREN@6..7 "("
                Param@7..11
                  Name@7..8
                    IDENT@7..8 "x"
                  COLON@8..9 ":"
                  PathType@9..11
                    Path@9..11
                      PathSegment@9..11
                        NameRef@9..11
                          WHITESPACE@9..10 " "
                          IDENT@10..11 "T"
                R_PAREN@11..12 ")"
              WhereClause@12..35
                WHITESPACE@12..13 " "
                WHERE_KW@13..18 "where"
                GenericParam@18..35
                  Name@18..20
                    WHITESPACE@18..19 " "
                    IDENT@19..20 "T"
                  COLON@20..21 ":"
                  TypeBound@21..27
                    Path@21..27
                      PathSegment@21..27
                        NameRef@21..27
                          WHITESPACE@21..22 " "
                          IDENT@22..27 "Clone"
                  WHITESPACE@27..28 " "
                  PLUS@28..29 "+"
                  TypeBound@29..35
                    Path@29..35
                      PathSegment@29..35
                        NameRef@29..35
                          WHITESPACE@29..30 " "
                          IDENT@30..35 "Debug"
              Block@35..38
                WHITESPACE@35..36 " "
                L_BRACE@36..37 "{"
                R_BRACE@37..38 "}"
        "#]],
    );
}

#[test]
fn impl_with_where_clause() {
    check_item(
        "impl Box where T {}",
        &expect![[r#"
            ImplBlock@0..19
              IMPL_KW@0..4 "impl"
              PathType@4..8
                Path@4..8
                  PathSegment@4..8
                    NameRef@4..8
                      WHITESPACE@4..5 " "
                      IDENT@5..8 "Box"
              WhereClause@8..16
                WHITESPACE@8..9 " "
                WHERE_KW@9..14 "where"
                GenericParam@14..16
                  Name@14..16
                    WHITESPACE@14..15 " "
                    IDENT@15..16 "T"
              WHITESPACE@16..17 " "
              L_BRACE@17..18 "{"
              R_BRACE@18..19 "}"
        "#]],
    );
}

#[test]
fn type_alias_with_where_clause() {
    check_item(
        "type Callback = T where T: Fn;",
        &expect![[r#"
            TypeAlias@0..30
              TYPE_KW@0..4 "type"
              Name@4..13
                WHITESPACE@4..5 " "
                IDENT@5..13 "Callback"
              WHITESPACE@13..14 " "
              EQ@14..15 "="
              PathType@15..17
                Path@15..17
                  PathSegment@15..17
                    NameRef@15..17
                      WHITESPACE@15..16 " "
                      IDENT@16..17 "T"
              WhereClause@17..29
                WHITESPACE@17..18 " "
                WHERE_KW@18..23 "where"
                GenericParam@23..29
                  Name@23..25
                    WHITESPACE@23..24 " "
                    IDENT@24..25 "T"
                  COLON@25..26 ":"
                  TypeBound@26..29
                    Path@26..29
                      PathSegment@26..29
                        NameRef@26..29
                          WHITESPACE@26..27 " "
                          IDENT@27..29 "Fn"
              SEMI@29..30 ";"
        "#]],
    );
}

// === Named Parameter Tests (Phase 1: LabelSpec parsing) ===

#[test]
fn param_with_underscore_label() {
    check_item(
        "fn add(_ a: i32) {}",
        &expect![[r#"
            FunctionDef@0..19
              FN_KW@0..2 "fn"
              Name@2..6
                WHITESPACE@2..3 " "
                IDENT@3..6 "add"
              ParamList@6..16
                L_PAREN@6..7 "("
                Param@7..15
                  LabelSpec@7..8
                    IDENT@7..8 "_"
                  Name@8..10
                    WHITESPACE@8..9 " "
                    IDENT@9..10 "a"
                  COLON@10..11 ":"
                  PathType@11..15
                    Path@11..15
                      PathSegment@11..15
                        NameRef@11..15
                          WHITESPACE@11..12 " "
                          IDENT@12..15 "i32"
                R_PAREN@15..16 ")"
              Block@16..19
                WHITESPACE@16..17 " "
                L_BRACE@17..18 "{"
                R_BRACE@18..19 "}"
        "#]],
    );
}

#[test]
fn param_with_external_label() {
    check_item(
        "fn greet(to person: String) {}",
        &expect![[r#"
            FunctionDef@0..30
              FN_KW@0..2 "fn"
              Name@2..8
                WHITESPACE@2..3 " "
                IDENT@3..8 "greet"
              ParamList@8..27
                L_PAREN@8..9 "("
                Param@9..26
                  LabelSpec@9..11
                    IDENT@9..11 "to"
                  Name@11..18
                    WHITESPACE@11..12 " "
                    IDENT@12..18 "person"
                  COLON@18..19 ":"
                  PathType@19..26
                    Path@19..26
                      PathSegment@19..26
                        NameRef@19..26
                          WHITESPACE@19..20 " "
                          IDENT@20..26 "String"
                R_PAREN@26..27 ")"
              Block@27..30
                WHITESPACE@27..28 " "
                L_BRACE@28..29 "{"
                R_BRACE@29..30 "}"
        "#]],
    );
}

#[test]
fn param_without_label() {
    // Normal params without label spec should work as before
    check_item(
        "fn foo(x: i32) {}",
        &expect![[r#"
            FunctionDef@0..17
              FN_KW@0..2 "fn"
              Name@2..6
                WHITESPACE@2..3 " "
                IDENT@3..6 "foo"
              ParamList@6..14
                L_PAREN@6..7 "("
                Param@7..13
                  Name@7..8
                    IDENT@7..8 "x"
                  COLON@8..9 ":"
                  PathType@9..13
                    Path@9..13
                      PathSegment@9..13
                        NameRef@9..13
                          WHITESPACE@9..10 " "
                          IDENT@10..13 "i32"
                R_PAREN@13..14 ")"
              Block@14..17
                WHITESPACE@14..15 " "
                L_BRACE@15..16 "{"
                R_BRACE@16..17 "}"
        "#]],
    );
}

#[test]
fn param_mixed_labels() {
    check_item(
        "fn range(from start: i32, _ count: i32) {}",
        &expect![[r#"
            FunctionDef@0..42
              FN_KW@0..2 "fn"
              Name@2..8
                WHITESPACE@2..3 " "
                IDENT@3..8 "range"
              ParamList@8..39
                L_PAREN@8..9 "("
                Param@9..24
                  LabelSpec@9..13
                    IDENT@9..13 "from"
                  Name@13..19
                    WHITESPACE@13..14 " "
                    IDENT@14..19 "start"
                  COLON@19..20 ":"
                  PathType@20..24
                    Path@20..24
                      PathSegment@20..24
                        NameRef@20..24
                          WHITESPACE@20..21 " "
                          IDENT@21..24 "i32"
                COMMA@24..25 ","
                Param@25..38
                  LabelSpec@25..27
                    WHITESPACE@25..26 " "
                    IDENT@26..27 "_"
                  Name@27..33
                    WHITESPACE@27..28 " "
                    IDENT@28..33 "count"
                  COLON@33..34 ":"
                  PathType@34..38
                    Path@34..38
                      PathSegment@34..38
                        NameRef@34..38
                          WHITESPACE@34..35 " "
                          IDENT@35..38 "i32"
                R_PAREN@38..39 ")"
              Block@39..42
                WHITESPACE@39..40 " "
                L_BRACE@40..41 "{"
                R_BRACE@41..42 "}"
        "#]],
    );
}
