use anyhow::Result;

#[test]
fn test_parse() {
    let interface_aidl = r#"
        package com.bwa.aidl_test;
    
        import com.bwa.aidl_test.MyParcelable;
        import com.bwa.aidl_test.MyParcelable;
        import com.bwa.aidl_test.NonExisting;
        import com.bwa.aidl_test.UnusedEnum;

        interface MyInterface {
            void method1(in MyParcelable);
            String method2();
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
    parser.add_content("id1", interface_aidl);
    parser.add_content("id2", parcelable_aidl);
    parser.add_content("id3", enum_aidl);
    let res = parser.validate();

    // For each file, 1 result
    assert_eq!(res.len(), 3);

    // Check AIDL 1
    let ast1 = res.get("id1").expect("result").ast.as_ref().expect("ast");
    let interface = ast1.item.as_interface().expect("interface");
    assert_eq!(interface.name, "MyInterface");

    // Check AIDL 2
    let ast2 = res.get("id2").expect("result").ast.as_ref().expect("ast");
    let parcelable = ast2.item.as_parcelable().expect("parcelable");
    assert_eq!(parcelable.name, "MyParcelable");

    // Check AIDL 3
    let ast3 = res.get("id3").expect("result").ast.as_ref().expect("ast");
    let enum_ = ast3.item.as_enum().expect("enum");
    assert_eq!(enum_.name, "UnusedEnum");

    // Check diagnostics
    assert_eq!(res["id1"].diagnostics.len(), 3);
    assert!(res["id1"].diagnostics[0]
        .message
        .contains("Duplicated import"));
    assert!(res["id1"].diagnostics[1]
        .message
        .contains("Unresolved import"));
    assert!(res["id1"].diagnostics[2].message.contains("Unused import"));
    assert!(res["id2"].diagnostics.is_empty());
    assert!(res["id3"].diagnostics.is_empty());

    // Traverse AST
    let mut methods = Vec::new();
    aidl_parser::traverse::walk_methods(ast1, |m| methods.push(m));
    assert_eq!(methods.len(), 2);
    assert_eq!(methods[0].name, "method1");
    assert_eq!(methods[1].name, "method2");
}

#[test]
fn test_parse_error() -> Result<()> {
    let aidl = "package x.y.z; completly wrong item {}";

    let mut parser = aidl_parser::Parser::new();
    parser.add_content(0, aidl);
    let parse_results = parser.validate();

    assert_eq!(parse_results.len(), 1);
    assert!(parse_results[&0].ast.is_none());

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
        message: "Invalid item - Unrecognized token `completly`\nExpected one of ANNOTATION, ENUM, IMPORT, INTERFACE or PARCELABLE",
        context_message: Some("unrecognized token"),
        hint: None,
        related_infos: [],
      ),
    ]
    "###);

    Ok(())
}
