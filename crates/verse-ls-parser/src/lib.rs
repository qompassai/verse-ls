#![forbid(unsafe_code)]
//! Verse source parsing for verse-ls: lexer, parser, AST, and parse
//! diagnostics.
//!
//! Contract (M0): this crate is pure, synchronous, and performs no I/O.
//! The parser itself lands in M1; until then [`parse_document`] validates
//! its input against the bounds below and then returns a typed
//! [`ParseError::NotImplemented`] — it never fabricates an empty,
//! successful parse. Callers must treat that error as "no diagnostics
//! available", never as "the document is clean".

use std::fmt;

/// Maximum accepted document size, in bytes. Inputs larger than this are
/// rejected before any parsing work happens.
pub const DOCUMENT_BYTES_MAX: usize = 8_388_608;

/// Maximum number of diagnostics published for one document. Publication
/// beyond this cap is truncated with an explicit marker diagnostic (M1),
/// never silently dropped.
pub const DIAGNOSTICS_PER_DOCUMENT_MAX: usize = 256;

/// Maximum length, in bytes, of one diagnostic message.
pub const DIAGNOSTIC_MESSAGE_BYTES_MAX: usize = 512;

/// Deadline for parsing one document version, in milliseconds. A missed
/// deadline degrades diagnostics for that version; it never blocks the
/// request path.
pub const PARSE_DEADLINE_MS: u64 = 500;

/// A byte range in a source document: `[start_byte, end_byte)`.
///
/// Offsets are byte offsets into the UTF-8 source text, not character or
/// UTF-16 code-unit indices; converting to LSP positions is the binary
/// crate's job (M1).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    /// First byte of the range.
    pub start_byte: u32,
    /// One past the last byte of the range.
    pub end_byte: u32,
}

/// Severity of a parse [`Diagnostic`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    /// The construct is not valid Verse.
    Error,
    /// The construct parses but is suspicious.
    Warning,
}

/// One parse diagnostic: where it is, how bad it is, and a bounded
/// human-readable message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    /// Source range the diagnostic points at.
    pub span: Span,
    /// Diagnostic severity.
    pub severity: Severity,
    /// Bounded message ([`DIAGNOSTIC_MESSAGE_BYTES_MAX`]).
    pub message: String,
}

/// The result of parsing one document version.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ParseOutcome {
    /// Diagnostics for the document, capped at
    /// [`DIAGNOSTICS_PER_DOCUMENT_MAX`].
    pub diagnostics: Vec<Diagnostic>,
}

/// Why a parse request failed before producing an outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseError {
    /// The input exceeded [`DOCUMENT_BYTES_MAX`].
    InputTooLarge {
        /// Actual input size, in bytes.
        bytes: usize,
        /// The cap that was exceeded, in bytes.
        max: usize,
    },
    /// The parser is not implemented yet (M0 contract stub).
    NotImplemented,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge { bytes, max } => {
                write!(
                    f,
                    "input of {bytes} bytes exceeds the {max}-byte document cap"
                )
            }
            Self::NotImplemented => write!(f, "the Verse parser is not implemented yet (M1)"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse one document version.
///
/// M0 contract stub: validates the input size, then returns
/// [`ParseError::NotImplemented`]. M1 replaces the stub with the real
/// lexer/parser; the bounds and the error contract stay.
pub fn parse_document(source: &str) -> Result<ParseOutcome, ParseError> {
    if source.len() > DOCUMENT_BYTES_MAX {
        return Err(ParseError::InputTooLarge {
            bytes: source.len(),
            max: DOCUMENT_BYTES_MAX,
        });
    }
    Err(ParseError::NotImplemented)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Validation: an ordinary input reaches the (honest) stub error —
    // the stub never reports a fabricated clean parse.
    #[test]
    fn ordinary_input_returns_not_implemented() {
        assert_eq!(parse_document("x := 1\n"), Err(ParseError::NotImplemented));
    }

    // Adversarial: an oversized input is rejected by size before any
    // other work, with the exact byte counts preserved.
    #[test]
    fn oversized_input_is_rejected_with_counts() {
        let source = "x".repeat(DOCUMENT_BYTES_MAX + 1);
        assert_eq!(
            parse_document(&source),
            Err(ParseError::InputTooLarge {
                bytes: DOCUMENT_BYTES_MAX + 1,
                max: DOCUMENT_BYTES_MAX,
            })
        );
    }
}
