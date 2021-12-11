use crate::types::Range;
use serde_derive::Serialize;

#[derive(Serialize, Debug, PartialEq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub range: Range,
    pub text: String,
}

#[derive(Serialize, Debug, PartialEq)]
pub enum DiagnosticKind {
    Error,
    Warning,
}

pub type ErrorRecovery<'input> =
    lalrpop_util::ErrorRecovery<usize, crate::aidl::Token<'input>, &'static str>;

pub type ParseError<'input> =
    lalrpop_util::ParseError<usize, crate::aidl::Token<'input>, &'static str>;

impl Diagnostic {
    pub(crate) fn from_error_recovery<'input>(
        msg: &'static str,
        lookup: &line_col::LineColLookup,
        error_recovery: ErrorRecovery<'input>,
    ) -> Result<Diagnostic, ParseError<'input>> {
        if error_recovery.dropped_tokens.is_empty() {
            return Err(error_recovery.error);
        }

        let p1 = error_recovery.dropped_tokens[0].0;
        let p2 = error_recovery.dropped_tokens.last().unwrap().0;

        Ok(Diagnostic {
            kind: DiagnosticKind::Error,
            text: format!("{}: {}", &msg, error_recovery.error),
            range: Range::new(lookup, p1, p2),
        })
    }
}
