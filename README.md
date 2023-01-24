[![Github.com](https://img.shields.io/badge/bwalter-rust--aidl--parser-blue?logo=github)](https://github.com/bwalter/rust-aidl-parser)
[![Crates.io](https://img.shields.io/crates/v/aidl-parser.svg?logo=rust)](https://crates.io/crates/aidl-parser)
[![Documentation](https://img.shields.io/docsrs/aidl-parser?label=docs.rs)](https://docs.rs/aidl-parser)
[![Github Actions](https://img.shields.io/github/workflow/status/bwalter/rust-aidl-parser/main?labels=CI)](https://github.com/bwalter/rust-aidl-parser)

# AIDL parser for Rust

Parse and validate AIDL files (or contents).

For each AIDL file, the parser will return:
- the AST (Abstract Syntax Tree)
- diagnostics (errors and warnings)

The [traverse] module contains various helper functions to extract informations from the AST.

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
aidl-parser = "0.10.1"
```

Create parser, analyze results:

```rust
use aidl_parser::Parser;
use aidl_parser::traverse::{self, SymbolFilter};

// Parse AIDL contents
let mut parser = Parser::new();
parser.add_content("id1", "package test.pkg; interface MyInterface { void hello(String); }");
parser.add_content("id2", "package test.pkg; parcelable Parcelable { int myField; }");
let results = parser.validate();

// Display results
for (id, res) in &results {
    println!("{}: AST = {:#?}", id, res.ast);
    println!("{}: Diagnostics = {:#?}", id, res.diagnostics);
}

// Traverse AST
let ast1 = results["id1"].ast.as_ref().expect("missing AST");
traverse::walk_symbols(ast1, traverse::SymbolFilter::All, |s| println!("- Symbol: {:#?}", s));

// Filter symbols with closure
let symbols = traverse::filter_symbols(ast1, SymbolFilter::ItemsAndItemElements, |s| s.get_name().unwrap_or_default().contains("el"));
println!("Found symbols containing 'el': {:#?}", symbols);

// Find symbol with closure
if let Some(symbol) = traverse::find_symbol(ast1, SymbolFilter::All, |s| s.get_name().as_deref() == Some("myField")) {
  println!("Found myField: {:#?}", symbol);
}

// Find symbol at given position
if let Some(symbol) = traverse::find_symbol_at_line_col(ast1, SymbolFilter::All, (0, 3)) {
  println!("Found myField: {:#?}", symbol);
}
```

## AIDL language support

It is currently a best effort to provide good diagnostic and navigation based on the official AIDL documentation and AOSP implementation.

It is planned to gradually improve language support to cover all the AIDL functionalities. If you encounter any issue or missing functionality which is not mentioned in the README, it is considered as a bug (please submit an issue or a pull request!).

To get more insight on the current grammar and validation, please refer to:
- grammar (lalrpop): <https://github.com/bwalter/rust-aidl-parser/blob/main/src/aidl.lalrpop>
- unit-tests for grammar: <https://github.com/bwalter/rust-aidl-parser/blob/main/src/rules.rs>
- validation incl. unit-tests: <https://github.com/bwalter/rust-aidl-parser/blob/main/src/validation.rs>

Link to AOSP AIDL implementation:
<https://android.googlesource.com/platform/system/tools/aidl/+/refs/heads/master>

## TODO
- Document how to display diagnostics (e.g. with CodeSpan)
- union (Android 12)
- nested types (Android T)
- Allow annotations for list/map parameters?
- User-defined generic types
- Fixed size arrays
- Ignore Java-like imports: "android.os.IInterface", "android.os.IBinder", "android.os.Parcelable", "android.os.Parcel",
      "android.content.Context", "java.lang.String", "java.lang.CharSequence" (but add a warning)
- Const values with arithmetic (e.g.: const int HELLO = 3 * 4)
- Format?
- Set default value of enums parcelable properties
- validate:
  - file name matching item name
  - annotations
  - annotation cannot be attached to primitive type

## License

This project is licensed under the MIT license.
