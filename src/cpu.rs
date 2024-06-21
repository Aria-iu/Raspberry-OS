//! Processor code.

#[cfg(target_arch = "aarch64")]
#[path = "arch/aarch64/cpu.rs"]
mod arch_cpu;

mod boot;
pub mod smp;

//--------------------------------------------------------------------------------------------------
// Architectural Public Reexports
//--------------------------------------------------------------------------------------------------
pub use arch_cpu::nop;
pub use arch_cpu::spin_for_cycles;
pub use arch_cpu::wait_forever;
