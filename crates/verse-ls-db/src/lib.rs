#![forbid(unsafe_code)]
//! Digest ingestion and symbol databases for verse-ls.
//!
//! Contract (M0): the symbol database lands in M2. This crate currently
//! defines the ingestion bounds and the error contract only; ingestion
//! validates its input and then returns a typed
//! [`DbError::NotImplemented`] — it never fabricates symbols. Real Epic
//! digest files are never committed to this repository (Owner ruling,
//! 2026-11-23); tests use small synthetic fixtures, and the full-corpus
//! gate runs against digests held locally on primo and hash-pinned in
//! TARGET.md.

use std::fmt;

/// Maximum accepted size of one digest file, in bytes.
pub const DIGEST_BYTES_MAX: usize = 33_554_432;

/// Maximum number of symbols held in the database. Hitting the cap is a
/// reported configuration event (M2), never a silent partial index.
pub const SYMBOL_COUNT_MAX: usize = 1_048_576;

/// Why a database operation failed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DbError {
    /// A digest exceeded [`DIGEST_BYTES_MAX`].
    DigestTooLarge {
        /// Actual digest size, in bytes.
        bytes: usize,
        /// The cap that was exceeded, in bytes.
        max: usize,
    },
    /// The database is not implemented yet (M0 contract stub).
    NotImplemented,
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DigestTooLarge { bytes, max } => {
                write!(
                    f,
                    "digest of {bytes} bytes exceeds the {max}-byte digest cap"
                )
            }
            Self::NotImplemented => write!(f, "the symbol database is not implemented yet (M2)"),
        }
    }
}

impl std::error::Error for DbError {}

/// The digest-backed symbol database. Empty until M2.
#[derive(Clone, Debug, Default)]
pub struct SymbolDb {
    symbol_count: usize,
}

impl SymbolDb {
    /// Create an empty database.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of symbols currently held. Always 0 until M2.
    pub fn symbol_count(&self) -> usize {
        self.symbol_count
    }

    /// Ingest one digest file's bytes.
    ///
    /// M0 contract stub: validates the size cap, then returns
    /// [`DbError::NotImplemented`]. M2 replaces the stub with real
    /// ingestion; the bounds and the error contract stay.
    pub fn ingest_digest(&mut self, bytes: &[u8]) -> Result<(), DbError> {
        if bytes.len() > DIGEST_BYTES_MAX {
            return Err(DbError::DigestTooLarge {
                bytes: bytes.len(),
                max: DIGEST_BYTES_MAX,
            });
        }
        Err(DbError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Validation: a fresh database is empty, and an ordinary synthetic
    // digest reaches the (honest) stub error instead of fabricating
    // symbols.
    #[test]
    fn new_database_is_empty_and_ingest_is_not_implemented() {
        let mut db = SymbolDb::new();
        assert_eq!(db.symbol_count(), 0);
        assert_eq!(
            db.ingest_digest(b"# synthetic digest\n"),
            Err(DbError::NotImplemented)
        );
        assert_eq!(db.symbol_count(), 0);
    }

    // Adversarial: an oversized digest is rejected by size before any
    // other work, with the exact byte counts preserved.
    #[test]
    fn oversized_digest_is_rejected_with_counts() {
        let mut db = SymbolDb::new();
        let digest = vec![0_u8; DIGEST_BYTES_MAX + 1];
        assert_eq!(
            db.ingest_digest(&digest),
            Err(DbError::DigestTooLarge {
                bytes: DIGEST_BYTES_MAX + 1,
                max: DIGEST_BYTES_MAX,
            })
        );
    }
}
