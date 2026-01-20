# SPL Syntax Grammar

This document defines the syntax grammar of SPL using Extended Backus-Naur Form (EBNF). It builds on the lexical grammar defined in `lexical-grammar.md`.

## EBNF Notation

| Notation       | Meaning                                      |
|----------------|----------------------------------------------|
| `=`            | Definition                                   |
| `\|`           | Alternation (choice)                         |
| `( ... )`      | Grouping                                     |
| `[ ... ]`      | Optional (zero or one)                       |
| `{ ... }`      | Repetition (zero or more)                    |
| `"text"`       | Terminal literal                             |
| `UPPERCASE`    | Terminal token from lexer                    |
| `PascalCase`   | Non-terminal (grammar rule)                  |

Trailing commas are allowed in all comma-separated lists (Rust-style).

---

## 1. Program Structure

```ebnf
Program = { Item } ;

Item = [ "pub" ] ( FunctionDef | StructDef | ImplBlock | TypeAlias ) ;
```

### Function Definitions

```ebnf
FunctionDef = "fn" IDENTIFIER [ GenericParams ] "(" [ ParamList ] ")" [ "->" Type ] Block ;

ParamList = Param { "," Param } [ "," ] ;

Param = SelfParam | TypedParam ;

SelfParam = [ "&" [ "mut" ] ] "self" ;

TypedParam = [ "mut" ] IDENTIFIER ":" Type ;

GenericParams = "<" GenericParam { "," GenericParam } [ "," ] ">" ;

GenericParam = IDENTIFIER ;
```

### Struct Definitions

```ebnf
StructDef = "struct" IDENTIFIER [ GenericParams ] "{" [ FieldList ] "}" ;

FieldList = Field { "," Field } [ "," ] ;

Field = [ "pub" ] IDENTIFIER ":" Type ;
```

### Implementation Blocks

```ebnf
ImplBlock = "impl" [ GenericParams ] TypePath [ GenericArgs ] "{" { ImplItem } "}" ;

ImplItem = [ "pub" ] FunctionDef ;
```

### Type Aliases

```ebnf
TypeAlias = "type" IDENTIFIER [ GenericParams ] "=" Type ";" ;
```

---

## 2. Types

```ebnf
Type = ReferenceType
     | ArrayType
     | TupleType
     | FnPointerType
     | PathType ;

ReferenceType = "&" [ "mut" ] Type ;

ArrayType = "[" Type [ ";" Expression ] "]" ;

TupleType = "(" [ Type { "," Type } [ "," ] ] ")" ;

FnPointerType = "fn" "(" [ TypeList ] ")" [ "->" Type ] ;

TypeList = Type { "," Type } [ "," ] ;

PathType = TypePath [ GenericArgs ]
         | "Self" ;

TypePath = IDENTIFIER { "::" IDENTIFIER } ;

GenericArgs = "<" Type { "," Type } [ "," ] ">" ;
```

### Type Examples

| Syntax              | Description                        |
|---------------------|------------------------------------|
| `i32`               | Simple type                        |
| `Point<T>`          | Generic type                       |
| `std::vec::Vec`     | Qualified path type                |
| `Self`              | Self type (in impl blocks)         |
| `&T`                | Immutable reference                |
| `&mut T`            | Mutable reference                  |
| `[T]`               | Slice type                         |
| `[T; 10]`           | Fixed-size array                   |
| `(T, U)`            | Tuple type                         |
| `()`                | Unit type                          |
| `fn(i32) -> bool`   | Function pointer                   |
| `fn(T, U) -> V`     | Generic function pointer           |
| `fn()`              | Function pointer returning unit    |

---

## 3. Statements

```ebnf
Block = "{" { Statement } [ Expression ] "}" ;

Statement = LetStatement
          | ExpressionStatement ;

LetStatement = "let" [ "mut" ] Pattern [ ":" Type ] [ "=" Expression ] ";" ;

ExpressionStatement = Expression ";"
                    | BlockExpression [ ";" ] ;
```

Block expressions (`if`, `while`, `for`, `loop`, and bare blocks) may omit the trailing semicolon when used as statements. The optional trailing expression in a `Block` becomes the block's value.

---

## 4. Expressions

Expressions are defined using layered production rules that encode operator precedence. Lower precedence operators are defined first; they call higher precedence rules.

### Precedence Table

| Precedence | Operators                    | Associativity | Production         |
|------------|------------------------------|---------------|--------------------|
| 1 (lowest) | `=` `+=` `-=` `*=` `/=` `%=` | Right         | AssignmentExpr     |
| 2          | `\|\|`                       | Left          | OrExpr             |
| 3          | `&&`                         | Left          | AndExpr            |
| 4          | `==` `!=`                    | Left          | EqualityExpr       |
| 5          | `<` `>` `<=` `>=`            | Left          | ComparisonExpr     |
| 6          | `..`                         | Left          | RangeExpr          |
| 7          | `+` `-`                      | Left          | AdditiveExpr       |
| 8          | `*` `/` `%`                  | Left          | MultiplicativeExpr |
| 9          | `as`                         | Left          | CastExpr           |
| 10         | `!` `-` `&` (unary)          | Right         | UnaryExpr          |
| 11 (highest)| `.` `::` `()` `[]` `[:]`    | Left          | PostfixExpr        |

### Expression Grammar

```ebnf
Expression = AssignmentExpr ;

AssignmentExpr = OrExpr [ AssignOp AssignmentExpr ] ;

AssignOp = "=" | "+=" | "-=" | "*=" | "/=" | "%=" ;

OrExpr = AndExpr { "||" AndExpr } ;

AndExpr = EqualityExpr { "&&" EqualityExpr } ;

EqualityExpr = ComparisonExpr { ( "==" | "!=" ) ComparisonExpr } ;

ComparisonExpr = RangeExpr { ( "<" | ">" | "<=" | ">=" ) RangeExpr } ;

RangeExpr = AdditiveExpr [ ".." [ AdditiveExpr ] ] ;

AdditiveExpr = MultiplicativeExpr { ( "+" | "-" ) MultiplicativeExpr } ;

MultiplicativeExpr = CastExpr { ( "*" | "/" | "%" ) CastExpr } ;

CastExpr = UnaryExpr { "as" Type } ;

UnaryExpr = ( "!" | "-" | "&" [ "mut" ] ) UnaryExpr
          | PostfixExpr ;

PostfixExpr = PrimaryExpr { PostfixOp } ;

PostfixOp = "." IDENTIFIER
          | "." IDENTIFIER "(" [ ArgList ] ")"
          | "::" IDENTIFIER
          | "::" IDENTIFIER "(" [ ArgList ] ")"
          | "(" [ ArgList ] ")"
          | "[" Expression "]"
          | "[" SliceExpr "]" ;

SliceExpr = [ Expression ] ":" [ Expression | "$" ] ;
```

### Primary Expressions

```ebnf
PrimaryExpr = LiteralExpr
            | PathExpr
            | GroupedExpr
            | TupleExpr
            | ArrayExpr
            | StructExpr
            | BlockExpression
            | IfExpr
            | WhileExpr
            | ForExpr
            | LoopExpr
            | BreakExpr
            | ContinueExpr
            | ReturnExpr ;

LiteralExpr = INTEGER | FLOAT | STRING | CHAR | "true" | "false" ;

PathExpr = IDENTIFIER { "::" IDENTIFIER } ;

GroupedExpr = "(" Expression ")" ;

TupleExpr = "(" [ Expression "," [ Expression { "," Expression } ] [ "," ] ] ")" ;

ArrayExpr = "[" [ Expression { "," Expression } [ "," ] ] "]"
          | "[" Expression ";" Expression "]" ;

StructExpr = StructExprPath "{" [ StructFieldList ] "}" ;

StructExprPath = TypePath [ GenericArgs ]
               | "Self" ;

StructFieldList = StructField { "," StructField } [ "," ] ;

StructField = IDENTIFIER [ ":" Expression ] ;

ArgList = Expression { "," Expression } [ "," ] ;
```

### Control Flow Expressions

```ebnf
BlockExpression = Block
                | IfExpr
                | WhileExpr
                | ForExpr
                | LoopExpr ;

IfExpr = "if" Expression Block [ "else" ( IfExpr | Block ) ] ;

WhileExpr = "while" Expression Block ;

ForExpr = "for" Pattern "in" Expression Block ;

LoopExpr = "loop" Block ;

BreakExpr = "break" [ Expression ] ;

ContinueExpr = "continue" ;

ReturnExpr = "return" [ Expression ] ;
```

---

## 5. Patterns

Patterns are used in `let` bindings, `for` loops, and (potentially) match arms.

```ebnf
Pattern = IdentifierPattern
        | WildcardPattern
        | LiteralPattern
        | RangePattern
        | TuplePattern
        | SlicePattern
        | StructPattern
        | ReferencePattern ;

IdentifierPattern = [ "mut" ] IDENTIFIER ;

WildcardPattern = "_" ;

LiteralPattern = [ "-" ] INTEGER
               | [ "-" ] FLOAT
               | STRING
               | CHAR
               | "true"
               | "false" ;

RangePattern = LiteralPattern ".." LiteralPattern ;

TuplePattern = "(" [ Pattern { "," Pattern } [ "," ] ] ")" ;

SlicePattern = "[" [ SlicePatternElement { "," SlicePatternElement } [ "," ] ] "]" ;

SlicePatternElement = RestPattern | Pattern ;

RestPattern = ".." [ IDENTIFIER ] ;

StructPattern = TypePath "{" [ StructPatternFields ] "}" ;

StructPatternFields = StructPatternField { "," StructPatternField } [ "," ] [ ".." ] ;

StructPatternField = IDENTIFIER [ ":" Pattern ] ;

ReferencePattern = "&" [ "mut" ] Pattern ;
```

**Note:** At most one `RestPattern` (`..` or `..name`) is allowed per slice pattern. This is enforced semantically, not syntactically.

### Pattern Examples

| Syntax                | Description                          |
|-----------------------|--------------------------------------|
| `x`                   | Bind to identifier                   |
| `mut x`               | Mutable binding                      |
| `_`                   | Wildcard (ignore value)              |
| `42`                  | Match literal integer                |
| `-1`                  | Match negative literal               |
| `0..10`               | Match range (inclusive start, exclusive end) |
| `'a'..'z'`            | Match character range                |
| `(a, b)`              | Destructure tuple                    |
| `[a, b, c]`           | Destructure fixed-size array/slice   |
| `[first, ..]`         | Match first, ignore rest             |
| `[first, ..rest]`     | Match first, bind rest to `rest`     |
| `[.., last]`          | Match last element                   |
| `[first, ..middle, last]` | Match first, last, bind middle   |
| `Point { x, y }`      | Destructure struct (shorthand)       |
| `Point { x: a, y: b }`| Destructure with rename              |
| `Point { x, .. }`     | Partial struct destructure           |
| `&x`                  | Match reference                      |
| `&mut x`              | Match mutable reference              |

---

## 6. Literals

Terminal tokens from the lexer. See `lexical-grammar.md` for precise definitions.

```ebnf
INTEGER = (* decimal, hex, binary, or octal integer *) ;

FLOAT = (* floating-point number *) ;

STRING = (* double-quoted string with escapes *) ;

CHAR = (* single-quoted character *) ;
```

Boolean literals use the keywords `true` and `false`.

---

## Operator Precedence Summary

From lowest to highest precedence:

| Prec | Category       | Operators                       | Assoc | Example               |
|------|----------------|--------------------------------|-------|-----------------------|
| 1    | Assignment     | `=` `+=` `-=` `*=` `/=` `%=`   | Right | `x = y = 1`           |
| 2    | Logical OR     | `\|\|`                         | Left  | `a \|\| b \|\| c`     |
| 3    | Logical AND    | `&&`                           | Left  | `a && b && c`         |
| 4    | Equality       | `==` `!=`                      | Left  | `a == b != c`         |
| 5    | Comparison     | `<` `>` `<=` `>=`              | Left  | `a < b`               |
| 6    | Range          | `..`                           | Left  | `0..10`               |
| 7    | Additive       | `+` `-`                        | Left  | `a + b - c`           |
| 8    | Multiplicative | `*` `/` `%`                    | Left  | `a * b / c`           |
| 9    | Cast           | `as`                           | Left  | `x as i32 as f64`     |
| 10   | Unary          | `!` `-` `&` `&mut`             | Right | `!&mut x`             |
| 11   | Postfix        | `.` `::` `()` `[]` `[:]`       | Left  | `a.b().c[0]`          |

---

## Ambiguity Resolution

### 1. Struct Expression vs Block

When the parser sees `IDENTIFIER {`, it must determine if this is a struct instantiation or a block statement.

**Rule:** If a type path (identifiers separated by `::`) is followed by `{`, it is a struct expression. A bare `{` begins a block.

```spl
Point { x: 1, y: 2 }   // Struct expression
{ let x = 1; x }       // Block expression
```

### 2. Generic Arguments vs Comparison

The `<` token could begin generic arguments or be a comparison operator.

**Rule:** In type position, `<` after an identifier opens generic arguments. In expression position, `<` is the less-than operator.

```spl
let v: Vec<i32> = ...;    // Generic arguments
let cmp = a < b;          // Comparison
```

For turbofish syntax in expressions, use `::`:

```spl
Vec::<i32>::new()         // Turbofish disambiguates
```

### 3. Range vs Path Separator

The `..` and `::` tokens are distinct.

**Rule:** `..` is always the range operator. `::` is always the path separator.

```spl
0..10            // Range from 0 to 10
std::vec::Vec    // Path
```

### 4. Tuple vs Grouped Expression

A parenthesized expression could be a tuple or a grouped expression.

**Rule:** A single expression in parentheses without a trailing comma is a grouped expression. With a trailing comma (or multiple elements), it is a tuple.

```spl
(1 + 2)          // Grouped expression, evaluates to 3
(1,)             // Single-element tuple
(1, 2)           // Two-element tuple
()               // Unit (empty tuple)
```

### 5. Struct Field Shorthand

When a struct field name matches a variable name, the value can be omitted.

```spl
let x = 1;
let y = 2;
Point { x, y }           // Equivalent to Point { x: x, y: y }
Point { x, y: y + 1 }    // Mixed shorthand and explicit
```

### 6. Index vs Slice

A bracketed expression could be an index or a slice.

**Rule:** If `:` appears at the top level inside brackets, it is a slice expression. Otherwise, it is an index expression.

```spl
arr[0]           // Index: element at position 0
arr[i + 1]       // Index: element at computed position
arr[1:3]         // Slice: elements 1, 2
arr[:3]          // Slice: elements 0, 1, 2
arr[1:]          // Slice: from index 1 to end
arr[1:$]         // Slice: from index 1 to end (explicit $)
arr[:]           // Slice: full copy
```

The `$` symbol represents the end of the array and is only valid in slice expressions.

---

## Complete Example

The following program demonstrates key grammar constructs:

```spl
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl<T> Point<T> {
    pub fn new(x: T, y: T) -> Point<T> {
        Point { x, y }
    }

    pub fn swap(&mut self) {
        let temp = self.x;
        self.x = self.y;
        self.y = temp;
    }
}

type Pair<T> = (T, T);

fn distance(p1: &Point<f64>, p2: &Point<f64>) -> f64 {
    let dx = p1.x - p2.x;
    let dy = p1.y - p2.y;
    (dx * dx + dy * dy).sqrt()
}

fn main() {
    let mut origin = Point::new(0.0, 0.0);
    let target = Point { x: 3.0, y: 4.0 };

    let dist = distance(&origin, &target);

    // Control flow
    if dist > 5.0 {
        return;
    }

    // Loops
    for i in 0..10 {
        if i % 2 == 0 {
            continue;
        }
        // Process odd numbers
    }

    let mut count = 0;
    while count < 3 {
        count += 1;
    }

    loop {
        if count >= 10 {
            break;
        }
        count += 1;
    }

    // Expressions and operators
    let value = 10 + 5 * 2;              // 20 (multiplicative binds tighter)
    let cast = 65 as f64;                // Type cast
    let reference = &mut origin;         // Mutable reference
    let indexed = [1, 2, 3][0];          // Array indexing
    let range = 0..100;                  // Range

    // Slicing
    let arr = [1, 2, 3, 4, 5];
    let slice1 = arr[1:3];               // [2, 3]
    let slice2 = arr[:3];                // [1, 2, 3]
    let slice3 = arr[2:];                // [3, 4, 5]
    let slice4 = arr[2:$];               // [3, 4, 5] (explicit end)
    let copy = arr[:];                   // full copy

    // Patterns
    let (a, b) = (1, 2);                 // Tuple destructuring
    let Point { x, y } = target;         // Struct destructuring
    let [first, ..rest] = [1, 2, 3, 4];  // Slice pattern with rest
    let [head, .., tail] = [1, 2, 3];    // First and last
}

// Function pointer types
type Predicate = fn(i32) -> bool;
type BinaryOp = fn(i32, i32) -> i32;
type Action = fn();

fn apply(f: fn(i32) -> i32, x: i32) -> i32 {
    f(x)
}

// Self type in impl blocks
impl<T> Point<T> {
    fn origin() -> Self {
        Self { x: 0, y: 0 }
    }

    fn clone(&self) -> Self {
        Self { x: self.x, y: self.y }
    }
}
```

---

## Grammar Summary

| Category    | Key Productions                                                 |
|-------------|-----------------------------------------------------------------|
| Program     | `Program`, `Item`, `FunctionDef`, `StructDef`                   |
| Types       | `Type`, `ReferenceType`, `ArrayType`, `FnPointerType`, `Self`   |
| Statements  | `Block`, `Statement`, `LetStatement`                            |
| Expressions | `Expression`, `PrimaryExpr`, `IfExpr`, `LoopExpr`               |
| Patterns    | `Pattern`, `RangePattern`, `SlicePattern`, `StructPattern`      |
| Literals    | `INTEGER`, `FLOAT`, `STRING`, `CHAR`, `true`, `false`           |
