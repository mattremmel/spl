//! Helper functions for type inference.

use spl_lexer::Span;
use crate::types::PrimitiveKind;

/// Convert a rowan `TextRange` to a Span.
pub fn text_range_to_span(range: rowan::TextRange) -> Span {
    range.start().into()..range.end().into()
}

/// Check if a primitive type is an integer type.
pub fn is_integer_type(prim: PrimitiveKind) -> bool {
    matches!(
        prim,
        PrimitiveKind::I8
            | PrimitiveKind::I16
            | PrimitiveKind::I32
            | PrimitiveKind::I64
            | PrimitiveKind::I128
            | PrimitiveKind::Isize
            | PrimitiveKind::U8
            | PrimitiveKind::U16
            | PrimitiveKind::U32
            | PrimitiveKind::U64
            | PrimitiveKind::U128
            | PrimitiveKind::Usize
    )
}

/// Check if a primitive type is a float type.
pub fn is_float_type(prim: PrimitiveKind) -> bool {
    matches!(prim, PrimitiveKind::F32 | PrimitiveKind::F64)
}

/// Check if a primitive type is a numeric type (integer or float).
pub fn is_numeric_type(prim: PrimitiveKind) -> bool {
    is_integer_type(prim) || is_float_type(prim)
}

/// Parse an integer literal suffix to determine the type.
/// Returns (Some(kind), true) if there's a suffix, (None, false) otherwise.
pub fn parse_int_suffix(text: &str) -> (Option<PrimitiveKind>, bool) {
    // Check suffixes in order of length (longest first to avoid i12 matching i1)
    let suffixes = [
        ("i128", PrimitiveKind::I128),
        ("u128", PrimitiveKind::U128),
        ("isize", PrimitiveKind::Isize),
        ("usize", PrimitiveKind::Usize),
        ("i64", PrimitiveKind::I64),
        ("u64", PrimitiveKind::U64),
        ("i32", PrimitiveKind::I32),
        ("u32", PrimitiveKind::U32),
        ("i16", PrimitiveKind::I16),
        ("u16", PrimitiveKind::U16),
        ("i8", PrimitiveKind::I8),
        ("u8", PrimitiveKind::U8),
    ];

    for (suffix, kind) in suffixes {
        if text.ends_with(suffix) {
            return (Some(kind), true);
        }
    }
    (None, false)
}

/// Parse the numeric value of an integer literal (stripping any suffix).
pub fn parse_int_literal_value(text: &str) -> Option<i128> {
    // Strip the type suffix (e.g., u8, i32, usize)
    // Must check longer suffixes first to avoid partial matches
    let suffixes = [
        "i128", "u128", "isize", "usize", "i64", "u64", "i32", "u32", "i16", "u16", "i8", "u8",
    ];
    let num_text = suffixes
        .iter()
        .find(|s| text.ends_with(*s))
        .map(|s| &text[..text.len() - s.len()])
        .unwrap_or(text);

    // Handle hex, octal, binary prefixes
    if num_text.starts_with("0x") || num_text.starts_with("0X") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 16).ok()
    } else if num_text.starts_with("0o") || num_text.starts_with("0O") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 8).ok()
    } else if num_text.starts_with("0b") || num_text.starts_with("0B") {
        i128::from_str_radix(&num_text[2..].replace('_', ""), 2).ok()
    } else {
        // Decimal - remove underscores
        num_text.replace('_', "").parse().ok()
    }
}

/// Compute the Levenshtein (edit) distance between two strings.
/// This measures the minimum number of single-character edits (insertions,
/// deletions, or substitutions) required to transform one string into another.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut dp: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut prev = i;
        dp[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let curr = dp[j + 1];
            dp[j + 1] = if ca == cb {
                prev
            } else {
                1 + prev.min(curr).min(dp[j])
            };
            prev = curr;
        }
    }
    dp[b.len()]
}

/// Find the most similar string from a list of candidates.
/// Returns the best match if it's within `max_distance` edits, otherwise None.
pub fn find_similar<'a>(
    target: &str,
    candidates: &[&'a str],
    max_distance: usize,
) -> Option<&'a str> {
    candidates
        .iter()
        .filter(|&&name| name != target)
        .filter_map(|&name| {
            let dist = levenshtein(target, name);
            if dist <= max_distance {
                Some((name, dist))
            } else {
                None
            }
        })
        .min_by_key(|(_, dist)| *dist)
        .map(|(name, _)| name)
}
