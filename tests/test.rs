use anyhow::{Context, Result};

#[test]
fn test_parse() -> Result<()> {
    let interface_aidl = r#"
        package com.bwa.aidl_test;
    
        import com.bwa.aidl_test.MyEnum;
        import com.bwa.aidl_test.MyParcelable;

        /**
         * Documentation of MyInterface
         */
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
    let parse_results = aidl_parser::parse(&inputs);

    // For each file, 1 result
    assert_eq!(parse_results.len(), 3);
    for res in parse_results.iter() {
        // File successfully parsed
        assert!(res.file.is_some());

        // No error/warning
        assert!(res.diagnostics.is_empty());
    }

    let file = parse_results
        .into_iter()
        .next()
        .unwrap()
        .file
        .context("Could not parse file")?;

    insta::assert_ron_snapshot!(file, {
        ".**.symbol_range" => "...",
        ".**.full_range" => "...",
    });

    Ok(())
}

#[test]
fn test_parse_error() -> Result<()> {
    let aidl = "package x.y.z; completly wrong item {}";

    let inputs = [aidl];
    let parse_results = aidl_parser::parse(&inputs);

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
            offset: 37,
            line_col: (1, 38),
          ),
        ),
        text: "Invalid item: Unrecognized token `completly` found at 15:24\nExpected one of ANNOTATION, ENUM, IMPORT, INTERFACE or PARCELABLE",
      ),
    ]
    "###);

    Ok(())
}
