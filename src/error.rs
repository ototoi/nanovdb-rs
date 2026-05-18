use std::fmt;

/// Errors that can occur while opening or parsing a `.nvdb` file.
#[derive(Debug)]
pub enum Error {
    /// Underlying I/O failure (file not found, permission denied, etc.).
    Io(std::io::Error),
    /// The file is shorter than the bytes the parser needs at the
    /// expected offset.
    Truncated {
        offset: u64,
        wanted: usize,
        actual: usize,
    },
    /// The 8-byte magic at the segment header didn't match NanoVDB's
    /// `"NanoVDB0"` value (`0x304244566f6e614e` little-endian).
    BadMagic(u64),
    /// The file uses ZIP / BLOSC compression that this crate doesn't
    /// decompress yet.
    CompressionUnsupported(crate::header::Codec),
    /// The grid metadata declared a name longer than the remaining
    /// segment bytes -- almost certainly a malformed file.
    BadGridName { wanted: usize, available: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::Truncated { offset, wanted, actual } => write!(
                f,
                "truncated NanoVDB file at offset {}: wanted {} bytes, only {} available",
                offset, wanted, actual
            ),
            Error::BadMagic(m) => write!(
                f,
                "bad magic 0x{:016x}: not a NanoVDB file (expected 0x304244566f6e614e \"NanoVDB0\")",
                m
            ),
            Error::CompressionUnsupported(c) => write!(
                f,
                "NanoVDB compression codec {:?} is not yet supported by nanovdb-rs",
                c
            ),
            Error::BadGridName { wanted, available } => write!(
                f,
                "grid name claims {} bytes but only {} remain in the segment",
                wanted, available
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
