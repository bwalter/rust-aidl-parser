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
