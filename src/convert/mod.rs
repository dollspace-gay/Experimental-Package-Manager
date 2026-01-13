//! Package format conversion module
//!
//! Converts package specifications from other distributions to .rook format.

pub mod arch;
pub mod pkgbuild;

pub use arch::ArchConverter;
