# aidl-parser

AIDL parser for Rust.

## Features

The following items are supported:
- Package
- Imports
- Item (Interface, Parcelable, Enum)
- InterfaceElement (Method, Constant)
- ParcelableElement (Member)
- Enum (EnumElement)

It provides a basic validation but does not (yet) check for the full specifications.

TODO:
- Resolve types

## Example

```rust
#[test]
fn test_parse() -> Result<()> {
    let interface_aidl = r#"
        package com.bwa.aidl_test;
    
        import com.bwa.aidl_test.MyEnum;
        import com.bwa.aidl_test.MyParcelable;

        interface MyInterface {
            oneway void hello(MyEnum e);
            String get_name(MyParcelable);
        }
    "#;

    let enum_aidl = r#"
        package com.bwa.aidl_test;
    
        enum MyEnum {
            VALUE1 = 1,
            VALUE2 = 2,
        }
    "#;

    let parcelable_aidl = r#"
        package com.bwa.aidl_test;
    
        parcelable MyParcelable {
            String name;
            byte[] data;
        }
    "#;

    // Parse AIDL files and return AST
    let parse_results = aidl_parser::parse(&inputs);

    // For each file, 1 result
    assert_eq!(parse_results.len(), 3);
    for res in parse_results.iter() {
        // File successfully parsed
        assert!(res.file.is_some());

        // No error/warning
        assert!(res.diagnostics.is_empty());
    }

    Ok(())
```

## License

This project is licensed under the [MIT license](LICENSE).

