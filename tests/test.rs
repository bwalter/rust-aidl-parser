use anyhow::Result;

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
    let files = aidl_parser::parse(&inputs)?;

    insta::assert_ron_snapshot!(files[0], {
        ".**.symbol_range" => "...",
        ".**.full_range" => "...",
    });

    Ok(())
}
