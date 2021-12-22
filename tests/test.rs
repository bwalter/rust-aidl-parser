use aidl_parser::ParseFileResult;
use anyhow::Result;

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

    insta::assert_ron_snapshot!(res[&0].file.as_ref().unwrap(), {
        ".**.symbol_range" => "...",
        ".**.full_range" => "...",
    });
    insta::assert_ron_snapshot!(res[&0].diagnostics, {
        ".**.range" => "...",
    });

    Ok(())
}

#[test]
fn test_parse_error() -> Result<()> {
    let aidl = "package x.y.z; completly wrong item {}";

    let mut parser = aidl_parser::Parser::new();
    parser.add_content(0, aidl);
    let parse_results = parser.parse();

    assert_eq!(parse_results.len(), 1);
    assert!(parse_results[&0].file.is_none());

    insta::assert_ron_snapshot!(parse_results[&0].diagnostics, @r###"
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
