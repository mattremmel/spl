fn main() {
    println!("Test 1: fn 123() {{}}");
    let parse = spl::parser::parse("fn 123() {}");
    println!("OK: {}, Errors: {:?}", parse.ok(), parse.errors());
}
