use crate::ast::Range;
use crate::rules;
use serde_derive::Serialize;

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub range: Range,
    pub message: String,

    /// Additional information displayed near the symbol
    pub context_message: Option<String>,

    pub hint: Option<String>,
    pub related_infos: Vec<RelatedInfo>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    Error,
    Warning,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct RelatedInfo {
    pub range: Range,
    pub message: String,
}

pub type ErrorRecovery<'input> =
    lalrpop_util::ErrorRecovery<usize, rules::aidl::Token<'input>, &'static str>;

pub type ParseError<'input> =
    lalrpop_util::ParseError<usize, rules::aidl::Token<'input>, &'static str>;

impl Diagnostic {
    pub(crate) fn from_error_recovery<'input>(
        msg: &str,
        lookup: &line_col::LineColLookup,
        error_recovery: ErrorRecovery<'input>,
    ) -> Option<Diagnostic> {
        Self::from_parse_error(lookup, error_recovery.error).map(|d| Diagnostic {
            message: format!("{} - {}", msg, d.message),
            ..d
        })
    }

    pub(crate) fn from_parse_error(
        lookup: &line_col::LineColLookup,
        e: ParseError,
    ) -> Option<Diagnostic> {
        match e {
            lalrpop_util::ParseError::InvalidToken { location } => Some(Diagnostic {
                kind: DiagnosticKind::Error,
                message: "Invalid token".to_owned(),
                context_message: Some("invalid token".to_owned()),
                range: Range::new(lookup, location, location),
                hint: None,
                related_infos: Vec::new(),
            }),
            lalrpop_util::ParseError::UnrecognizedEOF { location, expected } => Some(Diagnostic {
                kind: DiagnosticKind::Error,
                message: format!("Unrecognized EOF.\n{}", expected_token_str(&expected)),
                context_message: Some("unrecognized EOF".to_owned()),
                range: Range::new(lookup, location, location),
                hint: None,
                related_infos: Vec::new(),
            }),
            lalrpop_util::ParseError::UnrecognizedToken { token, expected } => Some(Diagnostic {
                kind: DiagnosticKind::Error,
                message: format!(
                    "Unrecognized token `{}`.\n{}",
                    token.1,
                    expected_token_str(&expected)
                ),
                context_message: Some("unrecognized token".to_owned()),
                range: Range::new(lookup, token.0, token.2),
                hint: None,
                related_infos: Vec::new(),
            }),
            lalrpop_util::ParseError::ExtraToken { token } => Some(Diagnostic {
                kind: DiagnosticKind::Error,
                message: format!("Extra token `{}`", token.1,),
                context_message: Some("extra token".to_owned()),
                range: Range::new(lookup, token.0, token.2),
                hint: None,
                related_infos: Vec::new(),
            }),
            lalrpop_util::ParseError::User { error: _ } => None, // User errors already produced a Diagnostic
        }
    }
}

// TODO: replace empty (or EOF?)!
fn expected_token_str(v: &[String]) -> String {
    match v.len() {
        0 => String::new(),
        1 => format!("Expected {}", v[0]),
        2 => format!("Expected {} or {}", v[0], v[1]),
        _ => format!(
            "Expected one of {} or {}",
            v[0..v.len() - 2].join(", "),
            v[v.len() - 1]
        ),
    }
}
