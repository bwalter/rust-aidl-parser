use crate::types::{Position, Range};
use serde::Serialize;

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

pub(crate) fn from_peg_error(
    lookup: &line_col::LineColLookup,
    peg_error: peg::error::ParseError<peg::str::LineCol>,
) -> Diagnostic {
    let pos = Position::new(lookup, peg_error.location.offset);

    Diagnostic {
        kind: DiagnosticKind::Error,
        text: format!("Expected: {}", peg_error.expected.to_string()),
        range: Range {
            start: pos.clone(),
            end: pos,
        },
    }
}
