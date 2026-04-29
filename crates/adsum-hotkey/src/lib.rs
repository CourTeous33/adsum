//! Cross-platform global hotkey wrapper with restart-once supervisor.

pub mod backend;
pub mod supervisor;

mod real_backend;
pub use real_backend::RealBackend;
