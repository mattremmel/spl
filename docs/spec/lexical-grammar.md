# SPL Lexical Grammar

This document defines the lexical structure of SPL (Simple Programming Language) - a multi-paradigm, statically-typed language with a clean, modern syntax.

## Overview

The lexer transforms source text into a stream of tokens. Whitespace and comments are skipped (not emitted as tokens). The lexer is greedy, always consuming the longest possible match.

---

## Keywords

SPL reserves 38 keywords that cannot be used as identifiers:

| Keyword    | Description                          |
|------------|--------------------------------------|
| `let`      | Variable binding                     |
| `mut`      | Mutable binding modifier             |
| `fn`       | Function declaration                 |
| `struct`   | Struct type declaration              |
| `enum`     | Enum type declaration                |
| `trait`    | Trait declaration                    |
| `type`     | Type alias declaration               |
| `impl`     | Implementation block                 |
| `if`       | Conditional branch                   |
| `else`     | Alternative branch                   |
| `while`    | While loop                           |
| `for`      | For loop                             |
| `in`       | Iterator/range keyword               |
| `loop`     | Infinite loop                        |
| `break`    | Exit loop                            |
| `continue` | Skip to next iteration               |
| `return`   | Return from function                 |
| `yield`    | Block expression value               |
| `as`       | Import renaming                      |
| `true`     | Boolean literal true                 |
| `false`    | Boolean literal false                |
| `pub`      | Public visibility modifier           |
| `package`  | Package scope in `pub(package)`      |
| `self`     | Self value in methods                |
| `Self`     | Self type in impl blocks             |
| `use`      | Import declaration                   |
| `module`   | Module declaration (inline or ref)   |
| `super`    | Parent module reference              |
| `where`    | Generic type constraints             |
| `is`       | Pattern matching operator            |
| `not`      | Negation in `is not` pattern         |
| `match`    | Match expression                     |
| `extern`   | External function declaration        |
| `const`    | Compile-time constant                |
| `static`   | Static variable                      |
| `unsafe`   | Unsafe block/function                |
| `async`    | Async function declaration           |
| `await`    | Await async expression               |

---

## Operators

### Arithmetic Operators

| Operator | Description    |
|----------|----------------|
| `+`      | Addition       |
| `-`      | Subtraction    |
| `*`      | Multiplication |
| `/`      | Division       |
| `%`      | Modulo         |

### Comparison Operators

| Operator | Description              |
|----------|--------------------------|
| `==`     | Equal                    |
| `!=`     | Not equal                |
| `<`      | Less than                |
| `>`      | Greater than             |
| `<=`     | Less than or equal       |
| `>=`     | Greater than or equal    |

### Logical Operators

| Operator | Description  |
|----------|--------------|
| `&&`     | Logical AND  |
| `\|\|`   | Logical OR   |
| `!`      | Logical NOT  |

### Assignment Operators

| Operator | Description              |
|----------|--------------------------|
| `=`      | Assignment               |
| `+=`     | Add and assign           |
| `-=`     | Subtract and assign      |
| `*=`     | Multiply and assign      |
| `/=`     | Divide and assign        |
| `%=`     | Modulo and assign        |

### Other Operators

| Operator | Description                        |
|----------|-----------------------------------|
| `.`      | Member access / path separator    |
| `&`      | Reference                         |
| `..`     | Range                             |
| `$`      | Package root (paths) / array end (slices) |
| `?`      | Error propagation (Try operator)  |

**Note:** Return types use `:` (colon) instead of `->`. Paths use `.` (dot) as the only separator (no `::`). Type application uses parentheses: `Vec(i32)` instead of `Vec<i32>`. Package-root paths use `$`: `$.utils.helper`. The `as` keyword is used only for import renaming, not type casting (use methods like `.widen()`, `.truncate()` for conversions).

### Operator Precedence (highest to lowest)

| Precedence | Operators                    | Associativity |
|------------|------------------------------|---------------|
| 1          | `.` `()` `[]` `?`            | Left          |
| 2          | `!` `-` (unary) `&`          | Right         |
| 3          | `*` `/` `%`                  | Left          |
| 4          | `+` `-`                      | Left          |
| 5          | `..`                         | Left          |
| 6          | `<` `>` `<=` `>=`            | Left          |
| 7          | `==` `!=`                    | Left          |
| 8          | `is` `is not`                | Left          |
| 9          | `&&`                         | Left          |
| 10         | `\|\|`                       | Left          |
| 11         | `=` `+=` `-=` `*=` `/=` `%=` | Right         |

Note: `as` is not in the precedence table as it is not used for type casting. Type conversions use methods like `.widen()`, `.truncate()`, `.saturate()`.

---

## Delimiters

| Token | Description          |
|-------|----------------------|
| `(`   | Left parenthesis     |
| `)`   | Right parenthesis    |
| `{`   | Left brace           |
| `}`   | Right brace          |
| `[`   | Left bracket         |
| `]`   | Right bracket        |
| `;`   | Semicolon            |
| `:`   | Colon                |
| `,`   | Comma                |

---

## Literals

### Integer Literals

Integers can be written in decimal, hexadecimal, binary, or octal:

| Format      | Pattern                     | Example       |
|-------------|-----------------------------|---------------|
| Decimal     | `[0-9][0-9_]*`              | `42`, `1_000` |
| Hexadecimal | `0x[0-9a-fA-F][0-9a-fA-F_]*`| `0x2A`, `0xFF`|
| Binary      | `0b[01][01_]*`              | `0b101010`    |
| Octal       | `0o[0-7][0-7_]*`            | `0o52`        |

**Rules:**
- Underscores (`_`) may appear between digits for readability
- Underscores cannot appear at the start of a number
- An underscore may appear before a type suffix (e.g., `42_i64`)
- Leading zeros in decimal literals are allowed (e.g., `007`)

**Type Suffixes:**
Integer literals may have a type suffix: `i8`, `i16`, `i32`, `i64`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128`, `isize`, `usize`.

```
42i64       // i64
255_u8      // u8 with underscore separator
0xFF_u32    // u32 hex literal
```

**Regex:**
```
INTEGER = 0x[0-9a-fA-F][0-9a-fA-F_]*
        | 0b[01][01_]*
        | 0o[0-7][0-7_]*
        | [0-9][0-9_]*
```

### Floating-Point Literals

| Format       | Pattern                              | Example         |
|--------------|--------------------------------------|-----------------|
| Basic        | `[0-9]+\.[0-9]+`                     | `3.14`          |
| Exponent     | `[0-9]+e[+-]?[0-9]+`                 | `1e10`, `2e-3`  |
| Full         | `[0-9]+\.[0-9]+e[+-]?[0-9]+`         | `2.5e-3`        |

**Rules:**
- Must have digits on both sides of the decimal point (`.5` and `5.` are invalid)
- Exponent indicator is lowercase `e`
- Underscores allowed between digits: `1_000.000_001`
- Type suffixes: `f32`, `f64` (e.g., `3.14_f32`, `2.718f64`)

**Regex:**
```
FLOAT = [0-9][0-9_]*\.[0-9][0-9_]*(e[+-]?[0-9][0-9_]*)?
      | [0-9][0-9_]*e[+-]?[0-9][0-9_]*
```

### String Literals

Strings are enclosed in double quotes and support escape sequences:

```
STRING = "[^"\\]*(\\.[^"\\]*)*"
```

**Escape Sequences:**

| Escape | Character            |
|--------|----------------------|
| `\n`   | Newline              |
| `\t`   | Tab                  |
| `\r`   | Carriage return      |
| `\\`   | Backslash            |
| `\"`   | Double quote         |
| `\0`   | Null character       |

**Examples:** `"hello"`, `"hello\nworld"`, `"say \"hi\""`

### Character Literals

Single characters enclosed in single quotes:

```
CHAR = '[^'\\]' | '\\[ntr\\'0]'
```

**Examples:** `'a'`, `'\n'`, `'\\'`, `'\0'`

### Boolean Literals

Boolean values use the keywords `true` and `false`.

---

## Identifiers

Identifiers name variables, functions, types, and other entities.

**Rules:**
- Must start with a letter (`a-z`, `A-Z`) or underscore (`_`)
- May contain letters, digits (`0-9`), and underscores
- Case-sensitive (`foo` and `Foo` are different)
- Cannot be a keyword

**Regex:**
```
IDENTIFIER = [a-zA-Z_][a-zA-Z0-9_]*
```

**Examples:** `x`, `foo_bar`, `Point2D`, `_private`, `__internal`

---

## Comments

### Line Comments

Begin with `//` and extend to the end of the line:

```
// This is a line comment
let x = 42; // inline comment
```

### Block Comments

Begin with `/*` and end with `*/`. Block comments **do nest** (like Rust):

```
/* This is a
   block comment */

/* Outer /* inner */ still in outer comment */
```

Nesting allows commenting out code that contains comments.

---

## Whitespace

The following characters are whitespace and serve only to separate tokens:

| Character | Description     |
|-----------|-----------------|
| ` `       | Space (U+0020)  |
| `\t`      | Tab (U+0009)    |
| `\n`      | Newline (U+000A)|
| `\r`      | Carriage return (U+000D) |

Whitespace is not significant except to separate tokens that would otherwise merge (e.g., `letx` vs `let x`).

---

## Token Categories Summary

| Category    | Examples                                    |
|-------------|---------------------------------------------|
| Keyword     | `let`, `fn`, `if`, `struct`, `where`, `is`  |
| Identifier  | `foo`, `Point2D`, `_value`                  |
| Integer     | `42`, `0xFF`, `0b1010`, `1_000_000`         |
| Float       | `3.14`, `1e10`, `2.5e-3`                    |
| String      | `"hello"`, `"line\nbreak"`                  |
| Char        | `'a'`, `'\n'`                               |
| Operator    | `+`, `==`, `&&`, `.`, `is`                  |
| Delimiter   | `(`, `)`, `{`, `}`, `;`, `,`                |
| Comment     | `// ...`, `/* ... */`                       |

---

## Example Program

The following example demonstrates all token categories:

```spl
// Point struct with public fields (parentheses, not braces)
pub struct Point(
    pub x: f64,
    pub y: f64,
)

impl Point {
    // Return type uses colon, not arrow
    pub fn new(x: f64, y: f64): Point {
        return Point(x = x, y = y)
    }

    // Named parameters with 'from' label
    pub fn distance(&self, from other: &Point): f64 {
        let dx = self.x - other.x
        let dy = self.y - other.y
        return (dx * dx + dy * dy).sqrt()
    }
}

// Generic function with where clause
fn identity(x: T): T where T {
    return x
}

fn main() {
    // Struct instantiation with named fields
    let mut p1 = Point.new(0.0, 0.0)
    let p2 = Point(x = 3.0, y = 4.0)

    // Calculate distance using named argument
    let dist = p1.distance(from = &p2)

    /* Update p1 position
       using compound assignment */
    p1.x += 1.5e1
    p1.y += 0x0A.widen()

    // Loop with range
    for i in 0..10 {
        if i % 2 == 0 {
            continue
        }
        // Process odd numbers
    }

    // Pattern matching with 'is'
    let maybe_value: Option(i32) = Some(42)
    if maybe_value is Some(v) {
        // Use v here
    }

    if maybe_value is not None {
        // Value exists
    }

    // Match expression
    let result = match maybe_value {
        Some(x) => x * 2,
        None => 0,
    }

    // Boolean and character literals
    let flag: bool = true
    let ch: char = '\n'
    let msg: str = "Hello, SPL!\n"

    // Control flow
    while flag && dist > 0.0 {
        if dist <= 5.0 {
            break
        }
    }

    loop {
        return
    }
}
```

---

## Lexical Ambiguity Resolution

1. **Longest match:** The lexer always takes the longest valid token (e.g., `==` not `=` `=`)
2. **Keyword priority:** Reserved words take precedence over identifiers (e.g., `let` is a keyword, not an identifier)
3. **Numeric prefix:** `0x`, `0b`, `0o` determine integer radix; digits must follow immediately
4. **Range vs float:** `1..2` is integer-range-integer, not a malformed float
