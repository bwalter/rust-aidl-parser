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
- Const values with arithmetic (e.g.: const int HELLO = 3 * 4)
- validate:
  - direction (based on Object), required for all non-primitive parameters, other restrictions regarding in
  - duplicated methods
  - unused/duplicated imports
  - oneway only for void
  - duplicated method values

## Example

```rust
#[test]
fn test_parse() -> Result<()> {
    let interface_aidl = r#"
        package com.bwa.aidl_test;
    
        import com.bwa.aidl_test.MyParcelable;
        import com.bwa.aidl_test.MyParcelable;
        import com.bwa.aidl_test.NonExisting;
        import com.bwa.aidl_test.UnusedEnum;

        interface MyInterface {
            String get_name(MyParcelable);
        }
    "#;

    let parcelable_aidl = r#"
        package com.bwa.aidl_test;
    
        parcelable MyParcelable {
            String name;
            byte[] data;
        }
    "#;

    let enum_aidl = r#"
        package com.bwa.aidl_test;
    
        enum UnusedEnum {
            VALUE1 = 1,
            VALUE2 = 2,
        }
    "#;

    // Parse AIDL files
    let mut parser = aidl_parser::Parser::new();
    parser.add_content(0, interface_aidl);
    parser.add_content(1, parcelable_aidl);
    parser.add_content(2, enum_aidl);
    let res = parser.parse();

    // For each file, 1 result
    assert_eq!(res.len(), 3);

    // Check AST
    use aidl_parser::ast;
    let ok = if let [ParseFileResult {
        file:
            Some(ast::File {
                package: ast::Package { .. },
                item: ast::Item::Interface(interface @ ast::Interface { .. }),
                ..
            }),
        ..
    }, ParseFileResult {
        file:
            Some(ast::File {
                package: ast::Package { .. },
                item: ast::Item::Parcelable(parcelable @ ast::Parcelable { .. }),
                ..
            }),
        ..
    }, ParseFileResult {
        file:
            Some(ast::File {
                package: ast::Package { .. },
                item: ast::Item::Enum(enum_ @ ast::Enum { .. }),
                ..
            }),
        ..
    }] = &res[..]
    {
        assert_eq!(interface.name, "MyInterface");
        assert_eq!(parcelable.name, "MyParcelable");
        assert_eq!(enum_.name, "UnusedEnum");
        true
    } else {
        false
    };

    // Check diagnostics
    assert_eq!(res[0].diagnostics.len(), 3);
    assert!(res[0].diagnostics[0].message.contains("Duplicated import"));
    assert!(res[0].diagnostics[1].message.contains("Unresolved import"));
    assert!(res[0].diagnostics[2].message.contains("Unused import"));
    assert!(res[1].diagnostics.is_empty());
    assert!(res[2].diagnostics.is_empty());

    Ok(())
```

## License

This project is licensed under the [MIT license](LICENSE).

