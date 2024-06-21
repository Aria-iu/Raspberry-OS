#![allow(clippy::upper_case_acronyms)]
#![allow(incomplete_features)]
#![feature(asm_const)]
#![feature(const_option)]
#![feature(core_intrinsics)]
#![allow(internal_features)]
#![feature(format_args_nl)]
#![feature(int_roundings)]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(trait_alias)]
#![allow(unused_variables)]
#![no_std]

mod panic;
mod synchronization;

pub mod bsp;
pub mod common;
pub mod console;
pub mod cpu;
pub mod driver;
pub mod exception;
pub mod memory;
pub mod print;
pub mod state;
pub mod time;

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

/// Version string.
pub fn version() -> &'static str {
    concat!(
        env!("CARGO_PKG_NAME"),
        " version ",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(not(test))]
extern "Rust" {
    fn kernel_init() -> !;
}
