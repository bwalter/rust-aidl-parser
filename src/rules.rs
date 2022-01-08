use lalrpop_util::lalrpop_mod;

lalrpop_mod!(#[allow(clippy::all, dead_code, unused_imports)] pub aidl);

#[cfg(test)]
mod tests {
    use crate::rules;
    use anyhow::Result;

    fn lookup(input: &str) -> line_col::LineColLookup {
        line_col::LineColLookup::new(input)
    }

    // Replace ranges into "..." and check parse output via insta Ron snapshot
    macro_rules! assert_parser {
        ($input:ident, $parser:expr) => {
            let mut diagnostics = Vec::new();
            let lookup = lookup($input);
            let res = $parser.parse(&lookup, &mut diagnostics, $input)?;
            ::insta::assert_ron_snapshot!(res, {
                ".**.symbol_range" => "...",
                ".**.full_range" => "...",
                ".**.value_range" => "...",
                ".**.oneway_range" => "...",
            });
            assert_eq!(diagnostics, &[]);
        };

        ($input:ident, $parser:expr, $diag:expr) => {
            let lookup = lookup($input);
            let res = $parser.parse(&lookup, $diag, $input)?;
            ::insta::assert_ron_snapshot!(res, {
                ".**.symbol_range" => "...",
                ".**.full_range" => "...",
                ".**.value_range" => "...",
                ".**.oneway_range" => "...",
            });
        };
    }

    macro_rules! assert_diagnostics {
        ($diag:expr, @$snapshot:literal) => {
            ::insta::assert_ron_snapshot!($diag, {
                ".**.range" => "...",
            }, @$snapshot);
        };
    }

    #[test]
    fn test_aidl() -> Result<()> {
        let input = r#"package x.y.z;
            import a.b.c;
            interface MyInterface {}
        "#;
        assert_parser!(input, rules::aidl::OptAidlParser::new());

        Ok(())
    }

    #[test]
    fn test_aidl_with_unrecovered_error() -> Result<()> {
        use crate::diagnostic::ParseError;

        let input = "wrong, wrong and wrong!";
        let lookup = lookup(input);
        let res = rules::aidl::OptAidlParser::new().parse(&lookup, &mut Vec::new(), input);

        assert!(matches!(res, Err(ParseError::InvalidToken { .. })));

        Ok(())
    }

    #[test]
    fn test_aidl_with_recovered_error() -> Result<()> {
        let input = r#"package x.y.z;
               import a.b.c;
               oops_interface MyInterface {}
           "#;
        let mut diagnostics = Vec::new();
        assert_parser!(input, rules::aidl::OptAidlParser::new(), &mut diagnostics);

        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            message: "Invalid item - Unrecognized token `oops_interface`.\nExpected one of ANNOTATION, ENUM, IMPORT, INTERFACE or PARCELABLE",
            context_message: Some("unrecognized token"),
            hint: None,
            related_infos: [],
          ),
        ]
        "###);

        Ok(())
    }

    #[test]
    fn test_package1() -> Result<()> {
        let input = "package x ;";
        assert_parser!(input, rules::aidl::PackageParser::new());

        Ok(())
    }

    #[test]
    fn test_package2() -> Result<()> {
        let input = "package x.y.z;";
        assert_parser!(input, rules::aidl::PackageParser::new());

        Ok(())
    }

    #[test]
    fn test_import() -> Result<()> {
        let input = "import x.y.z;";
        assert_parser!(input, rules::aidl::ImportParser::new());

        Ok(())
    }

    #[test]
    fn test_declared_parcelable() -> Result<()> {
        let mut diagnostics = Vec::new();

        let input = "parcelable X;";
        assert_parser!(
            input,
            rules::aidl::DeclaredParcelableParser::new(),
            &mut diagnostics
        );
        let input = "parcelable any.pkg.Y;";
        assert_parser!(
            input,
            rules::aidl::DeclaredParcelableParser::new(),
            &mut diagnostics
        );
        let input = "@Annotation1 @Annotation2\nparcelable any.pkg.Y;";
        assert_parser!(
            input,
            rules::aidl::DeclaredParcelableParser::new(),
            &mut diagnostics
        );

        Ok(())
    }

    #[test]
    fn test_interface() -> Result<()> {
        let input = r#"interface Potato {
            /**
             * const1 documentation
             */
            const int const1 = 1;
    
            /**
             * method1 documentation
             */
            String method1();
    
            const String const2 = "two";
            int method2();
        }"#;
        assert_parser!(input, rules::aidl::InterfaceParser::new());

        Ok(())
    }

    #[test]
    fn test_oneway_interface() -> Result<()> {
        let input = r#"oneway interface OneWayInterface {}"#;
        assert_parser!(input, rules::aidl::InterfaceParser::new());

        Ok(())
    }

    #[test]
    fn test_interface_with_annotation() -> Result<()> {
        let input = r#"@InterfaceAnnotation1
            @InterfaceAnnotation2 interface Potato {
            }"#;
        assert_parser!(input, rules::aidl::InterfaceParser::new());

        Ok(())
    }

    #[test]
    fn test_interface_with_javadoc() -> Result<()> {
        let input = r#"
            /** Documentation before */
            /** Interface documentation */
            /* Comment after */
            // Line comment after
            interface Potato {
            }"#;
        assert_parser!(input, rules::aidl::InterfaceParser::new());

        Ok(())
    }

    #[test]
    fn test_interface_with_errors() -> Result<()> {
        let input = r#"interface Potato {
            String method1();
            int method2();
            int oops_not_a_valid_method;
            const String const2 = 123;
            const oops_not_a_valid_const;
        }"#;
        let mut diagnostics = Vec::new();
        assert_parser!(input, rules::aidl::InterfaceParser::new(), &mut diagnostics);

        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            message: "Invalid interface element - Unrecognized token `;`.\nExpected \"(\"",
            context_message: Some("unrecognized token"),
            hint: None,
            related_infos: [],
          ),
          Diagnostic(
            kind: Error,
            range: "...",
            message: "Invalid interface element - Unrecognized token `;`.\nExpected one of \")\", \",\", \".\", \">\" or IDENT",
            context_message: Some("unrecognized token"),
            hint: None,
            related_infos: [],
          ),
        ]
        "###);

        Ok(())
    }

    #[test]
    fn test_parcelable() -> Result<()> {
        let input = r#"parcelable Tomato {
            /**
             * field1 documentation
             */
            int field1;
    
            String field2; // inline comment
        }"#;
        assert_parser!(input, rules::aidl::ParcelableParser::new());

        Ok(())
    }

    #[test]
    fn test_parcelable_with_javadoc() -> Result<()> {
        let input = r#"
            /** Parcelable documentation */
            parcelable Tomato {}"#;
        assert_parser!(input, rules::aidl::ParcelableParser::new());

        Ok(())
    }

    #[test]
    fn test_parcelable_with_errors() -> Result<()> {
        let input = r#"parcelable Tomato {
            int field1;
            wrongfield3;
            String field3;
        }"#;
        let mut diagnostics = Vec::new();
        assert_parser!(
            input,
            rules::aidl::ParcelableParser::new(),
            &mut diagnostics
        );
        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            message: "Invalid field - Unrecognized token `;`.\nExpected one of \",\", \".\", \">\" or IDENT",
            context_message: Some("unrecognized token"),
            hint: None,
            related_infos: [],
          ),
        ]
        "###);

        Ok(())
    }

    #[test]
    fn test_enum() -> Result<()> {
        let input = r#"enum Paprika {
                /**
                 * element1 documentation
                 */
                ELEMENT1 = 3,
    
                ELEMENT2 = "quattro",
                ELEMENT3
            }"#;
        assert_parser!(input, rules::aidl::EnumParser::new());

        Ok(())
    }

    #[test]
    fn test_enum_with_javadoc() -> Result<()> {
        let input = r#"
            /** Enum documentation */
            enum Tomato {
                /** ELEMENT1 documentation */
                ELEMENT1,
                ELEMENT2,
                /** ELEMENT3 documentation */
                ELEMENT3,
            }"#;
        assert_parser!(input, rules::aidl::EnumParser::new());

        Ok(())
    }

    #[test]
    fn test_enum_with_errors() -> Result<()> {
        let input = r#"enum Paprika {
                ELEMENT1 = 3,
                ELEMENT2 == "quattro",
                ELEMENT3,
                0843
            }"#;
        let mut diagnostics = Vec::new();
        assert_parser!(input, rules::aidl::EnumParser::new(), &mut diagnostics);
        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            message: "Invalid enum element - Unrecognized token `=`.\nExpected one of \"{\", BOOLEAN, FLOAT or QUOTED_STRING",
            context_message: Some("unrecognized token"),
            hint: None,
            related_infos: [],
          ),
          Diagnostic(
            kind: Error,
            range: "...",
            message: "Invalid enum element - Unrecognized token `0843`.\nExpected one of \"}\" or IDENT",
            context_message: Some("unrecognized token"),
            hint: None,
            related_infos: [],
          ),
        ]
        "###);

        Ok(())
    }

    #[test]
    fn test_enum_with_trailing_comma() -> Result<()> {
        let input = r#"enum Paprika {
                ELEMENT1,
                ELEMENT2,
            }"#;
        assert_parser!(input, rules::aidl::EnumParser::new());

        Ok(())
    }

    #[test]
    fn test_method_without_arg() -> Result<()> {
        let input = "TypeName myMethod() ;";
        assert_parser!(input, rules::aidl::MethodParser::new());

        Ok(())
    }

    #[test]
    fn test_method_with_1_arg() -> Result<()> {
        let input = "TypeName myMethod(ArgType arg) ;";
        assert_parser!(input, rules::aidl::MethodParser::new());

        Ok(())
    }

    #[test]
    fn test_method_with_3_args() -> Result<()> {
        let input = "TypeName myMethod(ArgType1, ArgType2 arg2, ArgType3) ;";
        assert_parser!(input, rules::aidl::MethodParser::new());

        Ok(())
    }

    #[test]
    fn test_method_oneway() -> Result<()> {
        let input = "oneway TypeName myMethod();";
        assert_parser!(input, rules::aidl::MethodParser::new());

        Ok(())
    }

    #[test]
    fn test_method_with_value() -> Result<()> {
        let input = "TypeName myMethod() = 123;";
        assert_parser!(input, rules::aidl::MethodParser::new());

        Ok(())
    }

    #[test]
    fn test_method_with_invalid_value() -> Result<()> {
        let input = "TypeName myMethod() = 12.3;";
        assert!(rules::aidl::MethodParser::new()
            .parse(&lookup(input), &mut Vec::new(), input)
            .is_err());

        Ok(())
    }

    #[test]
    fn test_method_with_annotation() -> Result<()> {
        let input = "@AnnotationName void myMethod();";
        assert_parser!(input, rules::aidl::MethodParser::new());

        Ok(())
    }

    #[test]
    fn test_method_with_javadoc() -> Result<()> {
        let input = "/** Method documentation */ void myMethod() = 123;";
        assert_parser!(input, rules::aidl::MethodParser::new());

        Ok(())
    }

    #[test]
    fn test_method_arg_with_name() -> Result<()> {
        let input = "TypeName albert";
        assert_parser!(input, rules::aidl::ArgParser::new());

        Ok(())
    }

    #[test]
    fn test_method_arg_with_direction() -> Result<()> {
        let input = "in TypeName";
        assert_parser!(input, rules::aidl::ArgParser::new());

        Ok(())
    }

    #[test]
    fn test_method_arg_with_direction_and_name() -> Result<()> {
        let input = "out TypeName roger";
        assert_parser!(input, rules::aidl::ArgParser::new());

        Ok(())
    }

    #[test]
    fn test_method_arg_with_annotations() -> Result<()> {
        let input = r#"@Annotation1
            @Annotation2(AnnotationParam ) TypeName albert"#;
        assert_parser!(input, rules::aidl::ArgParser::new());

        Ok(())
    }

    #[test]
    fn test_method_arg_with_javadoc() -> Result<()> {
        let input = "/** Arg documentation */ TypeName albert";
        assert_parser!(input, rules::aidl::ArgParser::new());

        Ok(())
    }

    #[test]
    fn test_field() -> Result<()> {
        let input = "TypeName fieldName ;";
        assert_parser!(input, rules::aidl::FieldParser::new());
        Ok(())
    }

    #[test]
    fn test_field_with_value() -> Result<()> {
        let input = "TypeName fieldName = \"field value\";";
        assert_parser!(input, rules::aidl::FieldParser::new());

        Ok(())
    }

    #[test]
    fn test_field_with_javadoc() -> Result<()> {
        let input = r#"/**
             * Field documentation
             */
            TypeName fieldName;"#;
        assert_parser!(input, rules::aidl::FieldParser::new());

        Ok(())
    }

    #[test]
    fn test_field_with_annotation() -> Result<()> {
        let input = "@AnnotationName TypeName fieldName = \"field value\";";
        assert_parser!(input, rules::aidl::FieldParser::new());

        Ok(())
    }

    #[test]
    fn test_const_num() -> Result<()> {
        let input = "const int CONST_NAME = 123 ;";
        assert_parser!(input, rules::aidl::ConstParser::new());

        Ok(())
    }

    #[test]
    fn test_const_string() -> Result<()> {
        let input = "const TypeName CONST_NAME = \"const value\";";
        assert_parser!(input, rules::aidl::ConstParser::new());

        Ok(())
    }

    #[test]
    fn test_const_with_javadoc() -> Result<()> {
        let input = r#"/**
            * Const documentation
            */
           const TypeName CONST_NAME = 123;"#;
        assert_parser!(input, rules::aidl::ConstParser::new());

        Ok(())
    }

    #[test]
    fn test_const_with_annotation() -> Result<()> {
        let input = "@AnnotationName const TypeName CONST_NAME = 123;";
        assert_parser!(input, rules::aidl::ConstParser::new());

        Ok(())
    }

    #[test]
    fn test_type_primitive1() -> Result<()> {
        let input = "double";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_primitive2() -> Result<()> {
        let input = "doublegum";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_string() -> Result<()> {
        let input = "String";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_char_sequence() -> Result<()> {
        let input = "CharSequence";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_android_builtin() -> Result<()> {
        let inputs = ["ParcelableHolder", "IBinder", "FileDescriptor", "ParcelFileDescriptor"];
        
        for input in inputs.into_iter() {
            assert_parser!(input, rules::aidl::TypeParser::new());
        }

        Ok(())
    }

    #[test]
    fn test_type_custom() -> Result<()> {
        let input = "TypeName";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_custom_with_namespace() -> Result<()> {
        let input = "com.example.TypeName";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_array() -> Result<()> {
        let input = "float []";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_array_bidirectional() -> Result<()> {
        let input = "int [] []";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_list() -> Result<()> {
        let input = "List <MyObject >";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_list_non_generic() -> Result<()> {
        let input = "List";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_list_invalid() -> Result<()> {
        let input = "List<A, B>";
        assert!(rules::aidl::ValueParser::new()
            .parse(&lookup(input), &mut Vec::new(), input)
            .is_err());

        Ok(())
    }

    #[test]
    fn test_type_map() -> Result<()> {
        let input = "Map<Key,List<V>>";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_map_non_generic() -> Result<()> {
        let input = "Map";
        assert_parser!(input, rules::aidl::TypeParser::new());

        Ok(())
    }

    #[test]
    fn test_type_map_invalid() -> Result<()> {
        let input = "Map<A>";
        assert!(rules::aidl::ValueParser::new()
            .parse(&lookup(input), &mut Vec::new(), input)
            .is_err());

        let input = "Map<A,B,C>";
        assert!(rules::aidl::ValueParser::new()
            .parse(&lookup(input), &mut Vec::new(), input)
            .is_err());

        Ok(())
    }

    #[test]
    fn test_value() -> Result<()> {
        // Numbers
        for input in ["12", "-12", "-0.12", "-.12", "-.12f"].into_iter() {
            assert_eq!(
                rules::aidl::ValueParser::new().parse(&lookup(input), &mut Vec::new(), input)?,
                input
            );
        }

        // Invalid numbers
        for input in ["-.", "--12", "0..2", "0.2y"].into_iter() {
            assert!(rules::aidl::ValueParser::new()
                .parse(&lookup(input), &mut Vec::new(), input)
                .is_err());
        }

        // Strings
        for input in ["\"hello\"", "\"\"", "\"\t\""].into_iter() {
            assert_eq!(
                rules::aidl::ValueParser::new().parse(&lookup(input), &mut Vec::new(), input)?,
                input
            );
        }

        // Invalid strings
        for input in ["\"\"\""].into_iter() {
            assert!(rules::aidl::ValueParser::new()
                .parse(&lookup(input), &mut Vec::new(), input)
                .is_err());
        }

        // Empty objects
        for input in ["{}", "{ }", "{      }"].into_iter() {
            assert_eq!(
                rules::aidl::ValueParser::new().parse(&lookup(input), &mut Vec::new(), input)?,
                "{}"
            );
        }

        // Non-empty objects
        for input in ["{\"hello{<\"}", "{1}", "{1, 2}", "{1, 2, 3, }"].into_iter() {
            assert_eq!(
                rules::aidl::ValueParser::new().parse(&lookup(input), &mut Vec::new(), input)?,
                "{...}"
            );
        }

        // Invalid objects
        for input in ["{\"hello{<\"", "{1sfewf}", "{1, 2, 3,, }"].into_iter() {
            assert!(rules::aidl::ValueParser::new()
                .parse(&lookup(input), &mut Vec::new(), input)
                .is_err());
        }

        Ok(())
    }

    #[test]
    fn test_annotation1() -> Result<()> {
        let input = "@AnnotationName";
        assert_parser!(input, rules::aidl::OptAnnotationParser::new());

        Ok(())
    }

    #[test]
    fn test_annotation2() -> Result<()> {
        let input = "@AnnotationName()";
        assert_parser!(input, rules::aidl::OptAnnotationParser::new());

        Ok(())
    }

    #[test]
    fn test_annotation3() -> Result<()> {
        let input = "@AnnotationName( Hello)";
        assert_parser!(input, rules::aidl::OptAnnotationParser::new());

        Ok(())
    }

    #[test]
    fn test_annotation4() -> Result<()> {
        let input = "@AnnotationName(Hello=\"World\")";
        assert_parser!(input, rules::aidl::OptAnnotationParser::new());

        Ok(())
    }

    #[test]
    fn test_annotation5() -> Result<()> {
        let mut settings = insta::Settings::clone_current();
        settings.set_sort_maps(true);
        settings.bind_to_thread();

        let input = "@AnnotationName(Hello=\"World\", Hi, Servus= 3 )";
        assert_parser!(input, rules::aidl::OptAnnotationParser::new());

        Ok(())
    }

    #[test]
    fn test_reserved_keywords() -> Result<()> {
        let input = "package a.for.b;";
        assert!(rules::aidl::PackageParser::new()
            .parse(&lookup(input), &mut Vec::new(), input)
            .is_err());

        Ok(())
    }
}
