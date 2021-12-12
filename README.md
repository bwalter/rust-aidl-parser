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
    use aidl_parser::ast::*;

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

    let inputs = [interface_aidl, enum_aidl, parcelable_aidl];
    let files = aidl_parser::parse(&inputs)?;

    assert_eq!(
        files[0],
        File(
          package: Package(
            name: "com.bwa.aidl_test",
            symbol_range: "...",
          ),
          imports: [
            Import(
              name: "com.bwa.aidl_test.MyEnum",
              symbol_range: "...",
            ),
            Import(
              name: "com.bwa.aidl_test.MyParcelable",
              symbol_range: "...",
            ),
          ],
          item: Interface(Interface(
            name: "MyInterface",
            elements: [
              Method(Method(
                oneway: true,
                name: "hello",
                return_type: Type(
                  name: "void",
                  kind: Void,
                  generic_types: [],
                  definition: None,
                  symbol_range: "...",
                ),
                args: [
                  Arg(
                    direction: Unspecified,
                    name: Some("e"),
                    arg_type: Type(
                      name: "MyEnum",
                      kind: Custom,
                      generic_types: [],
                      definition: None,
                      symbol_range: "...",
                    ),
                    doc: None,
                    annotations: [],
                  ),
                ],
                annotations: [],
                doc: None,
                symbol_range: "...",
                full_range: "...",
              )),
              Method(Method(
                oneway: false,
                name: "get_name",
                return_type: Type(
                  name: "String",
                  kind: String,
                  generic_types: [],
                  definition: None,
                  symbol_range: "...",
                ),
                args: [
                  Arg(
                    direction: Unspecified,
                    name: None,
                    arg_type: Type(
                      name: "MyParcelable",
                      kind: Custom,
                      generic_types: [],
                      definition: None,
                      symbol_range: "...",
                    ),
                    doc: None,
                    annotations: [],
                  ),
                ],
                annotations: [],
                doc: None,
                symbol_range: "...",
                full_range: "...",
              )),
            ],
            annotations: [],
            doc: Some("Documentation of MyInterface"),
            full_range: "...",
            symbol_range: "...",
          )),
        )
    );

    Ok(())
```

## License

This project is licensed under the [MIT license](LICENSE).

