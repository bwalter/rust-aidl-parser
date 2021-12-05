use anyhow::Result;

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
                #[rustfmt::skip]
                symbol_range: Range {
                    start: Position { offset: 17, line_col: (2, 17) },
                    end: Position { offset: 33, line_col: (2, 33) }
                }
            },
            imports: vec![
                Import {
                    name: "com.bwa.aidl_test.MyEnum".into(),
                    #[rustfmt::skip]
                    symbol_range: Range {
                        start: Position { offset: 56, line_col: (4, 16) },
                        end: Position { offset: 79, line_col: (4, 39) }
                    }
                },
                Import {
                    name: "com.bwa.aidl_test.MyParcelable".into(),
                    #[rustfmt::skip]
                    symbol_range: Range {
                        start: Position { offset: 97, line_col: (5, 16) },
                        end: Position { offset: 126, line_col: (5, 45) }
                    }
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

                            #[rustfmt::skip]
                            symbol_range: Range {
                                start: Position { offset: 181, line_col: (8, 20) },
                                end: Position { offset: 185, line_col: (8, 24) }
                            },
                        },
                        args: vec![Arg {
                            direction: Direction::Unspecified,
                            name: Some("e".into()),
                            arg_type: Type {
                                name: "MyEnum".into(),
                                kind: TypeKind::Custom,
                                generic_types: vec![],
                                definition: None,

                                #[rustfmt::skip]
                                symbol_range: Range {
                                    start: Position { offset: 192, line_col: (8, 31) },
                                    end: Position { offset: 198, line_col: (8, 37) }
                                },
                            },
                            annotations: vec![],
                        }],
                        annotations: vec![],

                        #[rustfmt::skip]
                        symbol_range: Range {
                            start: Position { offset: 186, line_col: (8, 25) },
                            end: Position { offset: 191, line_col: (8, 30) }
                        },

                        #[rustfmt::skip]
                        full_range: Range {
                            start: Position { offset: 174, line_col: (8, 13) },
                            end: Position { offset: 215, line_col: (9, 13) }
                        },
                    }),
                    InterfaceElement::Method(Method {
                        oneway: false,
                        name: "get_name".into(),
                        return_type: Type {
                            name: "String".into(),
                            kind: TypeKind::String,
                            generic_types: vec![],
                            definition: None,

                            #[rustfmt::skip]
                            symbol_range: Range {
                                start: Position { offset: 215, line_col: (9, 13) },
                                end: Position { offset: 221, line_col: (9, 19) }
                            },
                        },
                        args: vec![Arg {
                            direction: Direction::Unspecified,
                            name: None,
                            arg_type: Type {
                                name: "MyParcelable".into(),
                                kind: TypeKind::Custom,
                                generic_types: vec![],
                                definition: None,

                                #[rustfmt::skip]
                                symbol_range: Range {
                                    start: Position { offset: 231, line_col: (9, 29) },
                                    end: Position { offset: 243, line_col: (9, 41) }
                                },
                            },
                            annotations: vec![],
                        }],
                        annotations: vec![],

                        #[rustfmt::skip]
                        symbol_range: Range {
                            start: Position { offset: 222, line_col: (9, 20) },
                            end: Position { offset: 230, line_col: (9, 28) }
                        },

                        #[rustfmt::skip]
                        full_range: Range {
                            start: Position { offset: 215, line_col: (9, 13) },
                            end: Position { offset: 254, line_col: (10, 9) }
                        }
                    })
                ],
                annotations: vec![],

                #[rustfmt::skip]
                full_range: Range {
                    start: Position { offset: 138, line_col: (7, 9) },
                    end: Position { offset: 254, line_col: (10, 9) }
                },

                #[rustfmt::skip]
                symbol_range: Range {
                    start: Position { offset: 148, line_col: (7, 19) },
                    end: Position { offset: 158, line_col: (7, 29) }
                }
            })
        }
    );

    Ok(())
}
