use crate::tests::check_item;
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
fn visibility_pub_package() {
    check_item(
        "pub($) fn foo() {}",
        &expect![[r#"
            FunctionDef@0..18
              Visibility@0..6
                PUB_KW@0..3 "pub"
                L_PAREN@3..4 "("
                DOLLAR@4..5 "$"
                R_PAREN@5..6 ")"
              WHITESPACE@6..7 " "
              FN_KW@7..9 "fn"
              Name@9..13
                WHITESPACE@9..10 " "
                IDENT@10..13 "foo"
              ParamList@13..15
                L_PAREN@13..14 "("
                R_PAREN@14..15 ")"
              Block@15..18
                WHITESPACE@15..16 " "
                L_BRACE@16..17 "{"
                R_BRACE@17..18 "}"
        "#]],
    );
}

#[test]
fn visibility_pub_package_path() {
    check_item(
        "pub($.foo.bar) fn baz() {}",
        &expect![[r#"
            FunctionDef@0..26
              Visibility@0..14
                PUB_KW@0..3 "pub"
                L_PAREN@3..4 "("
                DOLLAR@4..5 "$"
                DOT@5..6 "."
                Path@6..13
                  PathSegment@6..9
                    NameRef@6..9
                      IDENT@6..9 "foo"
                  DOT@9..10 "."
                  PathSegment@10..13
                    NameRef@10..13
                      IDENT@10..13 "bar"
                R_PAREN@13..14 ")"
              WHITESPACE@14..15 " "
              FN_KW@15..17 "fn"
              Name@17..21
                WHITESPACE@17..18 " "
                IDENT@18..21 "baz"
              ParamList@21..23
                L_PAREN@21..22 "("
                R_PAREN@22..23 ")"
              Block@23..26
                WHITESPACE@23..24 " "
                L_BRACE@24..25 "{"
                R_BRACE@25..26 "}"
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
    use crate::tests::check_source_file;
    check_source_file(
        "",
        &expect![[r#"
            SourceFile@0..0
        "#]],
    );
}

#[test]
fn source_file_single_function() {
    use crate::tests::check_source_file;
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
    use crate::tests::check_source_file;
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
    use crate::tests::check_source_file;
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
        "struct S(pub a: i32, pub(super) b: i32, c: i32)",
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
                    SUPER_KW@25..30 "super"
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
    use crate::tests::check_source_file;
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

// === Use declaration tests ===

#[test]
fn use_simple_path() {
    check_item(
        "use std.vec.Vec;",
        &expect![[r#"
            UseDecl@0..16
              USE_KW@0..3 "use"
              UseTree@3..15
                WHITESPACE@3..4 " "
                IDENT@4..7 "std"
                DOT@7..8 "."
                IDENT@8..11 "vec"
                DOT@11..12 "."
                IDENT@12..15 "Vec"
              SEMI@15..16 ";"
        "#]],
    );
}

#[test]
fn use_module_prefix() {
    check_item(
        "use module.utils.helper;",
        &expect![[r#"
            UseDecl@0..24
              USE_KW@0..3 "use"
              UseTree@3..23
                WHITESPACE@3..4 " "
                MODULE_KW@4..10 "module"
                DOT@10..11 "."
                IDENT@11..16 "utils"
                DOT@16..17 "."
                IDENT@17..23 "helper"
              SEMI@23..24 ";"
        "#]],
    );
}

#[test]
fn use_super_prefix() {
    check_item(
        "use super.common.Config;",
        &expect![[r#"
            UseDecl@0..24
              USE_KW@0..3 "use"
              UseTree@3..23
                WHITESPACE@3..4 " "
                SUPER_KW@4..9 "super"
                DOT@9..10 "."
                IDENT@10..16 "common"
                DOT@16..17 "."
                IDENT@17..23 "Config"
              SEMI@23..24 ";"
        "#]],
    );
}

#[test]
fn use_self_prefix() {
    check_item(
        "use self.internal.parse;",
        &expect![[r#"
            UseDecl@0..24
              USE_KW@0..3 "use"
              UseTree@3..23
                WHITESPACE@3..4 " "
                SELF_VALUE_KW@4..8 "self"
                DOT@8..9 "."
                IDENT@9..17 "internal"
                DOT@17..18 "."
                IDENT@18..23 "parse"
              SEMI@23..24 ";"
        "#]],
    );
}

#[test]
fn use_with_rename() {
    check_item(
        "use std.collections.HashMap as Map;",
        &expect![[r#"
            UseDecl@0..35
              USE_KW@0..3 "use"
              UseTree@3..34
                WHITESPACE@3..4 " "
                IDENT@4..7 "std"
                DOT@7..8 "."
                IDENT@8..19 "collections"
                DOT@19..20 "."
                IDENT@20..27 "HashMap"
                WHITESPACE@27..28 " "
                AS_KW@28..30 "as"
                Name@30..34
                  WHITESPACE@30..31 " "
                  IDENT@31..34 "Map"
              SEMI@34..35 ";"
        "#]],
    );
}

#[test]
fn use_glob() {
    check_item(
        "use std.prelude.*;",
        &expect![[r#"
            UseDecl@0..18
              USE_KW@0..3 "use"
              UseTree@3..17
                WHITESPACE@3..4 " "
                IDENT@4..7 "std"
                DOT@7..8 "."
                IDENT@8..15 "prelude"
                DOT@15..16 "."
                STAR@16..17 "*"
              SEMI@17..18 ";"
        "#]],
    );
}

#[test]
fn use_group_simple() {
    check_item(
        "use std.io.{Read, Write};",
        &expect![[r#"
            UseDecl@0..25
              USE_KW@0..3 "use"
              UseTree@3..24
                WHITESPACE@3..4 " "
                IDENT@4..7 "std"
                DOT@7..8 "."
                IDENT@8..10 "io"
                DOT@10..11 "."
                UseTreeList@11..24
                  L_BRACE@11..12 "{"
                  UseTree@12..16
                    IDENT@12..16 "Read"
                  COMMA@16..17 ","
                  UseTree@17..23
                    WHITESPACE@17..18 " "
                    IDENT@18..23 "Write"
                  R_BRACE@23..24 "}"
              SEMI@24..25 ";"
        "#]],
    );
}

#[test]
fn use_nested_groups() {
    check_item(
        "use std.{vec.Vec, io.{Read, Write}};",
        &expect![[r#"
            UseDecl@0..36
              USE_KW@0..3 "use"
              UseTree@3..35
                WHITESPACE@3..4 " "
                IDENT@4..7 "std"
                DOT@7..8 "."
                UseTreeList@8..35
                  L_BRACE@8..9 "{"
                  UseTree@9..16
                    IDENT@9..12 "vec"
                    DOT@12..13 "."
                    IDENT@13..16 "Vec"
                  COMMA@16..17 ","
                  UseTree@17..34
                    WHITESPACE@17..18 " "
                    IDENT@18..20 "io"
                    DOT@20..21 "."
                    UseTreeList@21..34
                      L_BRACE@21..22 "{"
                      UseTree@22..26
                        IDENT@22..26 "Read"
                      COMMA@26..27 ","
                      UseTree@27..33
                        WHITESPACE@27..28 " "
                        IDENT@28..33 "Write"
                      R_BRACE@33..34 "}"
                  R_BRACE@34..35 "}"
              SEMI@35..36 ";"
        "#]],
    );
}

#[test]
fn use_pub() {
    check_item(
        "pub use module.types.Type;",
        &expect![[r#"
            UseDecl@0..26
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              USE_KW@4..7 "use"
              UseTree@7..25
                WHITESPACE@7..8 " "
                MODULE_KW@8..14 "module"
                DOT@14..15 "."
                IDENT@15..20 "types"
                DOT@20..21 "."
                IDENT@21..25 "Type"
              SEMI@25..26 ";"
        "#]],
    );
}

#[test]
fn use_pub_glob() {
    check_item(
        "pub use module.prelude.*;",
        &expect![[r#"
            UseDecl@0..25
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              USE_KW@4..7 "use"
              UseTree@7..24
                WHITESPACE@7..8 " "
                MODULE_KW@8..14 "module"
                DOT@14..15 "."
                IDENT@15..22 "prelude"
                DOT@22..23 "."
                STAR@23..24 "*"
              SEMI@24..25 ";"
        "#]],
    );
}

#[test]
fn use_single_segment() {
    check_item(
        "use HashMap;",
        &expect![[r#"
            UseDecl@0..12
              USE_KW@0..3 "use"
              UseTree@3..11
                WHITESPACE@3..4 " "
                IDENT@4..11 "HashMap"
              SEMI@11..12 ";"
        "#]],
    );
}

#[test]
fn use_group_trailing_comma() {
    check_item(
        "use std.{Read, Write,};",
        &expect![[r#"
            UseDecl@0..23
              USE_KW@0..3 "use"
              UseTree@3..22
                WHITESPACE@3..4 " "
                IDENT@4..7 "std"
                DOT@7..8 "."
                UseTreeList@8..22
                  L_BRACE@8..9 "{"
                  UseTree@9..13
                    IDENT@9..13 "Read"
                  COMMA@13..14 ","
                  UseTree@14..20
                    WHITESPACE@14..15 " "
                    IDENT@15..20 "Write"
                  COMMA@20..21 ","
                  R_BRACE@21..22 "}"
              SEMI@22..23 ";"
        "#]],
    );
}

// === Attribute Tests ===

#[test]
fn attribute_simple() {
    check_item(
        "#[test]\nfn foo() {}",
        &expect![[r##"
            FunctionDef@0..19
              Attribute@0..7
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..6
                  IDENT@2..6 "test"
                R_BRACKET@6..7 "]"
              WHITESPACE@7..8 "\n"
              FN_KW@8..10 "fn"
              Name@10..14
                WHITESPACE@10..11 " "
                IDENT@11..14 "foo"
              ParamList@14..16
                L_PAREN@14..15 "("
                R_PAREN@15..16 ")"
              Block@16..19
                WHITESPACE@16..17 " "
                L_BRACE@17..18 "{"
                R_BRACE@18..19 "}"
        "##]],
    );
}

#[test]
fn attribute_dotted_path() {
    check_item(
        "#[foo.bar.baz]\nfn foo() {}",
        &expect![[r##"
            FunctionDef@0..26
              Attribute@0..14
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..13
                  IDENT@2..5 "foo"
                  DOT@5..6 "."
                  IDENT@6..9 "bar"
                  DOT@9..10 "."
                  IDENT@10..13 "baz"
                R_BRACKET@13..14 "]"
              WHITESPACE@14..15 "\n"
              FN_KW@15..17 "fn"
              Name@17..21
                WHITESPACE@17..18 " "
                IDENT@18..21 "foo"
              ParamList@21..23
                L_PAREN@21..22 "("
                R_PAREN@22..23 ")"
              Block@23..26
                WHITESPACE@23..24 " "
                L_BRACE@24..25 "{"
                R_BRACE@25..26 "}"
        "##]],
    );
}

#[test]
fn multiple_attributes() {
    check_item(
        "#[test]\n#[ignore]\nfn foo() {}",
        &expect![[r##"
            FunctionDef@0..29
              Attribute@0..7
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..6
                  IDENT@2..6 "test"
                R_BRACKET@6..7 "]"
              Attribute@7..17
                WHITESPACE@7..8 "\n"
                HASH@8..9 "#"
                L_BRACKET@9..10 "["
                AttrPath@10..16
                  IDENT@10..16 "ignore"
                R_BRACKET@16..17 "]"
              WHITESPACE@17..18 "\n"
              FN_KW@18..20 "fn"
              Name@20..24
                WHITESPACE@20..21 " "
                IDENT@21..24 "foo"
              ParamList@24..26
                L_PAREN@24..25 "("
                R_PAREN@25..26 ")"
              Block@26..29
                WHITESPACE@26..27 " "
                L_BRACE@27..28 "{"
                R_BRACE@28..29 "}"
        "##]],
    );
}

#[test]
fn attribute_with_visibility() {
    check_item(
        "#[test]\npub fn foo() {}",
        &expect![[r##"
            FunctionDef@0..23
              Attribute@0..7
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..6
                  IDENT@2..6 "test"
                R_BRACKET@6..7 "]"
              Visibility@7..11
                WHITESPACE@7..8 "\n"
                PUB_KW@8..11 "pub"
              WHITESPACE@11..12 " "
              FN_KW@12..14 "fn"
              Name@14..18
                WHITESPACE@14..15 " "
                IDENT@15..18 "foo"
              ParamList@18..20
                L_PAREN@18..19 "("
                R_PAREN@19..20 ")"
              Block@20..23
                WHITESPACE@20..21 " "
                L_BRACE@21..22 "{"
                R_BRACE@22..23 "}"
        "##]],
    );
}

#[test]
fn attribute_on_struct() {
    check_item(
        "#[derive(Clone)]\nstruct Point(x: i32)",
        &expect![[r##"
            StructDef@0..37
              Attribute@0..16
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..8
                  IDENT@2..8 "derive"
                AttrInput@8..15
                  L_PAREN@8..9 "("
                  AttrArg@9..14
                    AttrPath@9..14
                      IDENT@9..14 "Clone"
                  R_PAREN@14..15 ")"
                R_BRACKET@15..16 "]"
              WHITESPACE@16..17 "\n"
              STRUCT_KW@17..23 "struct"
              Name@23..29
                WHITESPACE@23..24 " "
                IDENT@24..29 "Point"
              FieldList@29..37
                L_PAREN@29..30 "("
                FieldDef@30..36
                  Name@30..31
                    IDENT@30..31 "x"
                  COLON@31..32 ":"
                  PathType@32..36
                    Path@32..36
                      PathSegment@32..36
                        NameRef@32..36
                          WHITESPACE@32..33 " "
                          IDENT@33..36 "i32"
                R_PAREN@36..37 ")"
        "##]],
    );
}

#[test]
fn attribute_with_key_value_arg() {
    check_item(
        r#"#[cfg(os = "linux")]
fn foo() {}"#,
        &expect![[r##"
            FunctionDef@0..32
              Attribute@0..20
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..5
                  IDENT@2..5 "cfg"
                AttrInput@5..19
                  L_PAREN@5..6 "("
                  AttrArg@6..18
                    IDENT@6..8 "os"
                    WHITESPACE@8..9 " "
                    EQ@9..10 "="
                    WHITESPACE@10..11 " "
                    STRING_LITERAL@11..18 "\"linux\""
                  R_PAREN@18..19 ")"
                R_BRACKET@19..20 "]"
              WHITESPACE@20..21 "\n"
              FN_KW@21..23 "fn"
              Name@23..27
                WHITESPACE@23..24 " "
                IDENT@24..27 "foo"
              ParamList@27..29
                L_PAREN@27..28 "("
                R_PAREN@28..29 ")"
              Block@29..32
                WHITESPACE@29..30 " "
                L_BRACE@30..31 "{"
                R_BRACE@31..32 "}"
        "##]],
    );
}

#[test]
fn attribute_with_multiple_args() {
    check_item(
        r#"#[cfg(os = "linux", arch = "x86")]
fn foo() {}"#,
        &expect![[r##"
            FunctionDef@0..46
              Attribute@0..34
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..5
                  IDENT@2..5 "cfg"
                AttrInput@5..33
                  L_PAREN@5..6 "("
                  AttrArg@6..18
                    IDENT@6..8 "os"
                    WHITESPACE@8..9 " "
                    EQ@9..10 "="
                    WHITESPACE@10..11 " "
                    STRING_LITERAL@11..18 "\"linux\""
                  COMMA@18..19 ","
                  AttrArg@19..32
                    WHITESPACE@19..20 " "
                    IDENT@20..24 "arch"
                    WHITESPACE@24..25 " "
                    EQ@25..26 "="
                    WHITESPACE@26..27 " "
                    STRING_LITERAL@27..32 "\"x86\""
                  R_PAREN@32..33 ")"
                R_BRACKET@33..34 "]"
              WHITESPACE@34..35 "\n"
              FN_KW@35..37 "fn"
              Name@37..41
                WHITESPACE@37..38 " "
                IDENT@38..41 "foo"
              ParamList@41..43
                L_PAREN@41..42 "("
                R_PAREN@42..43 ")"
              Block@43..46
                WHITESPACE@43..44 " "
                L_BRACE@44..45 "{"
                R_BRACE@45..46 "}"
        "##]],
    );
}

#[test]
fn attribute_with_trailing_comma() {
    check_item(
        "#[allow(unused, deprecated,)]\nfn foo() {}",
        &expect![[r##"
            FunctionDef@0..41
              Attribute@0..29
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..7
                  IDENT@2..7 "allow"
                AttrInput@7..28
                  L_PAREN@7..8 "("
                  AttrArg@8..14
                    AttrPath@8..14
                      IDENT@8..14 "unused"
                  COMMA@14..15 ","
                  AttrArg@15..26
                    AttrPath@15..26
                      WHITESPACE@15..16 " "
                      IDENT@16..26 "deprecated"
                  COMMA@26..27 ","
                  R_PAREN@27..28 ")"
                R_BRACKET@28..29 "]"
              WHITESPACE@29..30 "\n"
              FN_KW@30..32 "fn"
              Name@32..36
                WHITESPACE@32..33 " "
                IDENT@33..36 "foo"
              ParamList@36..38
                L_PAREN@36..37 "("
                R_PAREN@37..38 ")"
              Block@38..41
                WHITESPACE@38..39 " "
                L_BRACE@39..40 "{"
                R_BRACE@40..41 "}"
        "##]],
    );
}

#[test]
fn attribute_nested() {
    check_item(
        r#"#[cfg(any(os = "linux", os = "macos"))]
fn foo() {}"#,
        &expect![[r##"
            FunctionDef@0..51
              Attribute@0..39
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..5
                  IDENT@2..5 "cfg"
                AttrInput@5..38
                  L_PAREN@5..6 "("
                  AttrArg@6..37
                    AttrPath@6..9
                      IDENT@6..9 "any"
                    AttrInput@9..37
                      L_PAREN@9..10 "("
                      AttrArg@10..22
                        IDENT@10..12 "os"
                        WHITESPACE@12..13 " "
                        EQ@13..14 "="
                        WHITESPACE@14..15 " "
                        STRING_LITERAL@15..22 "\"linux\""
                      COMMA@22..23 ","
                      AttrArg@23..36
                        WHITESPACE@23..24 " "
                        IDENT@24..26 "os"
                        WHITESPACE@26..27 " "
                        EQ@27..28 "="
                        WHITESPACE@28..29 " "
                        STRING_LITERAL@29..36 "\"macos\""
                      R_PAREN@36..37 ")"
                  R_PAREN@37..38 ")"
                R_BRACKET@38..39 "]"
              WHITESPACE@39..40 "\n"
              FN_KW@40..42 "fn"
              Name@42..46
                WHITESPACE@42..43 " "
                IDENT@43..46 "foo"
              ParamList@46..48
                L_PAREN@46..47 "("
                R_PAREN@47..48 ")"
              Block@48..51
                WHITESPACE@48..49 " "
                L_BRACE@49..50 "{"
                R_BRACE@50..51 "}"
        "##]],
    );
}

// === Inner Attribute Tests ===

use crate::tests::check_source_file;

#[test]
fn inner_attribute_simple() {
    check_source_file(
        r#"#![feature(async)]
fn foo() {}"#,
        &expect![[r##"
            SourceFile@0..30
              InnerAttribute@0..18
                HASH@0..1 "#"
                BANG@1..2 "!"
                L_BRACKET@2..3 "["
                AttrPath@3..10
                  IDENT@3..10 "feature"
                AttrInput@10..17
                  L_PAREN@10..11 "("
                  AttrArg@11..16
                    AttrPath@11..16
                      IDENT@11..16 "async"
                  R_PAREN@16..17 ")"
                R_BRACKET@17..18 "]"
              FunctionDef@18..30
                WHITESPACE@18..19 "\n"
                FN_KW@19..21 "fn"
                Name@21..25
                  WHITESPACE@21..22 " "
                  IDENT@22..25 "foo"
                ParamList@25..27
                  L_PAREN@25..26 "("
                  R_PAREN@26..27 ")"
                Block@27..30
                  WHITESPACE@27..28 " "
                  L_BRACE@28..29 "{"
                  R_BRACE@29..30 "}"
        "##]],
    );
}

#[test]
fn multiple_inner_attributes() {
    check_source_file(
        r#"#![name("mylib")]
#![allow(unused)]
fn main() {}"#,
        &expect![[r##"
            SourceFile@0..48
              InnerAttribute@0..17
                HASH@0..1 "#"
                BANG@1..2 "!"
                L_BRACKET@2..3 "["
                AttrPath@3..7
                  IDENT@3..7 "name"
                AttrInput@7..16
                  L_PAREN@7..8 "("
                  AttrArg@8..15
                    STRING_LITERAL@8..15 "\"mylib\""
                  R_PAREN@15..16 ")"
                R_BRACKET@16..17 "]"
              InnerAttribute@17..35
                WHITESPACE@17..18 "\n"
                HASH@18..19 "#"
                BANG@19..20 "!"
                L_BRACKET@20..21 "["
                AttrPath@21..26
                  IDENT@21..26 "allow"
                AttrInput@26..34
                  L_PAREN@26..27 "("
                  AttrArg@27..33
                    AttrPath@27..33
                      IDENT@27..33 "unused"
                  R_PAREN@33..34 ")"
                R_BRACKET@34..35 "]"
              FunctionDef@35..48
                WHITESPACE@35..36 "\n"
                FN_KW@36..38 "fn"
                Name@38..43
                  WHITESPACE@38..39 " "
                  IDENT@39..43 "main"
                ParamList@43..45
                  L_PAREN@43..44 "("
                  R_PAREN@44..45 ")"
                Block@45..48
                  WHITESPACE@45..46 " "
                  L_BRACE@46..47 "{"
                  R_BRACE@47..48 "}"
        "##]],
    );
}

#[test]
fn attribute_key_value() {
    check_item(
        r##"#[doc = "Documentation"]
fn foo() {}"##,
        &expect![[r##"
            FunctionDef@0..36
              Attribute@0..24
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..5
                  IDENT@2..5 "doc"
                AttrInput@5..23
                  WHITESPACE@5..6 " "
                  EQ@6..7 "="
                  WHITESPACE@7..8 " "
                  STRING_LITERAL@8..23 "\"Documentation\""
                R_BRACKET@23..24 "]"
              WHITESPACE@24..25 "\n"
              FN_KW@25..27 "fn"
              Name@27..31
                WHITESPACE@27..28 " "
                IDENT@28..31 "foo"
              ParamList@31..33
                L_PAREN@31..32 "("
                R_PAREN@32..33 ")"
              Block@33..36
                WHITESPACE@33..34 " "
                L_BRACE@34..35 "{"
                R_BRACE@35..36 "}"
        "##]],
    );
}

#[test]
fn attribute_on_impl_block() {
    check_item(
        "#[cfg(test)]\nimpl Foo { fn bar() {} }",
        &expect![[r##"
            ImplBlock@0..37
              Attribute@0..12
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..5
                  IDENT@2..5 "cfg"
                AttrInput@5..11
                  L_PAREN@5..6 "("
                  AttrArg@6..10
                    AttrPath@6..10
                      IDENT@6..10 "test"
                  R_PAREN@10..11 ")"
                R_BRACKET@11..12 "]"
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
              FunctionDef@23..35
                WHITESPACE@23..24 " "
                FN_KW@24..26 "fn"
                Name@26..30
                  WHITESPACE@26..27 " "
                  IDENT@27..30 "bar"
                ParamList@30..32
                  L_PAREN@30..31 "("
                  R_PAREN@31..32 ")"
                Block@32..35
                  WHITESPACE@32..33 " "
                  L_BRACE@33..34 "{"
                  R_BRACE@34..35 "}"
              WHITESPACE@35..36 " "
              R_BRACE@36..37 "}"
        "##]],
    );
}

#[test]
fn attribute_on_impl_method() {
    check_item(
        "impl Foo {\n  #[test]\n  fn bar() {}\n}",
        &expect![[r##"
            ImplBlock@0..36
              IMPL_KW@0..4 "impl"
              PathType@4..8
                Path@4..8
                  PathSegment@4..8
                    NameRef@4..8
                      WHITESPACE@4..5 " "
                      IDENT@5..8 "Foo"
              WHITESPACE@8..9 " "
              L_BRACE@9..10 "{"
              FunctionDef@10..34
                Attribute@10..20
                  WHITESPACE@10..13 "\n  "
                  HASH@13..14 "#"
                  L_BRACKET@14..15 "["
                  AttrPath@15..19
                    IDENT@15..19 "test"
                  R_BRACKET@19..20 "]"
                WHITESPACE@20..23 "\n  "
                FN_KW@23..25 "fn"
                Name@25..29
                  WHITESPACE@25..26 " "
                  IDENT@26..29 "bar"
                ParamList@29..31
                  L_PAREN@29..30 "("
                  R_PAREN@30..31 ")"
                Block@31..34
                  WHITESPACE@31..32 " "
                  L_BRACE@32..33 "{"
                  R_BRACE@33..34 "}"
              WHITESPACE@34..35 "\n"
              R_BRACE@35..36 "}"
        "##]],
    );
}

// === Attribute Error Handling Tests ===

use crate::parse;

#[test]
fn attribute_error_empty() {
    // Empty #[]
    let result = parse("#[]\nfn foo() {}");
    assert!(!result.ok());
    // The function should still be parsed
    let tree = result.debug_tree();
    assert!(tree.contains("FunctionDef"));
}

#[test]
fn attribute_error_unclosed_paren() {
    // Unclosed ( in attribute args - missing )
    let result = parse("#[cfg(debug]\nfn foo() {}");
    assert!(!result.ok());
    let tree = result.debug_tree();
    // Should have the attribute and function
    assert!(tree.contains("Attribute"));
    assert!(tree.contains("FunctionDef"));
}

#[test]
fn attribute_recovery_between_items() {
    // Error between valid items with attributes
    let result = parse("#[test]\nfn a() {} @@@ #[test]\nfn b() {}");
    assert!(!result.ok());
    let tree = result.debug_tree();
    // Both functions should be parsed
    assert_eq!(tree.matches("FunctionDef").count(), 2);
}

#[test]
fn attribute_error_missing_bracket() {
    // Missing [ after # - the # is consumed, then parsing continues
    let result = parse("#test\nfn foo() {}");
    assert!(!result.ok());
    // Should still have some structure
    let tree = result.debug_tree();
    assert!(tree.contains("Attribute") || tree.contains("FN_KW"));
}

#[test]
fn attribute_error_unclosed_bracket() {
    // Unclosed [ - should recover and still parse the function
    let result = parse("#[test\nfn foo() {}");
    assert!(!result.ok());
    let tree = result.debug_tree();
    // Should have parsed the function after recovery
    assert!(tree.contains("FunctionDef"));
}

#[test]
fn attribute_error_nested_unclosed() {
    // Nested attribute with unclosed paren
    let result = parse("#[cfg(any(a, b)]\nfn foo() {}");
    assert!(!result.ok());
    let tree = result.debug_tree();
    // Should still parse something
    assert!(tree.contains("Attribute"));
}

// ===== Module Tests =====

#[test]
fn module_empty() {
    check_item(
        "module foo {}",
        &expect![[r#"
            ModuleDef@0..13
              MODULE_KW@0..6 "module"
              Name@6..10
                WHITESPACE@6..7 " "
                IDENT@7..10 "foo"
              WHITESPACE@10..11 " "
              L_BRACE@11..12 "{"
              R_BRACE@12..13 "}"
        "#]],
    );
}

#[test]
fn module_with_function() {
    check_item(
        "module internal { fn helper(): i32 { 42 } }",
        &expect![[r#"
            ModuleDef@0..43
              MODULE_KW@0..6 "module"
              Name@6..15
                WHITESPACE@6..7 " "
                IDENT@7..15 "internal"
              WHITESPACE@15..16 " "
              L_BRACE@16..17 "{"
              FunctionDef@17..41
                WHITESPACE@17..18 " "
                FN_KW@18..20 "fn"
                Name@20..27
                  WHITESPACE@20..21 " "
                  IDENT@21..27 "helper"
                ParamList@27..29
                  L_PAREN@27..28 "("
                  R_PAREN@28..29 ")"
                COLON@29..30 ":"
                PathType@30..34
                  Path@30..34
                    PathSegment@30..34
                      NameRef@30..34
                        WHITESPACE@30..31 " "
                        IDENT@31..34 "i32"
                Block@34..41
                  WHITESPACE@34..35 " "
                  L_BRACE@35..36 "{"
                  LiteralExpr@36..39
                    WHITESPACE@36..37 " "
                    INT_LITERAL@37..39 "42"
                  WHITESPACE@39..40 " "
                  R_BRACE@40..41 "}"
              WHITESPACE@41..42 " "
              R_BRACE@42..43 "}"
        "#]],
    );
}

#[test]
fn module_pub() {
    check_item(
        "pub module api { pub fn endpoint() {} }",
        &expect![[r#"
            ModuleDef@0..39
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              MODULE_KW@4..10 "module"
              Name@10..14
                WHITESPACE@10..11 " "
                IDENT@11..14 "api"
              WHITESPACE@14..15 " "
              L_BRACE@15..16 "{"
              FunctionDef@16..37
                Visibility@16..20
                  WHITESPACE@16..17 " "
                  PUB_KW@17..20 "pub"
                WHITESPACE@20..21 " "
                FN_KW@21..23 "fn"
                Name@23..32
                  WHITESPACE@23..24 " "
                  IDENT@24..32 "endpoint"
                ParamList@32..34
                  L_PAREN@32..33 "("
                  R_PAREN@33..34 ")"
                Block@34..37
                  WHITESPACE@34..35 " "
                  L_BRACE@35..36 "{"
                  R_BRACE@36..37 "}"
              WHITESPACE@37..38 " "
              R_BRACE@38..39 "}"
        "#]],
    );
}

#[test]
fn module_nested() {
    check_item(
        "module outer { module inner { fn deep() {} } }",
        &expect![[r#"
            ModuleDef@0..46
              MODULE_KW@0..6 "module"
              Name@6..12
                WHITESPACE@6..7 " "
                IDENT@7..12 "outer"
              WHITESPACE@12..13 " "
              L_BRACE@13..14 "{"
              ModuleDef@14..44
                WHITESPACE@14..15 " "
                MODULE_KW@15..21 "module"
                Name@21..27
                  WHITESPACE@21..22 " "
                  IDENT@22..27 "inner"
                WHITESPACE@27..28 " "
                L_BRACE@28..29 "{"
                FunctionDef@29..42
                  WHITESPACE@29..30 " "
                  FN_KW@30..32 "fn"
                  Name@32..37
                    WHITESPACE@32..33 " "
                    IDENT@33..37 "deep"
                  ParamList@37..39
                    L_PAREN@37..38 "("
                    R_PAREN@38..39 ")"
                  Block@39..42
                    WHITESPACE@39..40 " "
                    L_BRACE@40..41 "{"
                    R_BRACE@41..42 "}"
                WHITESPACE@42..43 " "
                R_BRACE@43..44 "}"
              WHITESPACE@44..45 " "
              R_BRACE@45..46 "}"
        "#]],
    );
}

#[test]
fn module_with_struct() {
    check_item(
        "module types { pub struct Point(x: i32, y: i32) }",
        &expect![[r#"
            ModuleDef@0..49
              MODULE_KW@0..6 "module"
              Name@6..12
                WHITESPACE@6..7 " "
                IDENT@7..12 "types"
              WHITESPACE@12..13 " "
              L_BRACE@13..14 "{"
              StructDef@14..47
                Visibility@14..18
                  WHITESPACE@14..15 " "
                  PUB_KW@15..18 "pub"
                WHITESPACE@18..19 " "
                STRUCT_KW@19..25 "struct"
                Name@25..31
                  WHITESPACE@25..26 " "
                  IDENT@26..31 "Point"
                FieldList@31..47
                  L_PAREN@31..32 "("
                  FieldDef@32..38
                    Name@32..33
                      IDENT@32..33 "x"
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
                      IDENT@40..41 "y"
                    COLON@41..42 ":"
                    PathType@42..46
                      Path@42..46
                        PathSegment@42..46
                          NameRef@42..46
                            WHITESPACE@42..43 " "
                            IDENT@43..46 "i32"
                  R_PAREN@46..47 ")"
              WHITESPACE@47..48 " "
              R_BRACE@48..49 "}"
        "#]],
    );
}

#[test]
fn module_mixed_items() {
    check_item(
        "module utils { fn private() {} pub fn public() {} type Alias = i32; }",
        &expect![[r#"
            ModuleDef@0..69
              MODULE_KW@0..6 "module"
              Name@6..12
                WHITESPACE@6..7 " "
                IDENT@7..12 "utils"
              WHITESPACE@12..13 " "
              L_BRACE@13..14 "{"
              FunctionDef@14..30
                WHITESPACE@14..15 " "
                FN_KW@15..17 "fn"
                Name@17..25
                  WHITESPACE@17..18 " "
                  IDENT@18..25 "private"
                ParamList@25..27
                  L_PAREN@25..26 "("
                  R_PAREN@26..27 ")"
                Block@27..30
                  WHITESPACE@27..28 " "
                  L_BRACE@28..29 "{"
                  R_BRACE@29..30 "}"
              FunctionDef@30..49
                Visibility@30..34
                  WHITESPACE@30..31 " "
                  PUB_KW@31..34 "pub"
                WHITESPACE@34..35 " "
                FN_KW@35..37 "fn"
                Name@37..44
                  WHITESPACE@37..38 " "
                  IDENT@38..44 "public"
                ParamList@44..46
                  L_PAREN@44..45 "("
                  R_PAREN@45..46 ")"
                Block@46..49
                  WHITESPACE@46..47 " "
                  L_BRACE@47..48 "{"
                  R_BRACE@48..49 "}"
              TypeAlias@49..67
                WHITESPACE@49..50 " "
                TYPE_KW@50..54 "type"
                Name@54..60
                  WHITESPACE@54..55 " "
                  IDENT@55..60 "Alias"
                WHITESPACE@60..61 " "
                EQ@61..62 "="
                PathType@62..66
                  Path@62..66
                    PathSegment@62..66
                      NameRef@62..66
                        WHITESPACE@62..63 " "
                        IDENT@63..66 "i32"
                SEMI@66..67 ";"
              WHITESPACE@67..68 " "
              R_BRACE@68..69 "}"
        "#]],
    );
}

// === Enum Tests ===

#[test]
fn enum_empty() {
    check_item(
        "enum Empty {}",
        &expect![[r#"
            EnumDef@0..13
              ENUM_KW@0..4 "enum"
              Name@4..10
                WHITESPACE@4..5 " "
                IDENT@5..10 "Empty"
              WHITESPACE@10..11 " "
              L_BRACE@11..12 "{"
              R_BRACE@12..13 "}"
        "#]],
    );
}

#[test]
fn enum_unit_variants() {
    check_item(
        "enum Color { Red, Green, Blue }",
        &expect![[r#"
            EnumDef@0..31
              ENUM_KW@0..4 "enum"
              Name@4..10
                WHITESPACE@4..5 " "
                IDENT@5..10 "Color"
              WHITESPACE@10..11 " "
              L_BRACE@11..12 "{"
              VariantList@12..29
                Variant@12..16
                  Name@12..16
                    WHITESPACE@12..13 " "
                    IDENT@13..16 "Red"
                COMMA@16..17 ","
                Variant@17..23
                  Name@17..23
                    WHITESPACE@17..18 " "
                    IDENT@18..23 "Green"
                COMMA@23..24 ","
                Variant@24..29
                  Name@24..29
                    WHITESPACE@24..25 " "
                    IDENT@25..29 "Blue"
              WHITESPACE@29..30 " "
              R_BRACE@30..31 "}"
        "#]],
    );
}

#[test]
fn enum_trailing_comma() {
    check_item(
        "enum X { A, B, }",
        &expect![[r#"
            EnumDef@0..16
              ENUM_KW@0..4 "enum"
              Name@4..6
                WHITESPACE@4..5 " "
                IDENT@5..6 "X"
              WHITESPACE@6..7 " "
              L_BRACE@7..8 "{"
              VariantList@8..14
                Variant@8..10
                  Name@8..10
                    WHITESPACE@8..9 " "
                    IDENT@9..10 "A"
                COMMA@10..11 ","
                Variant@11..13
                  Name@11..13
                    WHITESPACE@11..12 " "
                    IDENT@12..13 "B"
                COMMA@13..14 ","
              WHITESPACE@14..15 " "
              R_BRACE@15..16 "}"
        "#]],
    );
}

#[test]
fn enum_tuple_variant() {
    check_item(
        "enum Option { Some(T), None }",
        &expect![[r#"
            EnumDef@0..30
              ENUM_KW@0..4 "enum"
              Name@4..11
                WHITESPACE@4..5 " "
                IDENT@5..11 "Option"
              WHITESPACE@11..12 " "
              L_BRACE@12..13 "{"
              VariantList@13..28
                Variant@13..22
                  Name@13..18
                    WHITESPACE@13..14 " "
                    IDENT@14..18 "Some"
                  FieldList@18..22
                    L_PAREN@18..19 "("
                    FieldDef@19..21
                      Name@19..20
                        INT_LITERAL@19..20 "0"
                      PathType@20..21
                        Path@20..21
                          PathSegment@20..21
                            NameRef@20..21
                              IDENT@20..21 "T"
                    R_PAREN@21..22 ")"
                COMMA@22..23 ","
                Variant@23..28
                  Name@23..28
                    WHITESPACE@23..24 " "
                    IDENT@24..28 "None"
              WHITESPACE@28..29 " "
              R_BRACE@29..30 "}"
        "#]],
    );
}

#[test]
fn enum_struct_variant() {
    check_item(
        "enum Msg { Move(x: i32, y: i32) }",
        &expect![[r#"
            EnumDef@0..33
              ENUM_KW@0..4 "enum"
              Name@4..8
                WHITESPACE@4..5 " "
                IDENT@5..8 "Msg"
              WHITESPACE@8..9 " "
              L_BRACE@9..10 "{"
              VariantList@10..31
                Variant@10..31
                  Name@10..15
                    WHITESPACE@10..11 " "
                    IDENT@11..15 "Move"
                  FieldList@15..31
                    L_PAREN@15..16 "("
                    FieldDef@16..22
                      Name@16..17
                        IDENT@16..17 "x"
                      COLON@17..18 ":"
                      PathType@18..22
                        Path@18..22
                          PathSegment@18..22
                            NameRef@18..22
                              WHITESPACE@18..19 " "
                              IDENT@19..22 "i32"
                    COMMA@22..23 ","
                    FieldDef@23..30
                      Name@23..25
                        WHITESPACE@23..24 " "
                        IDENT@24..25 "y"
                      COLON@25..26 ":"
                      PathType@26..30
                        Path@26..30
                          PathSegment@26..30
                            NameRef@26..30
                              WHITESPACE@26..27 " "
                              IDENT@27..30 "i32"
                    R_PAREN@30..31 ")"
              WHITESPACE@31..32 " "
              R_BRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn enum_where_clause() {
    check_item(
        "enum Option { Some(T), None } where T",
        &expect![[r#"
            EnumDef@0..38
              ENUM_KW@0..4 "enum"
              Name@4..11
                WHITESPACE@4..5 " "
                IDENT@5..11 "Option"
              WHITESPACE@11..12 " "
              L_BRACE@12..13 "{"
              VariantList@13..28
                Variant@13..22
                  Name@13..18
                    WHITESPACE@13..14 " "
                    IDENT@14..18 "Some"
                  FieldList@18..22
                    L_PAREN@18..19 "("
                    FieldDef@19..21
                      Name@19..20
                        INT_LITERAL@19..20 "0"
                      PathType@20..21
                        Path@20..21
                          PathSegment@20..21
                            NameRef@20..21
                              IDENT@20..21 "T"
                    R_PAREN@21..22 ")"
                COMMA@22..23 ","
                Variant@23..28
                  Name@23..28
                    WHITESPACE@23..24 " "
                    IDENT@24..28 "None"
              WHITESPACE@28..29 " "
              R_BRACE@29..30 "}"
              WhereClause@30..38
                WHITESPACE@30..31 " "
                WHERE_KW@31..36 "where"
                GenericParam@36..38
                  Name@36..38
                    WHITESPACE@36..37 " "
                    IDENT@37..38 "T"
        "#]],
    );
}

#[test]
fn enum_pub() {
    check_item(
        "pub enum Vis { A }",
        &expect![[r#"
            EnumDef@0..18
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              ENUM_KW@4..8 "enum"
              Name@8..12
                WHITESPACE@8..9 " "
                IDENT@9..12 "Vis"
              WHITESPACE@12..13 " "
              L_BRACE@13..14 "{"
              VariantList@14..16
                Variant@14..16
                  Name@14..16
                    WHITESPACE@14..15 " "
                    IDENT@15..16 "A"
              WHITESPACE@16..17 " "
              R_BRACE@17..18 "}"
        "#]],
    );
}

#[test]
fn enum_with_attribute() {
    check_item(
        "#[derive(Debug)] enum X { A }",
        &expect![[r##"
            EnumDef@0..29
              Attribute@0..16
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..8
                  IDENT@2..8 "derive"
                AttrInput@8..15
                  L_PAREN@8..9 "("
                  AttrArg@9..14
                    AttrPath@9..14
                      IDENT@9..14 "Debug"
                  R_PAREN@14..15 ")"
                R_BRACKET@15..16 "]"
              WHITESPACE@16..17 " "
              ENUM_KW@17..21 "enum"
              Name@21..23
                WHITESPACE@21..22 " "
                IDENT@22..23 "X"
              WHITESPACE@23..24 " "
              L_BRACE@24..25 "{"
              VariantList@25..27
                Variant@25..27
                  Name@25..27
                    WHITESPACE@25..26 " "
                    IDENT@26..27 "A"
              WHITESPACE@27..28 " "
              R_BRACE@28..29 "}"
        "##]],
    );
}

// === Trait Tests ===

#[test]
fn trait_empty() {
    check_item(
        "trait Empty {}",
        &expect![[r#"
            TraitDef@0..14
              TRAIT_KW@0..5 "trait"
              Name@5..11
                WHITESPACE@5..6 " "
                IDENT@6..11 "Empty"
              WHITESPACE@11..12 " "
              L_BRACE@12..13 "{"
              R_BRACE@13..14 "}"
        "#]],
    );
}

#[test]
fn trait_method_signature() {
    check_item(
        "trait Clone { fn clone(&self): Self; }",
        &expect![[r#"
            TraitDef@0..38
              TRAIT_KW@0..5 "trait"
              Name@5..11
                WHITESPACE@5..6 " "
                IDENT@6..11 "Clone"
              WHITESPACE@11..12 " "
              L_BRACE@12..13 "{"
              TraitItem@13..36
                WHITESPACE@13..14 " "
                FN_KW@14..16 "fn"
                Name@16..22
                  WHITESPACE@16..17 " "
                  IDENT@17..22 "clone"
                ParamList@22..29
                  L_PAREN@22..23 "("
                  SelfParam@23..28
                    AMP@23..24 "&"
                    SELF_VALUE_KW@24..28 "self"
                  R_PAREN@28..29 ")"
                COLON@29..30 ":"
                PathType@30..35
                  Path@30..35
                    PathSegment@30..35
                      NameRef@30..35
                        WHITESPACE@30..31 " "
                        SELF_TYPE_KW@31..35 "Self"
                SEMI@35..36 ";"
              WHITESPACE@36..37 " "
              R_BRACE@37..38 "}"
        "#]],
    );
}

#[test]
fn trait_method_default() {
    check_item(
        "trait Foo { fn bar(): i32 { 0 } }",
        &expect![[r#"
            TraitDef@0..33
              TRAIT_KW@0..5 "trait"
              Name@5..9
                WHITESPACE@5..6 " "
                IDENT@6..9 "Foo"
              WHITESPACE@9..10 " "
              L_BRACE@10..11 "{"
              TraitItem@11..31
                WHITESPACE@11..12 " "
                FN_KW@12..14 "fn"
                Name@14..18
                  WHITESPACE@14..15 " "
                  IDENT@15..18 "bar"
                ParamList@18..20
                  L_PAREN@18..19 "("
                  R_PAREN@19..20 ")"
                COLON@20..21 ":"
                PathType@21..25
                  Path@21..25
                    PathSegment@21..25
                      NameRef@21..25
                        WHITESPACE@21..22 " "
                        IDENT@22..25 "i32"
                Block@25..31
                  WHITESPACE@25..26 " "
                  L_BRACE@26..27 "{"
                  LiteralExpr@27..29
                    WHITESPACE@27..28 " "
                    INT_LITERAL@28..29 "0"
                  WHITESPACE@29..30 " "
                  R_BRACE@30..31 "}"
              WHITESPACE@31..32 " "
              R_BRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn trait_associated_type() {
    check_item(
        "trait Iterator { type Item; }",
        &expect![[r#"
            TraitDef@0..29
              TRAIT_KW@0..5 "trait"
              Name@5..14
                WHITESPACE@5..6 " "
                IDENT@6..14 "Iterator"
              WHITESPACE@14..15 " "
              L_BRACE@15..16 "{"
              AssociatedType@16..27
                WHITESPACE@16..17 " "
                TYPE_KW@17..21 "type"
                Name@21..26
                  WHITESPACE@21..22 " "
                  IDENT@22..26 "Item"
                SEMI@26..27 ";"
              WHITESPACE@27..28 " "
              R_BRACE@28..29 "}"
        "#]],
    );
}

#[test]
fn trait_associated_type_bounds() {
    check_item(
        "trait Foo { type Output: Clone; }",
        &expect![[r#"
            TraitDef@0..33
              TRAIT_KW@0..5 "trait"
              Name@5..9
                WHITESPACE@5..6 " "
                IDENT@6..9 "Foo"
              WHITESPACE@9..10 " "
              L_BRACE@10..11 "{"
              AssociatedType@11..31
                WHITESPACE@11..12 " "
                TYPE_KW@12..16 "type"
                Name@16..23
                  WHITESPACE@16..17 " "
                  IDENT@17..23 "Output"
                COLON@23..24 ":"
                TypeBound@24..30
                  Path@24..30
                    PathSegment@24..30
                      NameRef@24..30
                        WHITESPACE@24..25 " "
                        IDENT@25..30 "Clone"
                SEMI@30..31 ";"
              WHITESPACE@31..32 " "
              R_BRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn trait_supertrait() {
    check_item(
        "trait Eq: PartialEq {}",
        &expect![[r#"
            TraitDef@0..22
              TRAIT_KW@0..5 "trait"
              Name@5..8
                WHITESPACE@5..6 " "
                IDENT@6..8 "Eq"
              COLON@8..9 ":"
              TypeBound@9..19
                Path@9..19
                  PathSegment@9..19
                    NameRef@9..19
                      WHITESPACE@9..10 " "
                      IDENT@10..19 "PartialEq"
              WHITESPACE@19..20 " "
              L_BRACE@20..21 "{"
              R_BRACE@21..22 "}"
        "#]],
    );
}

#[test]
fn trait_multiple_supertraits() {
    check_item(
        "trait Ord: PartialOrd + Eq {}",
        &expect![[r#"
            TraitDef@0..29
              TRAIT_KW@0..5 "trait"
              Name@5..9
                WHITESPACE@5..6 " "
                IDENT@6..9 "Ord"
              COLON@9..10 ":"
              TypeBound@10..21
                Path@10..21
                  PathSegment@10..21
                    NameRef@10..21
                      WHITESPACE@10..11 " "
                      IDENT@11..21 "PartialOrd"
              WHITESPACE@21..22 " "
              PLUS@22..23 "+"
              TypeBound@23..26
                Path@23..26
                  PathSegment@23..26
                    NameRef@23..26
                      WHITESPACE@23..24 " "
                      IDENT@24..26 "Eq"
              WHITESPACE@26..27 " "
              L_BRACE@27..28 "{"
              R_BRACE@28..29 "}"
        "#]],
    );
}

#[test]
fn trait_where_clause() {
    check_item(
        "trait Add where RHS { type Output; }",
        &expect![[r#"
            TraitDef@0..36
              TRAIT_KW@0..5 "trait"
              Name@5..9
                WHITESPACE@5..6 " "
                IDENT@6..9 "Add"
              WhereClause@9..19
                WHITESPACE@9..10 " "
                WHERE_KW@10..15 "where"
                GenericParam@15..19
                  Name@15..19
                    WHITESPACE@15..16 " "
                    IDENT@16..19 "RHS"
              WHITESPACE@19..20 " "
              L_BRACE@20..21 "{"
              AssociatedType@21..34
                WHITESPACE@21..22 " "
                TYPE_KW@22..26 "type"
                Name@26..33
                  WHITESPACE@26..27 " "
                  IDENT@27..33 "Output"
                SEMI@33..34 ";"
              WHITESPACE@34..35 " "
              R_BRACE@35..36 "}"
        "#]],
    );
}

#[test]
fn trait_unsafe() {
    check_item(
        "unsafe trait Send {}",
        &expect![[r#"
            TraitDef@0..20
              UNSAFE_KW@0..6 "unsafe"
              WHITESPACE@6..7 " "
              TRAIT_KW@7..12 "trait"
              Name@12..17
                WHITESPACE@12..13 " "
                IDENT@13..17 "Send"
              WHITESPACE@17..18 " "
              L_BRACE@18..19 "{"
              R_BRACE@19..20 "}"
        "#]],
    );
}

// === Const definition tests ===

#[test]
fn const_def_simple() {
    check_item(
        "const MAX: i32 = 100;",
        &expect![[r#"
            ConstDef@0..21
              CONST_KW@0..5 "const"
              Name@5..9
                WHITESPACE@5..6 " "
                IDENT@6..9 "MAX"
              COLON@9..10 ":"
              PathType@10..14
                Path@10..14
                  PathSegment@10..14
                    NameRef@10..14
                      WHITESPACE@10..11 " "
                      IDENT@11..14 "i32"
              WHITESPACE@14..15 " "
              EQ@15..16 "="
              LiteralExpr@16..20
                WHITESPACE@16..17 " "
                INT_LITERAL@17..20 "100"
              SEMI@20..21 ";"
        "#]],
    );
}

#[test]
fn const_def_pub() {
    check_item(
        "pub const PI: f64 = 3.14;",
        &expect![[r#"
            ConstDef@0..25
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              CONST_KW@4..9 "const"
              Name@9..12
                WHITESPACE@9..10 " "
                IDENT@10..12 "PI"
              COLON@12..13 ":"
              PathType@13..17
                Path@13..17
                  PathSegment@13..17
                    NameRef@13..17
                      WHITESPACE@13..14 " "
                      IDENT@14..17 "f64"
              WHITESPACE@17..18 " "
              EQ@18..19 "="
              LiteralExpr@19..24
                WHITESPACE@19..20 " "
                FLOAT_LITERAL@20..24 "3.14"
              SEMI@24..25 ";"
        "#]],
    );
}

#[test]
fn const_def_with_attribute() {
    check_item(
        "#[deprecated] const OLD: i32 = 0;",
        &expect![[r##"
            ConstDef@0..33
              Attribute@0..13
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..12
                  IDENT@2..12 "deprecated"
                R_BRACKET@12..13 "]"
              WHITESPACE@13..14 " "
              CONST_KW@14..19 "const"
              Name@19..23
                WHITESPACE@19..20 " "
                IDENT@20..23 "OLD"
              COLON@23..24 ":"
              PathType@24..28
                Path@24..28
                  PathSegment@24..28
                    NameRef@24..28
                      WHITESPACE@24..25 " "
                      IDENT@25..28 "i32"
              WHITESPACE@28..29 " "
              EQ@29..30 "="
              LiteralExpr@30..32
                WHITESPACE@30..31 " "
                INT_LITERAL@31..32 "0"
              SEMI@32..33 ";"
        "##]],
    );
}

#[test]
fn const_def_complex_expr() {
    check_item(
        "const SIZE: usize = 1024 * 4;",
        &expect![[r#"
            ConstDef@0..29
              CONST_KW@0..5 "const"
              Name@5..10
                WHITESPACE@5..6 " "
                IDENT@6..10 "SIZE"
              COLON@10..11 ":"
              PathType@11..17
                Path@11..17
                  PathSegment@11..17
                    NameRef@11..17
                      WHITESPACE@11..12 " "
                      IDENT@12..17 "usize"
              WHITESPACE@17..18 " "
              EQ@18..19 "="
              BinExpr@19..28
                LiteralExpr@19..24
                  WHITESPACE@19..20 " "
                  INT_LITERAL@20..24 "1024"
                WHITESPACE@24..25 " "
                STAR@25..26 "*"
                LiteralExpr@26..28
                  WHITESPACE@26..27 " "
                  INT_LITERAL@27..28 "4"
              SEMI@28..29 ";"
        "#]],
    );
}

#[test]
fn const_def_no_semicolon() {
    check_item(
        "const X: i32 = 1",
        &expect![[r#"
            ConstDef@0..16
              CONST_KW@0..5 "const"
              Name@5..7
                WHITESPACE@5..6 " "
                IDENT@6..7 "X"
              COLON@7..8 ":"
              PathType@8..12
                Path@8..12
                  PathSegment@8..12
                    NameRef@8..12
                      WHITESPACE@8..9 " "
                      IDENT@9..12 "i32"
              WHITESPACE@12..13 " "
              EQ@13..14 "="
              LiteralExpr@14..16
                WHITESPACE@14..15 " "
                INT_LITERAL@15..16 "1"
        "#]],
    );
}

// === Static definition tests ===

#[test]
fn static_def_simple() {
    check_item(
        "static COUNTER: i32 = 0;",
        &expect![[r#"
            StaticDef@0..24
              STATIC_KW@0..6 "static"
              Name@6..14
                WHITESPACE@6..7 " "
                IDENT@7..14 "COUNTER"
              COLON@14..15 ":"
              PathType@15..19
                Path@15..19
                  PathSegment@15..19
                    NameRef@15..19
                      WHITESPACE@15..16 " "
                      IDENT@16..19 "i32"
              WHITESPACE@19..20 " "
              EQ@20..21 "="
              LiteralExpr@21..23
                WHITESPACE@21..22 " "
                INT_LITERAL@22..23 "0"
              SEMI@23..24 ";"
        "#]],
    );
}

#[test]
fn static_def_mut() {
    check_item(
        "static mut GLOBAL: i32 = 0;",
        &expect![[r#"
            StaticDef@0..27
              STATIC_KW@0..6 "static"
              WHITESPACE@6..7 " "
              MUT_KW@7..10 "mut"
              Name@10..17
                WHITESPACE@10..11 " "
                IDENT@11..17 "GLOBAL"
              COLON@17..18 ":"
              PathType@18..22
                Path@18..22
                  PathSegment@18..22
                    NameRef@18..22
                      WHITESPACE@18..19 " "
                      IDENT@19..22 "i32"
              WHITESPACE@22..23 " "
              EQ@23..24 "="
              LiteralExpr@24..26
                WHITESPACE@24..25 " "
                INT_LITERAL@25..26 "0"
              SEMI@26..27 ";"
        "#]],
    );
}

#[test]
fn static_def_pub_mut() {
    check_item(
        "pub static mut STATE: i64 = -1;",
        &expect![[r#"
            StaticDef@0..31
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              STATIC_KW@4..10 "static"
              WHITESPACE@10..11 " "
              MUT_KW@11..14 "mut"
              Name@14..20
                WHITESPACE@14..15 " "
                IDENT@15..20 "STATE"
              COLON@20..21 ":"
              PathType@21..25
                Path@21..25
                  PathSegment@21..25
                    NameRef@21..25
                      WHITESPACE@21..22 " "
                      IDENT@22..25 "i64"
              WHITESPACE@25..26 " "
              EQ@26..27 "="
              PrefixExpr@27..30
                WHITESPACE@27..28 " "
                MINUS@28..29 "-"
                LiteralExpr@29..30
                  INT_LITERAL@29..30 "1"
              SEMI@30..31 ";"
        "#]],
    );
}

#[test]
fn static_def_with_attribute() {
    check_item(
        "#[no_mangle] pub static HANDLE: i32 = 0;",
        &expect![[r##"
            StaticDef@0..40
              Attribute@0..12
                HASH@0..1 "#"
                L_BRACKET@1..2 "["
                AttrPath@2..11
                  IDENT@2..11 "no_mangle"
                R_BRACKET@11..12 "]"
              Visibility@12..16
                WHITESPACE@12..13 " "
                PUB_KW@13..16 "pub"
              WHITESPACE@16..17 " "
              STATIC_KW@17..23 "static"
              Name@23..30
                WHITESPACE@23..24 " "
                IDENT@24..30 "HANDLE"
              COLON@30..31 ":"
              PathType@31..35
                Path@31..35
                  PathSegment@31..35
                    NameRef@31..35
                      WHITESPACE@31..32 " "
                      IDENT@32..35 "i32"
              WHITESPACE@35..36 " "
              EQ@36..37 "="
              LiteralExpr@37..39
                WHITESPACE@37..38 " "
                INT_LITERAL@38..39 "0"
              SEMI@39..40 ";"
        "##]],
    );
}

#[test]
fn static_def_no_semicolon() {
    check_item(
        "static Y: i32 = 2",
        &expect![[r#"
            StaticDef@0..17
              STATIC_KW@0..6 "static"
              Name@6..8
                WHITESPACE@6..7 " "
                IDENT@7..8 "Y"
              COLON@8..9 ":"
              PathType@9..13
                Path@9..13
                  PathSegment@9..13
                    NameRef@9..13
                      WHITESPACE@9..10 " "
                      IDENT@10..13 "i32"
              WHITESPACE@13..14 " "
              EQ@14..15 "="
              LiteralExpr@15..17
                WHITESPACE@15..16 " "
                INT_LITERAL@16..17 "2"
        "#]],
    );
}

// === Optional Semicolon Tests ===

#[test]
fn use_decl_no_semicolon() {
    check_item(
        "use std.io",
        &expect![[r#"
            UseDecl@0..10
              USE_KW@0..3 "use"
              UseTree@3..10
                WHITESPACE@3..4 " "
                IDENT@4..7 "std"
                DOT@7..8 "."
                IDENT@8..10 "io"
        "#]],
    );
}

#[test]
fn type_alias_no_semicolon() {
    check_item(
        "type Int = i32",
        &expect![[r#"
            TypeAlias@0..14
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
        "#]],
    );
}

#[test]
fn trait_method_decl_no_semicolon() {
    check_item(
        "trait Foo { fn bar() }",
        &expect![[r#"
            TraitDef@0..22
              TRAIT_KW@0..5 "trait"
              Name@5..9
                WHITESPACE@5..6 " "
                IDENT@6..9 "Foo"
              WHITESPACE@9..10 " "
              L_BRACE@10..11 "{"
              TraitItem@11..20
                WHITESPACE@11..12 " "
                FN_KW@12..14 "fn"
                Name@14..18
                  WHITESPACE@14..15 " "
                  IDENT@15..18 "bar"
                ParamList@18..20
                  L_PAREN@18..19 "("
                  R_PAREN@19..20 ")"
              WHITESPACE@20..21 " "
              R_BRACE@21..22 "}"
        "#]],
    );
}

#[test]
fn extern_fn_decl_no_semicolon() {
    check_item(
        "extern \"C\" { fn foo() }",
        &expect![[r#"
            ExternBlock@0..23
              EXTERN_KW@0..6 "extern"
              WHITESPACE@6..7 " "
              STRING_LITERAL@7..10 "\"C\""
              WHITESPACE@10..11 " "
              L_BRACE@11..12 "{"
              ExternFn@12..21
                WHITESPACE@12..13 " "
                FN_KW@13..15 "fn"
                Name@15..19
                  WHITESPACE@15..16 " "
                  IDENT@16..19 "foo"
                ParamList@19..21
                  L_PAREN@19..20 "("
                  R_PAREN@20..21 ")"
              WHITESPACE@21..22 " "
              R_BRACE@22..23 "}"
        "#]],
    );
}

#[test]
fn trait_associated_type_no_semicolon() {
    check_item(
        "trait Iter { type Item }",
        &expect![[r#"
            TraitDef@0..24
              TRAIT_KW@0..5 "trait"
              Name@5..10
                WHITESPACE@5..6 " "
                IDENT@6..10 "Iter"
              WHITESPACE@10..11 " "
              L_BRACE@11..12 "{"
              AssociatedType@12..22
                WHITESPACE@12..13 " "
                TYPE_KW@13..17 "type"
                Name@17..22
                  WHITESPACE@17..18 " "
                  IDENT@18..22 "Item"
              WHITESPACE@22..23 " "
              R_BRACE@23..24 "}"
        "#]],
    );
}

// === const/unsafe function modifiers (spl-hai9) ===

#[test]
fn function_const() {
    check_item(
        "const fn compute(): i32 {}",
        &expect![[r#"
            FunctionDef@0..26
              CONST_KW@0..5 "const"
              WHITESPACE@5..6 " "
              FN_KW@6..8 "fn"
              Name@8..16
                WHITESPACE@8..9 " "
                IDENT@9..16 "compute"
              ParamList@16..18
                L_PAREN@16..17 "("
                R_PAREN@17..18 ")"
              COLON@18..19 ":"
              PathType@19..23
                Path@19..23
                  PathSegment@19..23
                    NameRef@19..23
                      WHITESPACE@19..20 " "
                      IDENT@20..23 "i32"
              Block@23..26
                WHITESPACE@23..24 " "
                L_BRACE@24..25 "{"
                R_BRACE@25..26 "}"
        "#]],
    );
}

#[test]
fn function_unsafe() {
    check_item(
        "unsafe fn danger() {}",
        &expect![[r#"
            FunctionDef@0..21
              UNSAFE_KW@0..6 "unsafe"
              WHITESPACE@6..7 " "
              FN_KW@7..9 "fn"
              Name@9..16
                WHITESPACE@9..10 " "
                IDENT@10..16 "danger"
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
fn function_const_unsafe() {
    check_item(
        "const unsafe fn risky(): i32 {}",
        &expect![[r#"
            FunctionDef@0..31
              CONST_KW@0..5 "const"
              WHITESPACE@5..6 " "
              UNSAFE_KW@6..12 "unsafe"
              WHITESPACE@12..13 " "
              FN_KW@13..15 "fn"
              Name@15..21
                WHITESPACE@15..16 " "
                IDENT@16..21 "risky"
              ParamList@21..23
                L_PAREN@21..22 "("
                R_PAREN@22..23 ")"
              COLON@23..24 ":"
              PathType@24..28
                Path@24..28
                  PathSegment@24..28
                    NameRef@24..28
                      WHITESPACE@24..25 " "
                      IDENT@25..28 "i32"
              Block@28..31
                WHITESPACE@28..29 " "
                L_BRACE@29..30 "{"
                R_BRACE@30..31 "}"
        "#]],
    );
}

#[test]
fn function_pub_const_unsafe() {
    check_item(
        "pub const unsafe fn risky() {}",
        &expect![[r#"
            FunctionDef@0..30
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              CONST_KW@4..9 "const"
              WHITESPACE@9..10 " "
              UNSAFE_KW@10..16 "unsafe"
              WHITESPACE@16..17 " "
              FN_KW@17..19 "fn"
              Name@19..25
                WHITESPACE@19..20 " "
                IDENT@20..25 "risky"
              ParamList@25..27
                L_PAREN@25..26 "("
                R_PAREN@26..27 ")"
              Block@27..30
                WHITESPACE@27..28 " "
                L_BRACE@28..29 "{"
                R_BRACE@29..30 "}"
        "#]],
    );
}

// === throws clause (spl-hai9) ===

#[test]
fn function_throws_untyped() {
    check_item(
        "fn may_fail() throws {}",
        &expect![[r#"
            FunctionDef@0..23
              FN_KW@0..2 "fn"
              Name@2..11
                WHITESPACE@2..3 " "
                IDENT@3..11 "may_fail"
              ParamList@11..13
                L_PAREN@11..12 "("
                R_PAREN@12..13 ")"
              ThrowsClause@13..20
                WHITESPACE@13..14 " "
                THROWS_KW@14..20 "throws"
              Block@20..23
                WHITESPACE@20..21 " "
                L_BRACE@21..22 "{"
                R_BRACE@22..23 "}"
        "#]],
    );
}

#[test]
fn function_throws_typed() {
    check_item(
        "fn may_fail() throws Error {}",
        &expect![[r#"
            FunctionDef@0..29
              FN_KW@0..2 "fn"
              Name@2..11
                WHITESPACE@2..3 " "
                IDENT@3..11 "may_fail"
              ParamList@11..13
                L_PAREN@11..12 "("
                R_PAREN@12..13 ")"
              ThrowsClause@13..26
                WHITESPACE@13..14 " "
                THROWS_KW@14..20 "throws"
                PathType@20..26
                  Path@20..26
                    PathSegment@20..26
                      NameRef@20..26
                        WHITESPACE@20..21 " "
                        IDENT@21..26 "Error"
              Block@26..29
                WHITESPACE@26..27 " "
                L_BRACE@27..28 "{"
                R_BRACE@28..29 "}"
        "#]],
    );
}

#[test]
fn function_return_and_throws() {
    check_item(
        "fn compute(): i32 throws Error {}",
        &expect![[r#"
            FunctionDef@0..33
              FN_KW@0..2 "fn"
              Name@2..10
                WHITESPACE@2..3 " "
                IDENT@3..10 "compute"
              ParamList@10..12
                L_PAREN@10..11 "("
                R_PAREN@11..12 ")"
              COLON@12..13 ":"
              PathType@13..17
                Path@13..17
                  PathSegment@13..17
                    NameRef@13..17
                      WHITESPACE@13..14 " "
                      IDENT@14..17 "i32"
              ThrowsClause@17..30
                WHITESPACE@17..18 " "
                THROWS_KW@18..24 "throws"
                PathType@24..30
                  Path@24..30
                    PathSegment@24..30
                      NameRef@24..30
                        WHITESPACE@24..25 " "
                        IDENT@25..30 "Error"
              Block@30..33
                WHITESPACE@30..31 " "
                L_BRACE@31..32 "{"
                R_BRACE@32..33 "}"
        "#]],
    );
}

#[test]
fn function_throws_and_where() {
    check_item(
        "fn generic(x: T) throws where T {}",
        &expect![[r#"
            FunctionDef@0..34
              FN_KW@0..2 "fn"
              Name@2..10
                WHITESPACE@2..3 " "
                IDENT@3..10 "generic"
              ParamList@10..16
                L_PAREN@10..11 "("
                Param@11..15
                  Name@11..12
                    IDENT@11..12 "x"
                  COLON@12..13 ":"
                  PathType@13..15
                    Path@13..15
                      PathSegment@13..15
                        NameRef@13..15
                          WHITESPACE@13..14 " "
                          IDENT@14..15 "T"
                R_PAREN@15..16 ")"
              ThrowsClause@16..23
                WHITESPACE@16..17 " "
                THROWS_KW@17..23 "throws"
              WhereClause@23..31
                WHITESPACE@23..24 " "
                WHERE_KW@24..29 "where"
                GenericParam@29..31
                  Name@29..31
                    WHITESPACE@29..30 " "
                    IDENT@30..31 "T"
              Block@31..34
                WHITESPACE@31..32 " "
                L_BRACE@32..33 "{"
                R_BRACE@33..34 "}"
        "#]],
    );
}

#[test]
fn function_full_signature() {
    check_item(
        "pub const unsafe fn process(x: T): Result throws Error where T {}",
        &expect![[r#"
            FunctionDef@0..65
              Visibility@0..3
                PUB_KW@0..3 "pub"
              WHITESPACE@3..4 " "
              CONST_KW@4..9 "const"
              WHITESPACE@9..10 " "
              UNSAFE_KW@10..16 "unsafe"
              WHITESPACE@16..17 " "
              FN_KW@17..19 "fn"
              Name@19..27
                WHITESPACE@19..20 " "
                IDENT@20..27 "process"
              ParamList@27..33
                L_PAREN@27..28 "("
                Param@28..32
                  Name@28..29
                    IDENT@28..29 "x"
                  COLON@29..30 ":"
                  PathType@30..32
                    Path@30..32
                      PathSegment@30..32
                        NameRef@30..32
                          WHITESPACE@30..31 " "
                          IDENT@31..32 "T"
                R_PAREN@32..33 ")"
              COLON@33..34 ":"
              PathType@34..41
                Path@34..41
                  PathSegment@34..41
                    NameRef@34..41
                      WHITESPACE@34..35 " "
                      IDENT@35..41 "Result"
              ThrowsClause@41..54
                WHITESPACE@41..42 " "
                THROWS_KW@42..48 "throws"
                PathType@48..54
                  Path@48..54
                    PathSegment@48..54
                      NameRef@48..54
                        WHITESPACE@48..49 " "
                        IDENT@49..54 "Error"
              WhereClause@54..62
                WHITESPACE@54..55 " "
                WHERE_KW@55..60 "where"
                GenericParam@60..62
                  Name@60..62
                    WHITESPACE@60..61 " "
                    IDENT@61..62 "T"
              Block@62..65
                WHITESPACE@62..63 " "
                L_BRACE@63..64 "{"
                R_BRACE@64..65 "}"
        "#]],
    );
}

// === Default parameters (spl-hai9) ===

#[test]
fn param_with_default() {
    check_item(
        r#"fn greet(name: String = "World") {}"#,
        &expect![[r#"
            FunctionDef@0..35
              FN_KW@0..2 "fn"
              Name@2..8
                WHITESPACE@2..3 " "
                IDENT@3..8 "greet"
              ParamList@8..32
                L_PAREN@8..9 "("
                Param@9..31
                  Name@9..13
                    IDENT@9..13 "name"
                  COLON@13..14 ":"
                  PathType@14..21
                    Path@14..21
                      PathSegment@14..21
                        NameRef@14..21
                          WHITESPACE@14..15 " "
                          IDENT@15..21 "String"
                  WHITESPACE@21..22 " "
                  EQ@22..23 "="
                  LiteralExpr@23..31
                    WHITESPACE@23..24 " "
                    STRING_LITERAL@24..31 "\"World\""
                R_PAREN@31..32 ")"
              Block@32..35
                WHITESPACE@32..33 " "
                L_BRACE@33..34 "{"
                R_BRACE@34..35 "}"
        "#]],
    );
}

#[test]
fn param_with_numeric_default() {
    check_item(
        "fn count(n: i32 = 10) {}",
        &expect![[r#"
            FunctionDef@0..24
              FN_KW@0..2 "fn"
              Name@2..8
                WHITESPACE@2..3 " "
                IDENT@3..8 "count"
              ParamList@8..21
                L_PAREN@8..9 "("
                Param@9..20
                  Name@9..10
                    IDENT@9..10 "n"
                  COLON@10..11 ":"
                  PathType@11..15
                    Path@11..15
                      PathSegment@11..15
                        NameRef@11..15
                          WHITESPACE@11..12 " "
                          IDENT@12..15 "i32"
                  WHITESPACE@15..16 " "
                  EQ@16..17 "="
                  LiteralExpr@17..20
                    WHITESPACE@17..18 " "
                    INT_LITERAL@18..20 "10"
                R_PAREN@20..21 ")"
              Block@21..24
                WHITESPACE@21..22 " "
                L_BRACE@22..23 "{"
                R_BRACE@23..24 "}"
        "#]],
    );
}

#[test]
fn param_with_expression_default() {
    check_item(
        "fn compute(x: i32 = 2 + 3) {}",
        &expect![[r#"
            FunctionDef@0..29
              FN_KW@0..2 "fn"
              Name@2..10
                WHITESPACE@2..3 " "
                IDENT@3..10 "compute"
              ParamList@10..26
                L_PAREN@10..11 "("
                Param@11..25
                  Name@11..12
                    IDENT@11..12 "x"
                  COLON@12..13 ":"
                  PathType@13..17
                    Path@13..17
                      PathSegment@13..17
                        NameRef@13..17
                          WHITESPACE@13..14 " "
                          IDENT@14..17 "i32"
                  WHITESPACE@17..18 " "
                  EQ@18..19 "="
                  BinExpr@19..25
                    LiteralExpr@19..21
                      WHITESPACE@19..20 " "
                      INT_LITERAL@20..21 "2"
                    WHITESPACE@21..22 " "
                    PLUS@22..23 "+"
                    LiteralExpr@23..25
                      WHITESPACE@23..24 " "
                      INT_LITERAL@24..25 "3"
                R_PAREN@25..26 ")"
              Block@26..29
                WHITESPACE@26..27 " "
                L_BRACE@27..28 "{"
                R_BRACE@28..29 "}"
        "#]],
    );
}

#[test]
fn multiple_params_some_with_defaults() {
    check_item(
        "fn foo(required: i32, optional: i32 = 42) {}",
        &expect![[r#"
            FunctionDef@0..44
              FN_KW@0..2 "fn"
              Name@2..6
                WHITESPACE@2..3 " "
                IDENT@3..6 "foo"
              ParamList@6..41
                L_PAREN@6..7 "("
                Param@7..20
                  Name@7..15
                    IDENT@7..15 "required"
                  COLON@15..16 ":"
                  PathType@16..20
                    Path@16..20
                      PathSegment@16..20
                        NameRef@16..20
                          WHITESPACE@16..17 " "
                          IDENT@17..20 "i32"
                COMMA@20..21 ","
                Param@21..40
                  Name@21..30
                    WHITESPACE@21..22 " "
                    IDENT@22..30 "optional"
                  COLON@30..31 ":"
                  PathType@31..35
                    Path@31..35
                      PathSegment@31..35
                        NameRef@31..35
                          WHITESPACE@31..32 " "
                          IDENT@32..35 "i32"
                  WHITESPACE@35..36 " "
                  EQ@36..37 "="
                  LiteralExpr@37..40
                    WHITESPACE@37..38 " "
                    INT_LITERAL@38..40 "42"
                R_PAREN@40..41 ")"
              Block@41..44
                WHITESPACE@41..42 " "
                L_BRACE@42..43 "{"
                R_BRACE@43..44 "}"
        "#]],
    );
}

#[test]
fn param_with_label_and_default() {
    check_item(
        r#"fn greet(to name: String = "World") {}"#,
        &expect![[r#"
            FunctionDef@0..38
              FN_KW@0..2 "fn"
              Name@2..8
                WHITESPACE@2..3 " "
                IDENT@3..8 "greet"
              ParamList@8..35
                L_PAREN@8..9 "("
                Param@9..34
                  LabelSpec@9..11
                    IDENT@9..11 "to"
                  Name@11..16
                    WHITESPACE@11..12 " "
                    IDENT@12..16 "name"
                  COLON@16..17 ":"
                  PathType@17..24
                    Path@17..24
                      PathSegment@17..24
                        NameRef@17..24
                          WHITESPACE@17..18 " "
                          IDENT@18..24 "String"
                  WHITESPACE@24..25 " "
                  EQ@25..26 "="
                  LiteralExpr@26..34
                    WHITESPACE@26..27 " "
                    STRING_LITERAL@27..34 "\"World\""
                R_PAREN@34..35 ")"
              Block@35..38
                WHITESPACE@35..36 " "
                L_BRACE@36..37 "{"
                R_BRACE@37..38 "}"
        "#]],
    );
}
