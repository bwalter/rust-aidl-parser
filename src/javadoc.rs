pub fn get_javadoc(input: &str, pos: usize) -> Option<String> {
    find_content_string(&input[..pos]).map(parse_javadoc)
}
fn find_content_string(input: &str) -> Option<&str> {
    let mut pos = 0;
    let mut start_pos: Option<usize> = None;
    let mut end_pos: Option<usize> = None;

    enum FindState {
        Idle,
        LineCommentOrSomethingElse,
        LineCommentOrSomethingElseBeforeSlash,
        BeforeEndSlash,
        InsideComment,
        BeforeBeginStar,
        BeforeBeginStarStar,
    }

    let mut state = FindState::Idle;

    for current in input.chars().rev() {
        pos += 1;
        match state {
            FindState::Idle => {
                if current == '/' {
                    state = FindState::BeforeEndSlash;
                } else if current != ' ' && current != '\n' && current != '\r' && current != '\t' {
                    state = FindState::LineCommentOrSomethingElse;
                }
            }
            FindState::LineCommentOrSomethingElse => {
                if current == '/' {
                    state = FindState::LineCommentOrSomethingElseBeforeSlash;
                } else if current == '\n' {
                    break;
                }
            }
            FindState::LineCommentOrSomethingElseBeforeSlash => {
                if current == '/' {
                    state = FindState::Idle;
                } else {
                    break;
                }
            }
            FindState::BeforeEndSlash => {
                state = if current == '*' {
                    end_pos = Some(pos);
                    FindState::InsideComment
                } else {
                    FindState::Idle
                };
            }
            FindState::InsideComment => {
                if current == '*' {
                    state = FindState::BeforeBeginStar;
                };
            }
            FindState::BeforeBeginStar => {
                state = if current == '*' {
                    FindState::BeforeBeginStarStar
                } else if current == '/' {
                    FindState::Idle
                } else {
                    FindState::InsideComment
                };
            }
            FindState::BeforeBeginStarStar => {
                if current == '/' {
                    start_pos = Some(pos - 3);
                    break;
                }

                state = FindState::InsideComment;
            }
        }
    }

    match (start_pos, end_pos) {
        (Some(start_pos), Some(end_pos)) => {
            let start_pos = input.len() - start_pos;
            let end_pos = input.len() - end_pos;

            Some(&input[start_pos..end_pos])
        }
        _ => None,
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

    #[test]
    fn test_find_content_string() {
        let input = "/**TestJavaDoc*/";
        assert_eq!(find_content_string(input), Some("TestJavaDoc"));

        let input = r#"bla bla
            /**
             * TestJavaDoc
             * 
             * End
             */
             // Comment"#;
        assert_eq!(
            find_content_string(input),
            Some(
                r#"
             * TestJavaDoc
             * 
             * End
             "#
            )
        );

        let input = r#"
            /** Documentation before */
            /** The real documentation */
            /* Comment after */
            // Line comment after
            "#;
        assert_eq!(find_content_string(input), Some(" The real documentation "),);

        let input = r#"
            /** Other documentation */
            something else;
            "#;
        assert_eq!(find_content_string(input), None);

        let input = r#"
            something else;
            /**The documentation*/
            "#;
        assert_eq!(find_content_string(input), Some("The documentation"));
    }

    #[test]
    fn test_parse_javadoc() {
        let input = "This is a javadoc\n * comment";
        assert_eq!(parse_javadoc(input), "This is a javadoc comment");

        let input = "\n * JavaDoc title\n *\n * JavaDoc text1\n * JavaDoc text2\n";
        assert_eq!(
            parse_javadoc(input),
            "JavaDoc title\nJavaDoc text1 JavaDoc text2"
        );

        let input = r#"
                * JavaDoc title
                * @param Param1 Description
                * @param Param2 Description
                *
                * Description
                "#;
        assert_eq!(
            parse_javadoc(input),
            "JavaDoc title\n@param Param1 Description\n@param Param2 Description\nDescription"
        );
    }
}
