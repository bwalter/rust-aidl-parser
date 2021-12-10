use crate::diagnostic::{Diagnostic, DiagnosticKind};
use crate::types;

peg::parser! {
    pub(crate) grammar rules(lookup: &'input line_col::LineColLookup, diagnostics: &mut Vec<Diagnostic>) for str {
        use peg::ParseLiteral;
        use std::collections::HashMap;

        pub(crate) rule file() -> Option<types::File> =
            _ package:package() _
            imports:(import() ** _) _
            item:(item_interface() / item_parcelable() / item_enum() / item_fallback()) _ {

            item.map(|item| {
                types::File {
                    package,
                    imports,
                    item,
                }
            })
        }

        pub(crate) rule package() -> types::Package =
            "package" whitespace_or_eol() _ p1:position!()
            name:qualified_name()
            p2:position!() _ ";" {

            types::Package {
                name: name.to_owned(),
                symbol_range: types::Range::new(lookup, p1, p2 - 1),
            }
        }

        pub(crate) rule import() -> types::Import =
            "import" whitespace_or_eol() _ p1:position!()
            name:qualified_name()
            p2:position!() _ ";" {

            types::Import {
                name: name.to_owned(),
                symbol_range: types::Range::new(lookup, p1, p2 - 1),
            }
        }

        rule item_interface() -> Option<types::Item> =
            i:interface() {

            Some(types::Item::Interface(i))
        }

        rule item_parcelable() -> Option<types::Item> =
            p:parcelable() {

            Some(types::Item::Parcelable(p))
        }

        rule item_enum() -> Option<types::Item> =
            e:enum_() {

            Some(types::Item::Enum(e))
        }

        rule item_fallback() -> Option<types::Item> =
            p1:position!() [^'{']* "{" [^'}']* "}" p2:position!() {

            diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Error,
                text: "Invalid file item (expected valid \"interface\", \"parcelable\" or \"enum\")".into(),
                range: types::Range::new(lookup, p1, p2),
            });
            None
        }

        pub(crate) rule interface() -> types::Interface
            = jd:javadoc()? _ annotations:annotations() _ fp1:position!() "interface" whitespace_or_eol() _
            sp1:position!() name:$ident() sp2:position!()
            _ "{" _ elements:interface_element_any()* _ "}"
            fp2:position!() {

                let elements: Vec<types::InterfaceElement> = elements.into_iter().flatten().collect();

                types::Interface {
                    name: name.to_owned(),
                    elements,
                    annotations,
                    doc: jd,
                    full_range: types::Range::new(lookup, fp1, fp2 - 1),
                    symbol_range: types::Range::new(lookup, sp1, sp2 - 1),
                }
            }

        // Accept anything which is an interface element and handle inner errors via diagnostics
        rule interface_element_any() -> Option<types::InterfaceElement> =
            s:$(javadoc()? parse_element_until(";") ";") _ {

            interface_element(s, lookup, diagnostics)
                .unwrap_or_else(|e| {
                    // Here we are sure that it is an invalid method because interface_element_const_any()
                    // does not fail for consts
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        text: format!("Invalid method: expected {}", e.expected.to_string()),
                        range: types::Range::new(lookup, e.location.line, e.location.column),
                    });
                    None
                })
        }

        pub(crate) rule interface_element() -> Option<types::InterfaceElement> =
            interface_element_const_any() / interface_element_method()

        // Accept anything which is a const and handle inner errors via diagnostics
        rule interface_element_const_any() -> Option<types::InterfaceElement> =
            s:$((javadoc() _)? "const" &whitespace_or_eol() _ parse_element_until(";") ";") {

            const_(s, lookup, diagnostics)
                .map(|c| Some(types::InterfaceElement::Const(c)))
                .unwrap_or_else(|e| {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        text: format!("Invalid const: expected {}", e.expected.to_string()),
                        range: types::Range::new(lookup, e.location.line, e.location.column),
                    });
                    None
                })
        }

        rule interface_element_method() -> Option<types::InterfaceElement> = m:method() {
            Some(types::InterfaceElement::Method(m))
        }

        pub(crate) rule parcelable() -> types::Parcelable =
            jd:javadoc()? annotations:annotations() _ fp1:position!() "parcelable" whitespace_or_eol() _
            sp1:position!() name:$ident() sp2:position!()
            _ "{" _ members:parcelable_member_any()* _ "}"
            fp2:position!() {

                let members: Vec<types::Member> = members.into_iter().flatten().collect();

                types::Parcelable {
                    name: name.to_owned(),
                    members,
                    annotations,
                    doc: jd,
                    full_range: types::Range::new(lookup, fp1, fp2 - 1),
                    symbol_range: types::Range::new(lookup, sp1, sp2 - 1),
                }
            }

        // Accept anything which is a member and handle inner errors via diagnostics
        rule parcelable_member_any() -> Option<types::Member> =
            p1:position!() s:$(parse_element_until(";") ";") p2:position!() _ {

                member(s, lookup, diagnostics)
                .map(Some)
                .unwrap_or_else(|e| {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        text: format!("Invalid member: expected {}", e.expected.to_string()),
                        range: types::Range::new(lookup, p1 + e.location.offset, p1 + e.location.offset),
                    });
                    None
                })
        }

        pub(crate) rule enum_() -> types::Enum =
            jd:javadoc()? _ annotations:annotations() _ fp1:position!() "enum" whitespace_or_eol() _
            sp1:position!() name:$ident() sp2:position!() _
            _ "{"
                _ elements:enum_element_any() ** (_ "," _)
            _ ","?  _ "}"
            fp2:position!() {

            let elements: Vec<types::EnumElement> = elements.into_iter().flatten().collect();

            types::Enum {
                name: name.to_owned(),
                elements,
                annotations,
                doc: jd,
                full_range: types::Range::new(lookup, fp1, fp2 - 1),
                symbol_range: types::Range::new(lookup, sp1, sp2 - 1),
            }
        }

        // Accept anything which is an enum element and handle inner errors via diagnostics
        rule enum_element_any() -> Option<types::EnumElement> =
            s:$parse_element_until(",") {

            enum_element(s, lookup, diagnostics)
                .map(Some)
                .unwrap_or_else(|e| {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        text: format!("Invalid enum element: expected {}", e.expected.to_string()),
                        range: types::Range::new(lookup, e.location.line, e.location.column),
                    });
                    None
                })
        }

        pub(crate) rule method() -> types::Method =
            jd:javadoc() ? _ annotations:annotations() _ fp1:position!() _ oneway:("oneway" whitespace_or_eol())? _ rt:type_any() _ sp1:position!() name:ident() sp2:position!() _
            "(" _ args:(method_arg() ** (_ "," _)) _ ","? _ ")" _
            ("=" _ digit()+)? _
            ";" _ fp2:position!() {

            types::Method {
                oneway: oneway.is_some(),
                name: name.to_owned(),
                return_type: rt,
                args,
                annotations,
                doc: jd,
                symbol_range: types::Range::new(lookup, sp1, sp2),
                full_range: types::Range::new(lookup, fp1, fp2),
            }
        }

        pub(crate) rule method_arg() -> types::Arg = method_arg_with_name() / method_arg_without_name()
        pub(crate) rule method_arg_with_name() -> types::Arg =
            jd:javadoc()? _ annotations:annotations() _ d:direction()? _ t:type_any() whitespace_or_eol() _ n:ident() {

            types::Arg {
                direction: d.unwrap_or(types::Direction::Unspecified),
                name: Some(n.to_owned()),
                arg_type: t,
                annotations,
                doc: jd,
            }
        }
        pub(crate) rule method_arg_without_name() -> types::Arg =
            jd:javadoc()? _ annotations:annotations() _ d:direction()? _ t:type_any() {

            types::Arg {
                direction: d.unwrap_or(types::Direction::Unspecified),
                name: None,
                arg_type: t,
                annotations,
                doc: jd,
            }
        }

        pub(crate) rule member() -> types::Member =
            jd:javadoc()? _ annotations() _ fp1:position!() _ t:type_any() _
            sp1:position!() name:ident() sp2:position!() _
            ("=" _ v:value())? _
            ";" fp2:position!() {

            types::Member {
                name: name.to_owned(),
                member_type: t,
                doc: jd,
                symbol_range: types::Range::new(lookup, sp1, sp2),
                full_range: types::Range::new(lookup, fp1, fp2),
            }
        }

        pub(crate) rule const_() -> types::Const =
            jd:javadoc()? _ annotations:annotations() _ fp1:position!() "const" whitespace_or_eol() _ t:type_any() _
            sp1:position!() name:ident() sp2:position!() _
            "=" _ v:value() _
            ";" _ fp2:position!() {

            types::Const {
                name: name.to_owned(),
                const_type: t,
                value: v.to_owned(),
                annotations,
                doc: jd,
                symbol_range: types::Range::new(lookup, sp1, sp2),
                full_range: types::Range::new(lookup, fp1, fp2),
            }
        }

        pub(crate) rule enum_element() -> types::EnumElement =
            jd:javadoc()? _
            fp1:position!()
            sp1:position!() _ n:ident() sp2:position!() _
            ev:equals_value()?// &(_ ("," / "}"))
            fp2:position!() {

            types::EnumElement {
                name: n.to_owned(),
                value: ev.map(str::to_owned),
                doc: jd,
                symbol_range: types::Range::new(lookup, sp1, sp2),
                full_range: types::Range::new(lookup, fp1, fp2),
            }
        }

        // Parse either valid type or fallback to an "InvalidType" (with diagnostic)
        pub(crate) rule type_any() -> types::Type =
            p1:position!() s:$(
                // Examples of recognized, invalid types:
                // - woof
                // - a.qualified.name
                // - freg.rgerger. <fefe ,fef > [ ]
                ((ident_char()) / ['.'] / (_ generic()) / (_ square_brackets()))+
            ) p2:position!() &(!ident_char() / ![_]) {

            type_(s, lookup, diagnostics)
                .unwrap_or_else(|e| {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        text: "Invalid type".into(),
                        range: types::Range::new(lookup, p1, p2),
                    });

                    types::Type {
                        name: s.to_owned(),
                        kind: types::TypeKind::Invalid,
                        generic_types: Vec::new(),
                        definition: None,
                        symbol_range: types::Range::new(lookup, p1, p2),
                    }
                })
        }

        pub(crate) rule type_() -> types::Type =
            t:(type_array_any() / type_list_any() / type_map_any() / type_primitive() / type_void() / type_string() / type_custom())
            (whitespace_or_eol() / ![_]) { t }

        // Parse either valid array type or fallback to an "InvalidType" (with diagnostic)
        rule type_array_any() -> types::Type =
            p1:position!() s:$(
                (ident_char() / (_ ['.'] _))+ _ "[" _ "]"
            ) p2:position!() &(whitespace_or_eol() / ![_]) {

            type_array(s, lookup, diagnostics)
                .unwrap_or_else(|e| {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        text: "Invalid array: only primitives or enums are allowed. For objects, please use List<Object>".into(),
                        range: types::Range::new(lookup, p1, p2),
                    });

                    types::Type {
                        name: s.to_owned(),
                        kind: types::TypeKind::Invalid,
                        generic_types: Vec::new(),
                        definition: None,
                        symbol_range: types::Range::new(lookup, p1, p2),
                    }
                })
        }

        pub(crate) rule type_array() -> types::Type =
            p1:position!() t:(type_primitive() / type_custom()) _ "[" _ "]" p2:position!() {  // type_custom is tolerated because it could be an enum
            types::Type {
                name: "Array".to_owned(),
                kind: types::TypeKind::Array,
                generic_types: Vec::from([t]),
                definition: None,
                symbol_range: types::Range::new(lookup, p1, p2),
            }
        }

        // Parse either valid list type or fallback to an "InvalidType" (with diagnostic)
        rule type_list_any() -> types::Type =
            p1:position!() s:$("List" _ generic()) p2:position!() {

            type_list(s, lookup, diagnostics)
                .unwrap_or_else(|e| {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        text: "Invalid list: the value must be an object (not a primitive)".into(),
                        range: types::Range::new(lookup, p1, p2),
                    });

                    types::Type {
                        name: s.to_owned(),
                        kind: types::TypeKind::Invalid,
                        generic_types: Vec::new(),
                        definition: None,
                        symbol_range: types::Range::new(lookup, p1, p2),
                    }
                })
        }

        pub(crate) rule type_list() -> types::Type =
            p1:position!() l:$"List" _ "<" _ t:type_object() _ ">" p2:position!() {  // type_custom is tolerated because it could be an enum
            types::Type {
                name: l.to_owned(),
                kind: types::TypeKind::List,
                generic_types: Vec::from([t]),
                definition: None,
                symbol_range: types::Range::new(lookup, p1, p2),
            }
        }

        // Parse either valid map type or fallback to an "InvalidType" (with diagnostic)
        rule type_map_any() -> types::Type =
            p1:position!() s:$("Map" _ generic()) p2:position!() {

            type_map(s, lookup, diagnostics)
                .unwrap_or_else(|e| {
                    diagnostics.push(Diagnostic {
                        kind: DiagnosticKind::Error,
                        text: "Invalid map: both key and value must be an object (not a primitive)".into(),
                        range: types::Range::new(lookup, p1, p2),
                    });

                    types::Type {
                        name: s.to_owned(),
                        kind: types::TypeKind::Invalid,
                        generic_types: Vec::new(),
                        definition: None,
                        symbol_range: types::Range::new(lookup, p1, p2),
                    }
                })
        }

        pub(crate) rule type_map() -> types::Type =
            p1:position!() m:$"Map" _ "<" _ k:type_object() _ "," _ v:type_object() ">" p2:position!() {  // type_custom is tolerated because it could be an enum

            types::Type {
                name: m.to_owned(),
                kind: types::TypeKind::Map,
                generic_types: Vec::from([k, v]),
                definition: None,
                symbol_range: types::Range::new(lookup, p1, p2),
            }
        }

        rule type_void() -> types::Type =
           p1:position!() t:$"void" p2:position!() !ident_char() {
            types::Type::simple_type(t, types::TypeKind::Void, lookup, p1, p2)
        }

        rule type_primitive() -> types::Type =
            p1:position!() t:$("byte" / "short" / "int" / "long" / "float" / "double" / "boolean" / "char") p2:position!() !ident_char() {
            types::Type::simple_type(t, types::TypeKind::Primitive, lookup, p1, p2)
        }

        rule type_string() -> types::Type =
            p1:position!() t:$("String" / "CharSequence") p2:position!() !ident_char() {
            types::Type::simple_type(t, types::TypeKind::String, lookup, p1, p2)
        }

        rule type_custom() -> types::Type =
            !(type_forbidden_custom() _) p1:position!() t:qualified_name() p2:position!() !ident_char() {
            types::Type::simple_type(t, types::TypeKind::Custom, lookup, p1, p2)
        }

        rule type_object() -> types::Type =
            t:(type_string() / type_custom() / type_list_any() / type_map_any()) { t }

        rule type_forbidden_custom() =
            ("List" / "Map" / type_primitive() / type_void() / type_string()) !ident_char()

        rule direction() -> types::Direction =
            d:(direction_in() / direction_out() / direction_inout()) !ident_char() { d }

        rule direction_in() -> types::Direction = "in" { types::Direction::In }
        rule direction_out() -> types::Direction = "out" { types::Direction::Out }
        rule direction_inout() -> types::Direction = "inout" { types::Direction::InOut }

        pub(crate) rule annotations() -> Vec<types::Annotation> = annotation() ** _
        pub(crate) rule annotation() -> types::Annotation = annotation_with_params() / annotation_without_param()
        pub(crate) rule annotation_without_param() -> types::Annotation =
            "@" ident() {
            types::Annotation { key_values: HashMap::new() }
        }
        pub(crate) rule annotation_with_params() -> types::Annotation =
            "@" ident()
            _ "(" _ v:(annotation_param() ** (_ "," _)) _ ")" {
            types::Annotation { key_values: v.into_iter().collect() }
        }
        rule annotation_param() -> (String, Option<String>) = k:ident() v:equals_value()? {
            (k.to_owned(), v.map(str::to_owned))
        }

        pub(crate) rule generic() = "<" _ (block_comment() / (!("<" / ">") [_]) / generic())* _ ">"
        pub(crate) rule square_brackets() = "[" _ "]"

        pub(crate) rule parse_element_until(without: &str) -> &'input str = $((
            value_string() / line_comment() / javadoc() / block_comment() / generic() / square_brackets() / whitespace_or_eol() /
                (!(##parse_string_literal(without) / "}") [_])
        )+)
        rule semi_column() -> &'input str = $";"
        rule comma() -> &'input str = $","

        pub(crate) rule value() -> &'input str =
            $(number_value() / value_string() / value_empty_object() / "null") / expected!("value (number, string or empty object)")
        rule number_value() -> &'input str = $(
            "-"? digit()* "." digit()+ "f"?  // with decimal point
            / "-"? digit()+ "f"?  // without decimal point
        )

        rule value_string() = "\"" (!['"' | '\n' | '\r'] [_])* "\""
        rule value_empty_object() = "{" _ "}"
        rule equals_value() -> &'input str = _ "=" _ v:value() { v }

        pub(crate) rule javadoc() -> String =
            javadoc_begin() _
            s:$(
                (!javadoc_end() [_])*
            ) _
            javadoc_end() {
            parse_javadoc(s)
        }
        rule javadoc_begin() = "/**";
        rule javadoc_end() = _ "*/";

        rule block_comment() -> &'input str =
            quiet!{$(!(javadoc() _ (
                // All rules which extract the Javadoc:
                interface() / parcelable() / enum_() / method() / method_arg() /
                interface_element_any() / parcelable_member_any() / enum_element_any()
            )) "/*" (!"*/" [_])* "*/")}
        rule line_comment() -> &'input str = s:$(quiet!{
            "//" (!(['\n' | '\r']) [_])*
        }) { s }
        rule whitespace() = quiet!{[ ' ' | '\t' ]}
        rule whitespace_or_eol() = quiet!{[ ' ' | '\n' | '\r' | '\t']}
        rule comment() = quiet!{block_comment() / line_comment()}
        rule _ = quiet!{(whitespace_or_eol() / comment())*}
        rule eol() = quiet!{"\n" / "\r\n"}

        rule digit() = quiet!{['0'..='9']}
        rule alphanumeric() = quiet!{['a'..='z' | 'A'..='Z' | '0'..='9']}
        rule ident_first_char() = quiet!{(['a'..='z' | 'A'..='Z'] / "_")}
        rule ident_char() = quiet!{alphanumeric() / "_"}
        rule ident() -> &'input str = $(ident_first_char() ident_char()*) / expected!("identifier")
        rule qualified_name() -> &'input str = $(ident() ++ (_ "." _))
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

    // Replace ranges into "..." and check parse output via insta Ron snapshot
    macro_rules! assert_rule {
        ($input:ident, $rule:expr) => {
            let mut diagnostics = Vec::new();
            ::insta::assert_ron_snapshot!($rule($input, &lookup($input), &mut diagnostics)?, {
                ".**.symbol_range" => "...",
                ".**.full_range" => "...",
            });
            $rule($input, &lookup($input), &mut diagnostics)?;
            assert_eq!(diagnostics, &[]);
        };

        ($input:ident, $rule:expr, $diag:expr) => {
            ::insta::assert_ron_snapshot!($rule($input, &lookup($input), $diag)?, {
                ".**.symbol_range" => "...",
                ".**.full_range" => "...",
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
    fn test_file() -> Result<()> {
        let input = r#"package x.y.z;
            import a.b.c;
            interface MyInterface {}
        "#;
        assert_rule!(input, rules::file);

        Ok(())
    }

    #[test]
    fn test_file_with_errors() -> Result<()> {
        let input = r#"package x.y.z;
            import a.b.c;
            oops_interface MyInterface {}
        "#;
        let mut diagnostics = Vec::new();
        assert_eq!(rules::file(input, &lookup(input), &mut diagnostics)?, None);

        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            text: "Invalid file item (expected valid \"interface\", \"parcelable\" or \"enum\")",
          ),
        ]
        "###);

        Ok(())
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
    fn test_interface_with_errors() -> Result<()> {
        let input = r#"interface Potato {
            String method1();
            int method2();
            int oops_not_a_valid_method;
            const String const2 = 123;
            const oops_not_a_valid_const;
        }"#;
        let mut diagnostics = Vec::new();
        assert_rule!(input, rules::interface, &mut diagnostics);
        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            text: "Invalid method: expected \"(\"",
          ),
          Diagnostic(
            kind: Error,
            range: "...",
            text: "Invalid const: expected one of \"<\", \"[\", [\'.\'], identifier",
          ),
        ]
        "###);

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
    fn test_parcelable_with_errors() -> Result<()> {
        let input = r#"parcelable Tomato {
            int member1;
            wrongmember3;
            String member3;
        }"#;
        let mut diagnostics = Vec::new();
        assert_rule!(input, rules::parcelable, &mut diagnostics);
        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            text: "Invalid member: expected one of \"<\", \"[\", [\'.\'], identifier",
          ),
        ]
        "###);

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
        assert_rule!(input, rules::enum_);

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
        assert_rule!(input, rules::enum_, &mut diagnostics);
        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            text: "Invalid enum element: expected one of \"-\", \".\", \"\\\"\", \"null\", \"{\", value (number, string or empty object)",
          ),
          Diagnostic(
            kind: Error,
            range: "...",
            text: "Invalid enum element: expected one of \"/**\", identifier",
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
        assert_rule!(input, rules::enum_);

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
        assert_rule!(input, rules::const_);

        Ok(())
    }

    #[test]
    fn test_const_string() -> Result<()> {
        let input = "const TypeName CONST_NAME = \"const value\";";
        assert_rule!(input, rules::const_);

        Ok(())
    }

    #[test]
    fn test_const_with_javadoc() -> Result<()> {
        let input = r#"/**
            * Const docu
            */
           const TypeName CONST_NAME = 123;"#;
        assert_rule!(input, rules::const_);

        Ok(())
    }

    #[test]
    fn test_const_with_annotation() -> Result<()> {
        let input = "@AnnotationName const TypeName CONST_NAME = 123;";
        assert_rule!(input, rules::const_);

        Ok(())
    }

    #[test]
    fn test_type_primitive1() -> Result<()> {
        let input = "double";
        assert!(
            rules::type_any(input, &lookup(input), &mut Vec::new())?.kind
                == types::TypeKind::Primitive
        );
        assert_rule!(input, rules::type_any);

        Ok(())
    }

    #[test]
    fn test_type_primitive2() -> Result<()> {
        let input = "doublegum";
        assert!(
            rules::type_any(input, &lookup(input), &mut Vec::new())?.kind
                != types::TypeKind::Primitive
        );
        assert_rule!(input, rules::type_any);

        Ok(())
    }

    #[test]
    fn test_type_custom() -> Result<()> {
        let input = "TypeName";
        assert_eq!(
            rules::type_any(input, &lookup(input), &mut Vec::new())?.kind,
            types::TypeKind::Custom
        );
        assert_rule!(input, rules::type_any);

        Ok(())
    }

    #[test]
    fn test_type_custom_with_namespace() -> Result<()> {
        let input = "com.example.TypeName";
        assert_eq!(
            rules::type_any(input, &lookup(input), &mut Vec::new())?.kind,
            types::TypeKind::Custom
        );
        assert_rule!(input, rules::type_any);

        Ok(())
    }

    #[test]
    fn test_type_array() -> Result<()> {
        let input = "float []";
        assert_eq!(
            rules::type_any(input, &lookup(input), &mut Vec::new())?.kind,
            types::TypeKind::Array
        );
        assert_rule!(input, rules::type_any);

        // No array of String...
        let input = "String []";
        let mut diagnostics = Vec::new();
        assert_eq!(
            rules::type_any(input, &lookup(input), &mut diagnostics)?.kind,
            types::TypeKind::Invalid
        );
        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            text: "Invalid array: only primitives or enums are allowed. For objects, please use List<Object>",
          ),
        ]
        "###);

        Ok(())
    }

    #[test]
    fn test_type_list() -> Result<()> {
        let input = "List <MyObject >";
        assert_eq!(
            rules::type_any(input, &lookup(input), &mut Vec::new())?.kind,
            types::TypeKind::List
        );
        assert_rule!(input, rules::type_any);

        // No List for type_primitives
        let input = "List<int>";
        let mut diagnostics = Vec::new();
        assert_eq!(
            rules::type_any(input, &lookup(input), &mut diagnostics)?.kind,
            types::TypeKind::Invalid
        );
        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            text: "Invalid list: the value must be an object (not a primitive)",
          ),
        ]
        "###);

        Ok(())
    }

    #[test]
    fn test_type_map() -> Result<()> {
        let input = "Map<Key,List<V>>";
        assert_rule!(input, rules::type_any);

        Ok(())
    }

    #[test]
    fn test_type_map_primitive1() -> Result<()> {
        // No Map for type_primitives
        let input = "Map<int, String>";
        let mut diagnostics = Vec::new();
        assert_eq!(
            rules::type_any(input, &lookup(input), &mut diagnostics)?.kind,
            types::TypeKind::Invalid
        );
        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            text: "Invalid map: both key and value must be an object (not a primitive)",
          ),
        ]
        "###);

        Ok(())
    }

    #[test]
    fn test_type_map_primitive2() -> Result<()> {
        // No Map for type_primitives
        let input = "Map<String, int>";
        let mut diagnostics = Vec::new();
        assert_eq!(
            rules::type_any(input, &lookup(input), &mut diagnostics)?.kind,
            types::TypeKind::Invalid
        );
        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            text: "Invalid map: both key and value must be an object (not a primitive)",
          ),
        ]
        "###);

        Ok(())
    }

    #[test]
    fn test_type_invalid() -> Result<()> {
        let input = "tchou_tchou.";
        assert_eq!(
            rules::type_any(input, &lookup(input), &mut Vec::new())?.kind,
            types::TypeKind::Invalid
        );
        let mut diagnostics = Vec::new();
        assert_rule!(input, rules::type_any, &mut diagnostics);
        assert_diagnostics!(diagnostics, @r###"
        [
          Diagnostic(
            kind: Error,
            range: "...",
            text: "Invalid type",
          ),
        ]
        "###);

        Ok(())
    }

    #[test]
    fn test_value() -> Result<()> {
        // Numbers
        for input in ["12", "-12", "-0.12", "-.12", "-.12f"].into_iter() {
            assert_eq!(rules::value(input, &lookup(input), &mut Vec::new())?, input);
        }

        // Invalid numbers
        for input in ["-.", "--12", "0..2", "0.2y"].into_iter() {
            assert!(rules::value(input, &lookup(input), &mut Vec::new()).is_err());
        }

        // Strings
        for input in ["\"hello\"", "\"\"", "\"\t\""].into_iter() {
            assert_eq!(rules::value(input, &lookup(input), &mut Vec::new())?, input);
        }

        // Invalid strings
        for input in ["\"\"\""].into_iter() {
            assert!(rules::value(input, &lookup(input), &mut Vec::new()).is_err());
        }

        Ok(())
    }

    #[test]
    fn test_javadoc() -> Result<(), Box<dyn std::error::Error>> {
        let input = "/** This is a javadoc\n * comment*/";
        assert_eq!(
            rules::javadoc(input, &lookup(input), &mut Vec::new())?,
            "This is a javadoc comment"
        );

        let input = "/**\n * JavaDoc title\n *\n * JavaDoc text1\n * JavaDoc text2\n*/";
        assert_eq!(
            rules::javadoc(input, &lookup(input), &mut Vec::new())?,
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
            rules::javadoc(input, &lookup(input), &mut Vec::new())?,
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

    #[test]
    fn test_parse_element_until() -> Result<()> {
        let input = "_<,>_\",\"_[\n ]_";
        assert_eq!(
            rules::parse_element_until(input, &lookup(input), &mut Vec::new(), ",")?,
            input,
        );

        let input = "_,_";
        assert!(rules::parse_element_until(input, &lookup(input), &mut Vec::new(), ",").is_err());

        let input = "/** Hello, world */ blabla";
        assert_eq!(
            rules::parse_element_until(input, &lookup(input), &mut Vec::new(), ",")?,
            input,
        );

        Ok(())
    }

    #[test]
    fn test_dodo() -> Result<()> {
        let input = r#"/*
        * Structur to provide all information about specific road section characteristics.
        */
       parcelable RoadSectionOnRouteType {
           /**
            * The distanceToFinalDestination element specifies the 'static' distance value from the
            * beginning of the road section (the first touching point) to the final destination for the
            * given route. The distance from ccp to this element can be determined
            * using the ccp-based data.
            * The value is positiv and it is defined in meters.
            */
       
           int distanceToFinalDestination;
       
           /**
            * The roadSectrionType element represents the code or type of the road section.
            */
           RoadSectionOnRouteType roadSectionType;
       
           /**
            * The roadSectrionSpecifier element represents additional data which are relevant with some
            * roadOnRouteTypes only:
            *  - ROAD_CLASS: the roadOnRouteSpecifier specifies the value of the roadClass
            */
           int roadSectionSpecifier;
       
           /**
            * The lengthOnRoute element represents the length of the road section on the route in meter.
            */
           int lengthOnRoute;
        }"#;

        //assert_rule!(input, rules::parcelable);

        Ok(())
    }
}
