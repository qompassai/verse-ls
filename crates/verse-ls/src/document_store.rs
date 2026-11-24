//! Open-document bookkeeping for the server.
//!
//! Contract: the store owns the current text of every open document,
//! bounded by [`verse_ls_parser::DOCUMENT_BYTES_MAX`]. All failures are
//! typed errors; the store never panics on client-driven input (change
//! before open, close of an unknown document, duplicate open, oversized
//! text). URIs are stored as their string form — the store does not
//! interpret them.

use std::collections::HashMap;
use std::fmt;

use verse_ls_parser::DOCUMENT_BYTES_MAX;

/// Why a document-store operation failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StoreError {
    /// `didOpen` arrived for a document that is already open.
    AlreadyOpen {
        /// URI of the document, in string form.
        uri: String,
    },
    /// A change or close arrived for a document that is not open.
    NotOpen {
        /// URI of the document, in string form.
        uri: String,
    },
    /// The document text exceeded [`DOCUMENT_BYTES_MAX`].
    TooLarge {
        /// URI of the document, in string form.
        uri: String,
        /// Actual text size, in bytes.
        bytes: usize,
        /// The cap that was exceeded, in bytes.
        max: usize,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyOpen { uri } => write!(f, "document already open: {uri}"),
            Self::NotOpen { uri } => write!(f, "document not open: {uri}"),
            Self::TooLarge { uri, bytes, max } => {
                write!(
                    f,
                    "document {uri} is {bytes} bytes, over the {max}-byte cap; not stored"
                )
            }
        }
    }
}

impl std::error::Error for StoreError {}

/// One open document: its last seen version and its full current text
/// (the server advertises full document sync).
struct Document {
    version: i32,
    text: String,
}

/// The set of currently open documents, keyed by URI string.
#[derive(Default)]
pub struct DocumentStore {
    documents: HashMap<String, Document>,
}

impl DocumentStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a `didOpen`: the document becomes tracked at `version`
    /// with the given full `text`.
    ///
    /// # Errors
    ///
    /// [`StoreError::TooLarge`] if `text` exceeds the document cap (the
    /// document is not stored), [`StoreError::AlreadyOpen`] if the URI
    /// is already tracked.
    pub fn open(&mut self, uri: String, version: i32, text: String) -> Result<(), StoreError> {
        check_size(&uri, &text)?;
        if self.documents.contains_key(&uri) {
            return Err(StoreError::AlreadyOpen { uri });
        }
        self.documents.insert(uri, Document { version, text });
        Ok(())
    }

    /// Record a `didChange` under full sync: replace the tracked text
    /// with the new full `text` at `version`.
    ///
    /// # Errors
    ///
    /// [`StoreError::TooLarge`] if `text` exceeds the document cap (the
    /// previous text is kept), [`StoreError::NotOpen`] if the URI is
    /// not tracked.
    pub fn change(&mut self, uri: &str, version: i32, text: String) -> Result<(), StoreError> {
        check_size(uri, &text)?;
        let Some(document) = self.documents.get_mut(uri) else {
            return Err(StoreError::NotOpen {
                uri: uri.to_owned(),
            });
        };
        document.version = version;
        document.text = text;
        Ok(())
    }

    /// Record a `didClose`: the document stops being tracked.
    ///
    /// # Errors
    ///
    /// [`StoreError::NotOpen`] if the URI is not tracked.
    pub fn close(&mut self, uri: &str) -> Result<(), StoreError> {
        if self.documents.remove(uri).is_none() {
            return Err(StoreError::NotOpen {
                uri: uri.to_owned(),
            });
        }
        Ok(())
    }

    /// Number of documents currently tracked.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Last seen version of a tracked document, if it is open.
    ///
    /// Test-only at M0: it exists to verify the lifecycle bookkeeping
    /// until M1's diagnostics path consumes versions directly.
    #[cfg(test)]
    pub fn version_of(&self, uri: &str) -> Option<i32> {
        self.documents.get(uri).map(|document| document.version)
    }
}

/// Reject text over the document cap before any storage work happens.
fn check_size(uri: &str, text: &str) -> Result<(), StoreError> {
    if text.len() > DOCUMENT_BYTES_MAX {
        return Err(StoreError::TooLarge {
            uri: uri.to_owned(),
            bytes: text.len(),
            max: DOCUMENT_BYTES_MAX,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const URI: &str = "file:///fixture/island.verse";

    // Validation: the ordinary open → change → close lifecycle tracks
    // version and count exactly.
    #[test]
    fn open_change_close_lifecycle() {
        let mut store = DocumentStore::new();
        store
            .open(URI.to_owned(), 1, "a := 1\n".to_owned())
            .expect("open succeeds");
        assert_eq!(store.document_count(), 1);
        assert_eq!(store.version_of(URI), Some(1));

        store
            .change(URI, 2, "a := 2\n".to_owned())
            .expect("change succeeds");
        assert_eq!(store.version_of(URI), Some(2));

        store.close(URI).expect("close succeeds");
        assert_eq!(store.document_count(), 0);
        assert_eq!(store.version_of(URI), None);
    }

    // Adversarial: a change for a document that was never opened is a
    // typed error, not a panic and not a silent insert.
    #[test]
    fn change_before_open_is_a_typed_error() {
        let mut store = DocumentStore::new();
        assert_eq!(
            store.change(URI, 1, "a := 1\n".to_owned()),
            Err(StoreError::NotOpen {
                uri: URI.to_owned()
            })
        );
        assert_eq!(store.document_count(), 0);
    }

    // Adversarial: closing an unknown document is a typed error.
    #[test]
    fn close_of_unknown_document_is_a_typed_error() {
        let mut store = DocumentStore::new();
        assert_eq!(
            store.close(URI),
            Err(StoreError::NotOpen {
                uri: URI.to_owned()
            })
        );
    }

    // Adversarial: a duplicate open is refused and the original
    // document is untouched.
    #[test]
    fn duplicate_open_is_refused() {
        let mut store = DocumentStore::new();
        store
            .open(URI.to_owned(), 1, "a := 1\n".to_owned())
            .expect("first open succeeds");
        assert_eq!(
            store.open(URI.to_owned(), 2, "a := 2\n".to_owned()),
            Err(StoreError::AlreadyOpen {
                uri: URI.to_owned()
            })
        );
        assert_eq!(store.version_of(URI), Some(1));
    }

    // Adversarial: oversized text is rejected by size and nothing is
    // stored.
    #[test]
    fn oversized_open_is_rejected_and_not_stored() {
        let mut store = DocumentStore::new();
        let text = "x".repeat(DOCUMENT_BYTES_MAX + 1);
        assert_eq!(
            store.open(URI.to_owned(), 1, text),
            Err(StoreError::TooLarge {
                uri: URI.to_owned(),
                bytes: DOCUMENT_BYTES_MAX + 1,
                max: DOCUMENT_BYTES_MAX,
            })
        );
        assert_eq!(store.document_count(), 0);
    }
}
