# SPL Lexical Grammar

This document defines the lexical structure of SPL (Simple Programming Language) - a multi-paradigm, statically-typed language with a clean, modern syntax.

## Overview

The lexer transforms source text into a stream of tokens. Whitespace (`WHITESPACE`, `NEWLINE`) and comments (`COMMENT`) are emitted as **trivia tokens** — they are preserved in the concrete syntax tree (CST) for tooling (formatting, refactoring) but are skipped during grammar-level parsing. The lexer is greedy, always consuming the longest possible match.

---

## Keywords

SPL reserves 37 keywords that cannot be used as identifiers:

| Keyword    | Description                          |
|------------|--------------------------------------|
| `let`      | Variable binding                     |
| `mut`      | Mutable binding modifier             |
| `fn`       | Function declaration                 |
| `gen`      | Generator function declaration       |
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
| `yield`    | Yield value in generator function    |
| `throw`    | Return an error in throws function   |
| `throws`   | Function may return an error         |
| `as`       | Import renaming                      |
| `true`     | Boolean literal true                 |
| `false`    | Boolean literal false                |
| `pub`      | Public visibility modifier           |
| `self`     | Self value in methods                |
| `Self`     | Self type in impl blocks             |
| `use`      | Import declaration                   |
| `module`   | Module declaration (inline or ref)   |
| `super`    | Parent module reference              |
| `where`    | Generic type constraints             |
| `is`       | Pattern matching operator (infix)    |
| `match`    | Match expression                     |
| `extern`   | External function declaration        |
| `const`    | Compile-time constant                |
| `static`   | Static variable                      |
| `unsafe`   | Unsafe block/function                |

---

## Operators

### Arithmetic Operators

| Operator | Description    |
| -------- | -------------- |
| `+`      | Addition       |
| `-`      | Subtraction    |
| `*`      | Multiplication |
| `/`      | Division       |
| `%`      | Modulo         |
| `**`     | Exponentiation |

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

### Bitwise Operators

| Operator | Description      |
|----------|------------------|
| `&`      | Bitwise AND      |
| `\|`     | Bitwise OR       |
| `^`      | Bitwise XOR      |
| `<<`     | Left shift       |
| `>>`     | Right shift      |

### Assignment Operators

| Operator | Description             |
| -------- | ----------------------- |
| `=`      | Assignment              |
| `+=`     | Add and assign          |
| `-=`     | Subtract and assign     |
| `*=`     | Multiply and assign     |
| `/=`     | Divide and assign       |
| `%=`     | Modulo and assign       |
| `**=`    | Exponentiate and assign |
| `&=`     | Bitwise AND and assign  |
| `\|=`    | Bitwise OR and assign   |
| `^=`     | Bitwise XOR and assign  |
| `<<=`    | Left shift and assign   |
| `>>=`    | Right shift and assign  |

### Other Operators

| Operator | Description                        |
|----------|-----------------------------------|
| `.`      | Member access / path separator    |
| `&`      | Reference                         |
| `..`     | Exclusive range (end not included) |
| `..=`    | Inclusive range (end included)    |
| `...`    | Spread / rest / variadic         |
| `$`      | Package root (paths) / array length (indexing/slices) |
| `!`      | Try/propagate (postfix)           |
| `?.`     | Optional chaining                 |
| `??`     | Nullish coalescing                |
| `@`      | Capture list prefix in closures (`@[...]`) |
| `*`      | Dereference (for references)      |
| `=>`     | Match arm separator               |
| `?`      | Optional type (suffix for `Option(T)`) |

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
| `:`   | Colon (types, slices) |
| `,`   | Comma                |

---

## Literals

### Integer Literals

Integers can be written in decimal, hexadecimal, binary, or octal:

| Format      | Pattern                     | Example       |
|-------------|-----------------------------|---------------|
| Decimal     | `[0-9][0-9_]*`              | `42`, `1_000` |
| Hexadecimal | `0[xX][0-9a-fA-F][0-9a-fA-F_]*`| `0x2A`, `0XFF`|
| Binary      | `0[bB][01][01_]*`              | `0b101010`, `0B11`|
| Octal       | `0[oO][0-7][0-7_]*`            | `0o52`, `0O77`|

**Rules:**
- Underscores (`_`) may appear between digits for readability
- Underscores cannot appear at the start of a number
- An underscore may appear before a type suffix (e.g., `42_i64`)
- Leading zeros in decimal literals are allowed (e.g., `007`)

**Type Suffixes:**
Integer literals may have a type suffix: `i8`, `i16`, `i32`, `i64`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128`, `isize`, `usize`, `bigint`.

```
42i64       // i64
255_u8      // u8 with underscore separator
0xFF_u32    // u32 hex literal
999999999999999999999bigint  // arbitrary precision integer
```

**Regex:**
```
INTEGER = 0[xX][0-9a-fA-F][0-9a-fA-F_]* INTEGER_SUFFIX?
        | 0[bB][01][01_]* INTEGER_SUFFIX?
        | 0[oO][0-7][0-7_]* INTEGER_SUFFIX?
        | [0-9][0-9_]* INTEGER_SUFFIX?

INTEGER_SUFFIX = i8 | i16 | i32 | i64 | i128 | isize
               | u8 | u16 | u32 | u64 | u128 | usize
               | bigint
```

### Floating-Point Literals

| Format       | Pattern                              | Example         |
|--------------|--------------------------------------|-----------------|
| Basic        | `[0-9]+\.[0-9]+`                     | `3.14`          |
| Exponent     | `[0-9]+[eE][+-]?[0-9]+`              | `1e10`, `2E-3`  |
| Full         | `[0-9]+\.[0-9]+[eE][+-]?[0-9]+`      | `2.5e-3`        |

**Rules:**
- Must have digits on both sides of the decimal point (`.5` and `5.` are invalid)
- Exponent indicator is `e` or `E`
- Underscores allowed between digits: `1_000.000_001`
- Type suffixes: `f32`, `f64`, `decimal` (e.g., `3.14_f32`, `2.718f64`, `0.10decimal`)

**Decimal Literals:**
The `decimal` suffix creates an exact decimal floating-point value, avoiding binary floating-point precision issues:

```
0.10decimal + 0.20decimal  // Exactly 0.30, not 0.30000000000000004
19.99decimal * 1.0825decimal  // Precise monetary calculation
```

**Regex:**
```
FLOAT = [0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9][0-9_]*)? FLOAT_SUFFIX?
      | [0-9][0-9_]*[eE][+-]?[0-9][0-9_]* FLOAT_SUFFIX?

FLOAT_SUFFIX = f32 | f64 | decimal
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
| `\'`   | Single quote (see note) |
| `\0`   | Null character       |
| `\xNN` | Byte value (hex)     |
| `\u{NNNNNN}` | Unicode code point (1-6 hex digits) |

**Note:** The `\'` escape is primarily useful in character literals (`'\''`). In string literals, single quotes don't need escaping: `"it's"` is valid.

**Examples:** `"hello"`, `"hello\nworld"`, `"say \"hi\""`, `"\u{1F600}"` (emoji)

### Raw String Literals

Raw strings do not process escape sequences. Useful for regex, paths, etc.

```
RAW_STRING = 'r"' [^"]* '"'
           | 'r#"' .* '"#'
           | 'r##"' .* '"##'
           (* ... and so on with more # delimiters *)
```

**Rules:**
- Start with `r"` and end with `"`
- Or use `r#"` ... `"#` to allow `"` inside the string
- Add more `#` characters for nesting: `r##"..."##`

**Lexer Algorithm:**

Raw strings require stateful lexing: the lexer counts the number of `#` characters after `r` in the opening delimiter, then scans for a closing `"` followed by exactly that many `#` characters. The content between delimiters is taken literally without escape processing.

**Implementation Note:** There is no defined limit on the number of `#` delimiters. Implementations should support at least 255 hashes, matching Rust's behavior.

```
r"..."      → 0 hashes, ends at first "
r#"..."#    → 1 hash, ends at first "# sequence
r##"..."##  → 2 hashes, ends at first "## sequence
```

**Examples:**
```spl
let path = r"C:\Users\name";           // No escape processing
let regex = r#"(\d+)-(\d+)"#;          // Contains quotes
let nested = r##"He said "Hi""##;      // Nested quotes
```

### Byte String Literals

Byte strings represent `&[u8]` data, not UTF-8 text:

```
BYTE_STRING = 'b"' [^\x80-\xff"\\]* '"'
```

**Rules:**
- Prefixed with `b`
- Source text contains only ASCII characters (escape sequences can produce any byte 0x00-0xFF)
- Same escape sequences as regular strings
- Type is `&[u8]`, not `&str`

**Examples:** `b"hello"`, `b"data\x00\xFF"`

### Raw Byte String Literals

Combine raw and byte string features:

**Examples:** `br"raw bytes"`, `br#"with "quotes""#`

### C String Literals

C strings are null-terminated and used for FFI:

```
C_STRING = 'c"' [^"]* '"'
```

**Rules:**
- Prefixed with `c`
- Automatically null-terminated
- Type is `CStr`
- Cannot contain interior null bytes (compile error)

**Examples:** `c"Hello, C!"`, `c"/path/to/file"`

### Character Literals

Single characters enclosed in single quotes:

```
CHAR = "'" [^'\\] "'"
     | "'\\" [ntr\\'\"0] "'"
     | "'\\x" [0-9a-fA-F]{2} "'"
     | "'\\u{" [0-9a-fA-F]{1,6} "}'"
```

**Examples:** `'a'`, `'\n'`, `'\\'`, `'\0'`, `'\x7F'`, `'\u{1F600}'`

### Byte Character Literals

Single byte values:

```
BYTE_CHAR = "b'" [^\x80-\xff'\\] "'"
          | "b'\\" [ntr\\'0] "'"
          | "b'\\x" [0-9a-fA-F]{2} "'"
```

**Examples:** `b'A'`, `b'\x00'`, `b'\xFF'`

### Boolean Literals

Boolean values use the keywords `true` and `false`.

### Literal Summary

| Literal Type | Prefix | Example | Result Type |
|--------------|--------|---------|-------------|
| String | (none) | `"hello"` | `&str` |
| Raw string | `r` | `r"C:\path"` | `&str` |
| Byte string | `b` | `b"bytes"` | `&[u8]` |
| Raw byte string | `br` | `br"raw"` | `&[u8]` |
| C string | `c` | `c"ffi"` | `CStr` |
| Character | (none) | `'a'` | `char` |
| Byte character | `b` | `b'A'` | `u8` |

---

## Identifiers

Identifiers name variables, functions, types, and other entities.

**Rules:**
- Must start with a Unicode XID_Start character or underscore (`_`)
- May continue with Unicode XID_Continue characters
- Case-sensitive (`foo` and `Foo` are different)
- Cannot be a keyword

XID_Start and XID_Continue are Unicode character properties defined in [UAX #31](https://unicode.org/reports/tr31/). This includes ASCII letters and digits as a subset, plus letters and combining marks from other scripts.

**Note:** The lexer produces a single `IDENTIFIER` token regardless of case. Case-based disambiguation (uppercase for types, lowercase for values) is handled by the parser.

**Regex:**
```
IDENTIFIER = [\p{XID_Start}_]\p{XID_Continue}*
```

**Examples:** `x`, `foo_bar`, `Point2D`, `_private`, `café`, `日本語`, `αβγ`

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
| Operator    | `+`, `**`, `==`, `&&`, `.`                  |
| Delimiter   | `(`, `)`, `{`, `}`, `;`, `,`                |
| Comment     | `// ...`, `/* ... */`                       |

---

## Lexical Ambiguity Resolution

1. **Longest match:** The lexer always takes the longest valid token (e.g., `**` not `*` `*`, `==` not `=` `=`)
2. **Keyword priority:** Reserved words take precedence over identifiers (e.g., `let` is a keyword, not an identifier)
3. **Numeric prefix:** `0x`/`0X`, `0b`/`0B`, `0o`/`0O` determine integer radix; digits must follow immediately
4. **Range vs float:** `1..2` is integer-range-integer, not a malformed float
