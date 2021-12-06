use crate::types;

peg::parser! {
    pub grammar rules(lookup: &'input line_col::LineColLookup) for str {
        use std::collections::HashMap;

        pub rule file() -> types::File
            = _ package:package() _
            imports:(import() ** _) _
            item:(interface() / enumeration() / parcelable()) _ {
            types::File {
                package,
                imports,
                item,
            }
        }

        pub rule package() -> types::Package
            = "package" whitespace_or_eol() _ p1:position!()
                name:$((ident() ".")* ident())
            p2:position!() _ ";" {
               types::Package {
                   name: name.to_owned(),
                   symbol_range: types::Range::new(lookup, p1, p2 - 1),
               }
            }

        pub rule import() -> types::Import
            = "import" whitespace_or_eol() _ p1:position!()
                name:$((ident() ".")* ident())
            p2:position!() _ ";" {
               types::Import {
                   name: name.to_owned(),
                   symbol_range: types::Range::new(lookup, p1, p2 - 1),
               }
            }

        pub rule interface() -> types::Item
            = jd:javadoc()? _ annotations:annotations() _ fp1:position!() "interface" whitespace_or_eol() _
            sp1:position!() name:$((ident() ".")* ident()) sp2:position!()
            _ "{" _ elements:(method() / constant())* _ "}"
            fp2:position!() {
               types::Item::Interface(types::Interface {
                   name: name.to_owned(),
                   elements,
                   annotations,
                   doc: jd,
                   full_range: types::Range::new(lookup, fp1, fp2 - 1),
                   symbol_range: types::Range::new(lookup, sp1, sp2 - 1),
               })
            }

        pub rule parcelable() -> types::Item
            = jd:javadoc()? annotations:annotations() _ fp1:position!() "parcelable" whitespace_or_eol() _
            sp1:position!() name:$((ident() ".")* ident()) sp2:position!()
            _ "{" _ members:member()* _ "}"
            fp2:position!() {
               types::Item::Parcelable(types::Parcelable {
                   name: name.to_owned(),
                   members,
                   annotations,
                   doc: jd,
                   full_range: types::Range::new(lookup, fp1, fp2 - 1),
                   symbol_range: types::Range::new(lookup, sp1, sp2 - 1),
               })
            }

        pub rule enumeration() -> types::Item
            = jd:javadoc()? _ annotations:annotations() _ fp1:position!() "enum" whitespace_or_eol() _
            sp1:position!() name:$((ident() ".")* ident()) sp2:position!() _
            _ "{" _ elements:(enum_element() ++ (_ "," _)) _ ","? _ "}"
            fp2:position!() {
               types::Item::Enum(types::Enum {
                   name: name.to_owned(),
                   elements,
                   annotations,
                   doc: jd,
                   full_range: types::Range::new(lookup, fp1, fp2 - 1),
                   symbol_range: types::Range::new(lookup, sp1, sp2 - 1),
               })
            }

        pub rule method() -> types::InterfaceElement
            = jd:javadoc() ? _ annotations:annotations() _ fp1:position!() _ oneway:("oneway" whitespace_or_eol())? _ rt:type_() _ sp1:position!() name:ident() sp2:position!() _
            "(" _ args:(method_arg() ** (_ "," _)) _ ","? _ ")" _
            ("=" _ digit()+)? _
            ";" _ fp2:position!() {
            types::InterfaceElement::Method(types::Method {
                oneway: oneway.is_some(),
                name: name.to_owned(),
                return_type: rt,
                args,
                annotations,
                doc: jd,
                symbol_range: types::Range::new(lookup, sp1, sp2),
                full_range: types::Range::new(lookup, fp1, fp2),
            })
        }

        pub rule method_arg() -> types::Arg = method_arg_with_name() / method_arg_without_name()
        pub rule method_arg_with_name() -> types::Arg
            = jd:javadoc()? _ annotations:annotations() _ d:direction()? _ t:type_() whitespace_or_eol() _ n:ident() {
            types::Arg {
                direction: d.unwrap_or(types::Direction::Unspecified),
                name: Some(n.to_owned()),
                arg_type: t,
                annotations,
                doc: jd,
            }
        }
        pub rule method_arg_without_name() -> types::Arg
            = jd:javadoc()? _ annotations:annotations() _ d:direction()? _ t:type_() {
            types::Arg {
                direction: d.unwrap_or(types::Direction::Unspecified),
                name: None,
                arg_type: t,
                annotations,
                doc: jd,
            }
        }

        pub rule member() -> types::Member
            = jd:javadoc()? _ annotations() _ fp1:position!() _ t:type_() _
            sp1:position!() name:ident() sp2:position!() _
            ("=" _ v:value())? _
            ";" _ fp2:position!() {
            types::Member {
                name: name.to_owned(),
                member_type: t,
                doc: jd,
                symbol_range: types::Range::new(lookup, sp1, sp2),
                full_range: types::Range::new(lookup, fp1, fp2),
            }
        }

        // Note: currently no check for the correct value type
        pub rule constant() -> types::InterfaceElement
            = jd:javadoc()? _ annotations:annotations() _ fp1:position!() "const" whitespace_or_eol() _ t:type_() _
            sp1:position!() name:ident() sp2:position!() _
            "=" _ v:value() _
            ";" _ fp2:position!() {
            types::InterfaceElement::Const(types::Const {
                name: name.to_owned(),
                const_type: t,
                value: v.to_owned(),
                annotations,
                doc: jd,
                symbol_range: types::Range::new(lookup, sp1, sp2),
                full_range: types::Range::new(lookup, fp1, fp2),
            })
        }

        pub rule enum_element() -> types::EnumElement
            = jd:javadoc()? _
            fp1:position!()
            sp1:position!() _ n:ident() sp2:position!() _
            ev:equals_value()?
            fp2:position!()
            {
                types::EnumElement {
                    name: n.to_owned(),
                    value: ev.map(str::to_owned),
                    doc: jd,
                    symbol_range: types::Range::new(lookup, sp1, sp2),
                    full_range: types::Range::new(lookup, fp1, fp2),
                }
            }

        pub rule type_() -> types::Type
            = type_array() / type_list() / type_map() / type_primitive() / type_void() / type_string() / type_custom()

        rule type_array() -> types::Type
            = p1:position!() t:(type_primitive() / type_custom()) _ "[" _ "]" p2:position!() {  // type_custom is tolerated because it could be an enum
            types::Type {
                name: "Array".to_owned(),
                kind: types::TypeKind::Array,
                generic_types: Vec::from([t]),
                definition: None,
                symbol_range: types::Range::new(lookup, p1, p2),
            }
        }

        rule type_list() -> types::Type
            = p1:position!() l:$"List" _ "<" _ t:type_object() _ ">" p2:position!() {  // type_custom is tolerated because it could be an enum
            types::Type {
                name: l.to_owned(),
                kind: types::TypeKind::List,
                generic_types: Vec::from([t]),
                definition: None,
                symbol_range: types::Range::new(lookup, p1, p2),
            }
        }

        rule type_map() -> types::Type
            = p1:position!() m:$"Map" _ "<" _ k:type_object() _ "," _ v:type_object() ">" p2:position!() {  // type_custom is tolerated because it could be an enum
            types::Type {
                name: m.to_owned(),
                kind: types::TypeKind::Map,
                generic_types: Vec::from([k, v]),
                definition: None,
                symbol_range: types::Range::new(lookup, p1, p2),
            }
        }

        rule type_void() -> types::Type
            = p1:position!() t:$"void" p2:position!() !ident_char() {
            types::Type::simple_type(t, types::TypeKind::Void, lookup, p1, p2)
        }

        rule type_primitive() -> types::Type
            = p1:position!() t:$("byte" / "short" / "int" / "long" / "float" / "double" / "boolean" / "char") p2:position!() !ident_char() {
            types::Type::simple_type(t, types::TypeKind::Primitive, lookup, p1, p2)
        }

        rule type_string() -> types::Type
            = p1:position!() t:$("String" / "CharSequence") p2:position!() !ident_char() {
            types::Type::simple_type(t, types::TypeKind::String, lookup, p1, p2)
        }

        rule type_custom() -> types::Type
            = !type_forbidden_custom() _ p1:position!() t:$((ident() ++ (_ "." _))) p2:position!() !ident_char() {
            types::Type::simple_type(t, types::TypeKind::Custom, lookup, p1, p2)
        }

        rule type_object() -> types::Type
            = !(type_array() / type_primitive()) _ t:type_() { t }

        rule type_forbidden_custom()
            = ("List" / "Map" / type_primitive() / type_void() / type_string()) !ident_char()

        rule direction() -> types::Direction
            = d:(direction_in() / direction_out() / direction_inout()) !ident_char() { d }

        rule direction_in() -> types::Direction = "in" { types::Direction::In }
        rule direction_out() -> types::Direction = "out" { types::Direction::Out }
        rule direction_inout() -> types::Direction = "inout" { types::Direction::InOut }

        pub rule annotations() -> Vec<types::Annotation> = annotation() ** _
        pub rule annotation() -> types::Annotation = annotation_with_params() / annotation_without_param()
        pub rule annotation_without_param() -> types::Annotation
            = "@" ident() {
            types::Annotation { key_values: HashMap::new() }
        }
        pub rule annotation_with_params() -> types::Annotation
            = "@" ident()
            _ "(" _ v:(annotation_param() ** (_ "," _)) _ ")" {
            types::Annotation { key_values: v.into_iter().collect() }
        }
        rule annotation_param() -> (String, Option<String>) = k:ident() v:equals_value()? {
            (k.to_owned(), v.map(str::to_owned))
        }

        pub rule value() -> &'input str
            = $(number_value() / value_string() / value_empty_object() / "null")
        rule number_value() -> &'input str = $(
            "-"? digit()* "." digit()+ "f"?  // with decimal point
            / "-"? digit()+ "f"?  // without decimal point
        )

        rule value_string() = "\"" (!"\"" [_])* "\""
        rule value_empty_object() = "{" _ "}"
        rule equals_value() -> &'input str = _ "=" _ v:value() { v }

        pub rule javadoc() -> String
            = javadoc_begin() _
            s:$(
                (!javadoc_end() [_])*
            ) _
            javadoc_end() {
            parse_javadoc(s)
        }
        rule javadoc_begin() = "/**";
        rule javadoc_end() = _ "*/";

        rule block_comment() -> &'input str
            = $(!(javadoc() _ (
                // All rules which extract the Javadoc:
                interface() / parcelable() / enumeration() / method() / method_arg() /
                member() / constant() / enum_element()
            )) "/*" (!"*/" [_])* "*/")
        rule line_comment() -> &'input str = s:$(quiet!{
            "//" (!(['\n' | '\r']) [_])*
        }) { s }
        rule whitespace() = quiet!{[ ' ' | '\t' ]}
        rule whitespace_or_eol() = quiet!{[ ' ' | '\n' | '\r' | '\t' ]}
        rule comment() = quiet!{block_comment() / line_comment()}
        rule _ = quiet!{(whitespace_or_eol() / comment())*}
        rule eol() = quiet!{"\n" / "\r\n"}

        rule digit() = ['0'..='9']
        rule alphanumeric() = ['a'..='z' | 'A'..='Z' | '0'..='9']
        rule ident_first_char() = (['a'..='z' | 'A'..='Z'] / "_")
        rule ident_char() = alphanumeric() / "_"
        rule ident() -> &'input str = $(ident_first_char() ident_char()*) / expected!("identifier")
    }
}

fn parse_javadoc(s: &str) -> String {
    // Transform into vec
    let re = regex::Regex::new("\r?\n[ \t*]*\r?\n").unwrap();
    let lines = re.split(s);

    // Remove begin/end noise of each line
    let re = regex::Regex::new("[ \t\r\n*]*\n[ \t\r\n*]*").unwrap();
    let lines = lines.map(|s| {
        let s = s.trim_matches(|c| c == '\r' || c == '\n' || c == ' ' || c == '\t' || c == '*');
        re.replace_all(s, " ").to_string()
    });

    // Add \n before @
    let re = regex::Regex::new("([^\n])[ \t]*@").unwrap();
    let lines = lines.map(|s| re.replace_all(&s, "${1}\n@").to_string());

    lines.collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    fn lookup(input: &str) -> line_col::LineColLookup {
        line_col::LineColLookup::new(input)
    }

    macro_rules! assert_rule {
        ($input:ident, $rule:expr) => {
            ::insta::assert_ron_snapshot!($rule($input, &lookup($input))?, {
                ".**.symbol_range" => "...",
                ".**.full_range" => "...",
            });
        };

        ($input:ident, $rule:expr, @$snap:literal) => {
            ::insta::assert_ron_snapshot!($rule($input, &lookup($input))?, {
                ".**.symbol_range" => "...",
                ".**.full_range" => "...",
            }, @$snap);
        };
    }

    #[test]
    fn test_package1() -> Result<()> {
        let input = "package x ;";
        assert_rule!(input, rules::package);

        Ok(())
    }

    #[test]
    fn test_package2() -> Result<()> {
        let input = "package x.y.z;";
        assert_rule!(input, rules::package);

        Ok(())
    }

    #[test]
    fn test_import() -> Result<()> {
        let input = "import x.y.z;";
        assert_rule!(input, rules::import);

        Ok(())
    }

    #[test]
    fn test_interface() -> Result<()> {
        let input = r#"interface Potato {
            /**
             * const1 docu
             */
            const int const1 = 1;

            /**
             * method1 docu
             */
            String method1();

            const String const2 = "two";
            int method2();
        }"#;
        assert_rule!(input, rules::interface);

        Ok(())
    }

    #[test]
    fn test_interface_with_annotation() -> Result<()> {
        let input = r#"@InterfaceAnnotation1
            @InterfaceAnnotation2 interface Potato {
            }"#;
        assert_rule!(input, rules::interface);

        Ok(())
    }

    #[test]
    fn test_interface_error_inside() -> Result<()> {
        let input = r#"interface Potato {
            String method1();
            completly_unexpected;
            int method2();
        }"#;
        assert!(rules::interface(input, &lookup(input)).is_err());

        Ok(())
    }

    #[test]
    fn test_parcelable() -> Result<()> {
        let input = r#"parcelable Tomato {
            /**
             * member1 docu
             */
            int member1;

            String member2; // inline comment
        }"#;
        assert_rule!(input, rules::parcelable);

        Ok(())
    }

    #[test]
    fn test_enum() -> Result<()> {
        let input = r#"enum Paprika {
            /**
             * element1 docu
             */
            ELEMENT1 = 3,

            ELEMENT2 = "quattro",
            ELEMENT3
        }"#;
        assert_rule!(input, rules::enumeration);

        Ok(())
    }

    #[test]
    fn test_enum_with_trailing_comma() -> Result<()> {
        let input = r#"enum Paprika {
            ELEMENT1,
            ELEMENT2,
        }"#;
        assert_rule!(input, rules::enumeration);

        Ok(())
    }

    #[test]
    fn test_method_without_arg() -> Result<()> {
        let input = "TypeName myMethod() ;";
        assert_rule!(input, rules::method);

        Ok(())
    }

    #[test]
    fn test_method_with_1_arg() -> Result<()> {
        let input = "TypeName myMethod(ArgType arg) ;";
        assert_rule!(input, rules::method);

        Ok(())
    }

    #[test]
    fn test_method_with_3_args() -> Result<()> {
        let input = "TypeName myMethod(ArgType1, ArgType2 arg2, ArgType3) ;";
        assert_rule!(input, rules::method);

        Ok(())
    }

    #[test]
    fn test_method_oneway() -> Result<()> {
        let input = "oneway TypeName myMethod();";
        assert_rule!(input, rules::method);

        Ok(())
    }

    #[test]
    fn test_method_with_value() -> Result<()> {
        let input = "TypeName myMethod() = 123;";
        assert_rule!(input, rules::method);

        Ok(())
    }

    #[test]
    fn test_method_with_annotation() -> Result<()> {
        let input = "@AnnotationName void myMethod();";
        assert_rule!(input, rules::method);

        Ok(())
    }

    #[test]
    fn test_method_with_javadoc() -> Result<()> {
        let input = r#"/**
         * Method docu
         */
         void myMethod() = 123;"#;

        assert_rule!(input, rules::method);
        Ok(())
    }

    #[test]
    fn test_method_arg_with_name() -> Result<()> {
        let input = "TypeName albert";
        assert_rule!(input, rules::method_arg);

        Ok(())
    }

    #[test]
    fn test_method_arg_with_direction() -> Result<()> {
        let input = "in TypeName";
        assert_rule!(input, rules::method_arg);

        Ok(())
    }

    #[test]
    fn test_method_arg_with_direction_and_name() -> Result<()> {
        let input = "out TypeName roger";
        assert_rule!(input, rules::method_arg);

        Ok(())
    }

    #[test]
    fn test_method_arg_with_annotations() -> Result<()> {
        let input = r#"@Annotation1
            @Annotation2(AnnotationParam ) TypeName albert"#;
        assert_rule!(input, rules::method_arg);

        Ok(())
    }

    #[test]
    fn test_member() -> Result<()> {
        let input = "TypeName memberName ;";
        assert_rule!(input, rules::member);
        Ok(())
    }

    #[test]
    fn test_member_with_value() -> Result<()> {
        let input = "TypeName memberName = \"member value\";";
        assert_rule!(input, rules::member);

        Ok(())
    }

    #[test]
    fn test_member_with_javadoc() -> Result<(), Box<dyn std::error::Error>> {
        let input = r#"/**
             * Member docu
             */
            TypeName memberName;"#;
        assert_rule!(input, rules::member);

        Ok(())
    }

    #[test]
    fn test_member_with_annotation() -> Result<()> {
        let input = "@AnnotationName TypeName memberName = \"member value\";";
        assert_rule!(input, rules::member);

        Ok(())
    }

    #[test]
    fn test_const_num() -> Result<()> {
        let input = "const int CONST_NAME = 123 ;";
        assert_rule!(input, rules::constant);

        Ok(())
    }

    #[test]
    fn test_const_string() -> Result<()> {
        let input = "const TypeName CONST_NAME = \"const value\";";
        assert_rule!(input, rules::constant);

        Ok(())
    }

    #[test]
    fn test_const_with_javadoc() -> Result<()> {
        let input = r#"/**
            * Const docu
            */
           const TypeName CONST_NAME = 123;"#;
        assert_rule!(input, rules::constant);

        Ok(())
    }

    #[test]
    fn test_const_with_annotation() -> Result<()> {
        let input = "@AnnotationName const TypeName CONST_NAME = 123;";
        assert_rule!(input, rules::constant);

        Ok(())
    }

    #[test]
    fn test_type_primitive1() -> Result<()> {
        let input = "double";
        assert!(rules::type_(input, &lookup(input))?.kind == types::TypeKind::Primitive);
        assert_rule!(input, rules::type_);

        Ok(())
    }

    #[test]
    fn test_type_primitive2() -> Result<()> {
        let input = "doublegum";
        assert!(rules::type_(input, &lookup(input))?.kind != types::TypeKind::Primitive);
        assert_rule!(input, rules::type_);

        Ok(())
    }

    #[test]
    fn test_type_custom() -> Result<()> {
        let input = "TypeName";
        assert!(rules::type_(input, &lookup(input))?.kind == types::TypeKind::Custom);
        assert_rule!(input, rules::type_);

        Ok(())
    }

    #[test]
    fn test_type_custom_with_namespace() -> Result<()> {
        let input = "com.example.TypeName";
        assert_eq!(
            rules::type_(input, &lookup(input))?.kind,
            types::TypeKind::Custom
        );
        assert_rule!(input, rules::type_);

        Ok(())
    }

    #[test]
    fn test_type_array() -> Result<()> {
        let input = "float []";
        assert_eq!(
            rules::type_(input, &lookup(input))?.kind,
            types::TypeKind::Array
        );
        assert_rule!(input, rules::type_);

        // No array of String...
        let input = "String []";
        assert!(rules::type_(input, &lookup(input)).is_err());

        Ok(())
    }

    #[test]
    fn test_type_list() -> Result<()> {
        let input = "List <MyObject >";
        assert_eq!(
            rules::type_(input, &lookup(input))?.kind,
            types::TypeKind::List
        );
        assert_rule!(input, rules::type_);

        // No List for type_primitives
        let input = "List<int>";
        assert!(rules::type_(input, &lookup(input)).is_err());

        Ok(())
    }

    #[test]
    fn test_type_map() -> Result<()> {
        let input = "Map<Key,List<V>>";
        assert_eq!(
            rules::type_(input, &lookup(input))?.kind,
            types::TypeKind::Map
        );
        assert_rule!(input, rules::type_);

        // No Map for type_primitives
        let input = "Map<int, String>";
        assert!(rules::type_(input, &lookup(input)).is_err());
        let input = "Map<String, int>";
        assert!(rules::type_(input, &lookup(input)).is_err());

        // OK for objects
        let input = "Map<String, String>";
        assert!(rules::type_(input, &lookup(input)).is_ok());

        Ok(())
    }

    #[test]
    fn test_value() -> Result<()> {
        // Numbers
        for input in ["12", "-12", "-0.12", "-.12", "-.12f"].into_iter() {
            assert_eq!(rules::value(input, &lookup(input))?, input);
        }

        // Invalid numbers
        for input in ["-.", "--12", "0..2", "0.2y"].into_iter() {
            assert!(rules::value(input, &lookup(input)).is_err());
        }

        // Strings
        for input in ["\"hello\"", "\"\"", "\"\n\""].into_iter() {
            assert_eq!(rules::value(input, &lookup(input))?, input);
        }

        // Invalid strings
        for input in ["\"\"\""].into_iter() {
            assert!(rules::value(input, &lookup(input)).is_err());
        }

        Ok(())
    }

    #[test]
    fn test_javadoc() -> Result<(), Box<dyn std::error::Error>> {
        let input = "/** This is a javadoc\n * comment*/";
        assert_eq!(
            rules::javadoc(input, &lookup(input))?,
            "This is a javadoc comment"
        );

        let input = "/**\n * JavaDoc title\n *\n * JavaDoc text1\n * JavaDoc text2\n*/";
        assert_eq!(
            rules::javadoc(input, &lookup(input))?,
            "JavaDoc title\nJavaDoc text1 JavaDoc text2"
        );

        let input = r#"/**
                * JavaDoc title
                * @param Param1 Description
                * @param Param2 Description
                * 
                * Description
                */"#;
        assert_eq!(
            rules::javadoc(input, &lookup(input))?,
            "JavaDoc title\n@param Param1 Description\n@param Param2 Description\nDescription"
        );

        Ok(())
    }

    #[test]
    fn test_annotation1() -> Result<()> {
        let input = "@AnnotationName";
        assert_rule!(input, rules::annotation);

        Ok(())
    }

    #[test]
    fn test_annotation2() -> Result<()> {
        let input = "@AnnotationName()";
        assert_rule!(input, rules::annotation);

        Ok(())
    }

    #[test]
    fn test_annotation3() -> Result<()> {
        let input = "@AnnotationName( Hello)";
        assert_rule!(input, rules::annotation);

        Ok(())
    }

    #[test]
    fn test_annotation4() -> Result<()> {
        let input = "@AnnotationName(Hello=\"World\")";
        assert_rule!(input, rules::annotation);

        Ok(())
    }

    #[test]
    fn test_annotation5() -> Result<()> {
        let mut settings = insta::Settings::clone_current();
        settings.set_sort_maps(true);
        settings.bind_to_thread();

        let input = "@AnnotationName(Hello=\"World\", Hi, Servus= 3 )";
        assert_rule!(input, rules::annotation);

        Ok(())
    }
}
