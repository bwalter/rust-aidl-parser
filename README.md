# aidl-parser

AIDL parser for Rust.

## Features

- Generate AIDL AST with all supported elements (including Javadoc)
- Recover errors
- Provide diagnostics (errors and warnings) with location

TODO:
- Document how to display diagnostics (e.g. with CodeSpan)
- Remove line_col from results and let the client calculate it?
- Annotation attached to primitive type
- union (Android 12)
- nested types (Android T)
- Parcelable declaration(= forward declaration), with optional annotations
- Allow annotations for list/map parameters?
- Android types: android.os.Parcelable, IBinder, FileDescriptor, ParcelFileDescriptor, 
- validate:
  - direction (based on Object), required for all non-primitive parameters, other restrictions regarding in
  - duplicated methods
  - unused/duplicated imports
  - oneway only for void



## Example

```rust
#[test]
fn test_parse() -> Result<()> {
    use aidl_parser::{ast, Parser};
    
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

    // Parse AIDL files
    let mut parser = Parser::new();
    parser.add_content(0, interface_aidl);
    parser.add_content(1, parcelable_aidl);
    parser.add_content(2, enum_aidl);
    let res = parser.parse();

    // For each file, 1 result
    assert_eq!(res.len(), 3);

    // No error/warning
    assert!(res[0].diagnostics.is_empty());
    assert!(res[1].diagnostics.is_empty());
    assert!(res[2].diagnostics.is_empty());

    // File items
    let item0 = res[0].file.as_ref().unwrap().item;
    if let ast::Item::Interface(ast::Interface { ref name, .. }) {
        assert_eq!("name", "MyInterface");
    }
    assert!(matches!(
        item0,
        ast::Item::Interface(ast::Interface { ref name, .. }) if name == "MyInterface",
    ));
    assert!(matches!(
        res[1].file.as_ref().unwrap().item,
        ast::Item::Parcelable(ast::Parcelable { ref name, .. }) if name == "MyParcelable",
    ));
    assert!(matches!(
        res[2].file.as_ref().unwrap().item,
        ast::Item::Enum(ast::Enum { ref name, .. }) if name == "MyEnum",
    ));

    Ok(())
```

## License

This project is licensed under the [MIT license](LICENSE).

