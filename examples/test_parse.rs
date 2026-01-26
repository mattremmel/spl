fn main() {
    // Test with for loop and tail expression
    let input = "fn foo() { for x in y { z } bar }";
    println!("Input: {input:?}");

    let parse = spl::parser::parse(input);
    println!("OK: {}, Errors: {:?}", parse.ok(), parse.errors());
    println!("\nTree:\n{}", parse.debug_tree());
}
