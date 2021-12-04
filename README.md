# rust-aidl

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
- Parse JavaDoc

## Example

```rust
#[test]
fn test_parse() -> Result<()> {
    use aidl_parser::types::*;

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
        File {
            package: Package {
                name: "com.bwa.aidl_test".into(),
                symbol_range: Range { ... },
            },
            imports: vec![
                Import {
                    name: "com.bwa.aidl_test.MyEnum".into(),
                    symbol_range: Range { ... },
                },
                Import {
                    name: "com.bwa.aidl_test.MyParcelable".into(),
                    symbol_range: Range { ... },
                }
            ],
            item: Item::Interface(Interface {
                name: "MyInterface".into(),
                elements: vec![
                    InterfaceElement::Method(Method {
                        oneway: true,
                        name: "hello".into(),
                        return_type: Type {
                            name: "void".into(),
                            kind: TypeKind::Void,
                            generic_types: vec![],
                            definition: None,
                            range: Range { ... },
                        },
                        args: vec![Arg {
                            direction: Direction::Unspecified,
                            name: Some("e".into()),
                            arg_type: Type {
                                name: "MyEnum".into(),
                                kind: TypeKind::Custom,
                                generic_types: vec![],
                                definition: None,
                                range: Range { ... },
                            }
                        }],
                        symbol_range: Range { ... },
                        full_range: Range { ... },
                    }),
                    InterfaceElement::Method(Method {
                        oneway: false,
                        name: "get_name".into(),
                        return_type: Type {
                            name: "String".into(),
                            kind: TypeKind::String,
                            generic_types: vec![],
                            definition: None,
                            range: Range { ... },
                        },
                        args: vec![Arg {
                            direction: Direction::Unspecified,
                            name: None,
                            arg_type: Type {
                                name: "MyParcelable".into(),
                                kind: TypeKind::Custom,
                                generic_types: vec![],
                                definition: None,
                                range: Range { ... },
                            }
                        }],
                        symbol_range: Range { ... },
                        full_range: Range { ... },
                    })
                ],
                full_range: Range { ... },
                symbol_range: Range { ... },
            })
        }
    );

    Ok(())
```

## License

This project is licensed under the [MIT license](LICENSE).

