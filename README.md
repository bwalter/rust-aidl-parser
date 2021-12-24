# aidl-parser

AIDL parser for Rust.

## Features

- Generate AIDL AST with all supported elements (including Javadoc)
- Recover errors
- Validate project
- Provide diagnostics (errors and warnings) with location

## AIDL language support

It is currently a best effort to provide good diagnostic and navigation based on AIDL documentation.

The code base is (much) simpler than the official implementation but (arguably) more readable and easier to understand and does not support legacy options. It is planned to gradually improve language support to cover most of the functionalities of the AIDL language.

If you need specific support, please do not hesitate to submit an issue or a pull request.

Link to AOSP AIDL implementation:
https://android.googlesource.com/platform/system/tools/aidl/+/refs/heads/master/

## TODO
- Document how to display diagnostics (e.g. with CodeSpan)
- Annotation attached to primitive type
- union (Android 12)
- nested types (Android T)
- Parcelable declaration(= forward declaration), with optional annotations
- Allow annotations for list/map parameters?
- Android types: android.os.Parcelable, IBinder, FileDescriptor, ParcelFileDescriptor, 
- Const values with arithmetic (e.g.: const int HELLO = 3 * 4)
- validate:
  - duplicated method names
  - duplicated method values
  - file name matching item name

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
aidl-parser = "0.3.0"
```

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
    assert!(matches!(res.get(&0), Some(ParseFileResult {
        file: Some(ast::File {
            package: ast::Package { .. },
            item: ast::Item::Interface(interface @ ast::Interface { .. }),
            ..
        }),
        ..
    }) if interface.name == "MyInterface"));
    assert!(matches!(res.get(&1), Some(ParseFileResult {
        file: Some(ast::File {
            package: ast::Package { .. },
            item: ast::Item::Parcelable(parcelable @ ast::Parcelable { .. }),
            ..
        }),
        ..
    }) if parcelable.name == "MyParcelable"));
    assert!(matches!(res.get(&2), Some(ParseFileResult {
        file: Some(ast::File {
            package: ast::Package { .. },
            item: ast::Item::Enum(enum_ @ ast::Enum { .. }),
            ..
        }),
        ..
    }) if enum_.name == "UnusedEnum"));

    // Check diagnostics
    assert_eq!(res[&0].diagnostics.len(), 3);
    assert!(res[&0].diagnostics[0].message.contains("Duplicated import"));
    assert!(res[&0].diagnostics[1].message.contains("Unresolved import"));
    assert!(res[&0].diagnostics[2].message.contains("Unused import"));
    assert!(res[&1].diagnostics.is_empty());
    assert!(res[&2].diagnostics.is_empty());

    Ok(())
```

## License

This project is licensed under the [MIT license](LICENSE).
