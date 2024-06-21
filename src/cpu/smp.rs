//! Symmetric multiprocessing.

#[cfg(target_arch = "aarch64")]
#[path = "../arch/aarch64/cpu/smp.rs"]
mod arch_smp;

//--------------------------------------------------------------------------------------------------
// Architectural Public Reexports
//--------------------------------------------------------------------------------------------------
pub use arch_smp::core_id;
