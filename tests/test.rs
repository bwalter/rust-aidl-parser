use aidl_parser::ParseFileResult;
use anyhow::Result;

#[test]
fn test_parse() -> Result<()> {
    use aidl_parser::Parser;

    let interface_aidl = r#"
        package com.bwa.aidl_test;
    
        import com.bwa.aidl_test.MyEnum;
        import com.bwa.aidl_test.MyParcelable;

        interface MyInterface {
            const int MY_CONST = 12;
            /**
             * Be polite and say hello
             */
            String hello(MyEnum e, MyParcelable);
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

    // AIDL content
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
        assert_eq!(enum_.name, "MyEnum");
        true
    } else {
        false
    };

    insta::assert_ron_snapshot!(res[0].file.as_ref().unwrap(), {
        ".**.symbol_range" => "...",
        ".**.full_range" => "...",
    });

    assert!(ok);
    Ok(())
}

#[test]
fn test_parse_error() -> Result<()> {
    let aidl = "package x.y.z; completly wrong item {}";

    let mut parser = aidl_parser::Parser::new();
    parser.add_content((), aidl);
    let parse_results = parser.parse();

    assert_eq!(parse_results.len(), 1);
    assert!(parse_results[0].file.is_none());

    insta::assert_ron_snapshot!(parse_results[0].diagnostics, @r###"
    [
      Diagnostic(
        kind: Error,
        range: Range(
          start: Position(
            offset: 15,
            line_col: (1, 16),
          ),
          end: Position(
            offset: 24,
            line_col: (1, 25),
          ),
        ),
        message: "Invalid item - Unrecognized token `completly`\nExpected one of ANNOTATION, ENUM, IMPORT or PARCELABLE",
        context_message: Some("unrecognized token"),
        hint: None,
        related_infos: [],
      ),
    ]
    "###);

    Ok(())
}
