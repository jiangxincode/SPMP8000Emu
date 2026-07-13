// Error types for SPMP8000 emulator

use thiserror::Error;

/// Main error type for emulator operations
#[derive(Error, Debug)]
pub enum EmulatorError {
    #[error("Invalid BIN file: {0}")]
    InvalidBinFile(String),

    #[error("Unsupported compression format")]
    UnsupportedCompression,

    #[error("CPU emulation error: {0}")]
    CpuError(String),

    #[error("Memory access error: {0}")]
    MemoryError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("API call not implemented: {0}")]
    UnimplementedApi(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, EmulatorError>;
