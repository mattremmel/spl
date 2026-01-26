//! Demo of the AST pretty-printer.
//!
//! Run with: cargo run --example `ast_demo`

use rowan::ast::AstNode;
use spl::ast::SourceFile;
use spl::ast::pretty::pretty_print;
use spl::parse;

fn main() {
    println!("=== SPL AST Pretty-Printer Demo ===\n");

    // A non-trivial SPL program demonstrating various language features
    let source = r"
struct Point {
    x: i32,
    y: i32
}

impl Point {
    fn new(x: i32, y: i32) -> Point {
        Point { x: x, y: y }
    }

    fn distance(&self) -> i32 {
        self.x * self.x + self.y * self.y
    }

    fn translate(&mut self, dx: i32, dy: i32) {
        self.x = self.x + dx;
        self.y = self.y + dy;
    }
}

fn fibonacci(n: i32) -> i32 {
    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}

fn sum_array(arr: &[i32]) -> i32 {
    let mut total = 0;
    for item in arr {
        total = total + item;
    }
    total
}

fn main() {
    let p = Point::new(3, 4);
    let dist = p.distance();

    let fib = fibonacci(10);

    let numbers = [1, 2, 3, 4, 5];
    let sum = sum_array(&numbers);

    let result = if dist > 10 {
        fib
    } else {
        sum
    };

    result
}
";

    // Parse the source
    let parsed = parse(source);

    // Check for parse errors
    if !parsed.ok() {
        println!("Parse errors:");
        for error in parsed.errors() {
            println!("  - {error:?}");
        }
        println!();
    }

    // Convert to typed AST and pretty-print
    let source_file = SourceFile::cast(parsed.syntax()).expect("valid source file");
    let ast_output = pretty_print(&source_file);

    println!("Source code:");
    println!("─────────────────────────────────────────");
    println!("{}", source.trim());
    println!("─────────────────────────────────────────\n");

    println!("AST structure:");
    println!("─────────────────────────────────────────");
    print!("{ast_output}");
    println!("─────────────────────────────────────────\n");

    // Also show the raw syntax tree for comparison
    println!("Raw syntax tree (first 100 lines):");
    println!("─────────────────────────────────────────");
    let debug_tree = parsed.debug_tree();
    for (i, line) in debug_tree.lines().take(100).enumerate() {
        println!("{line}");
        if i == 99 {
            println!("... (truncated)");
        }
    }
    println!("─────────────────────────────────────────");
}
