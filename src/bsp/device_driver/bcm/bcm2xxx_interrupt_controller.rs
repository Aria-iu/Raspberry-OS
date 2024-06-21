mod peripheral_ic;

use crate::{
    bsp::device_driver::common::BoundedUsize,
    driver,
    exception::{self, asynchronous::IRQHandlerDescriptor},
};
use core::fmt;

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

/// 定义了一个结构体 PendingIRQs，用于封装表示挂起中断号的位掩码。
struct PendingIRQs {
    bitmask: u64,
}

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// 中断号类型 (LocalIRQ 和 PeripheralIRQ)
pub type LocalIRQ = BoundedUsize<{ InterruptController::MAX_LOCAL_IRQ_NUMBER }>;
pub type PeripheralIRQ = BoundedUsize<{ InterruptController::MAX_PERIPHERAL_IRQ_NUMBER }>;

/// Used for the associated type of trait [`exception::asynchronous::interface::IRQManager`].
/// 枚举 IRQNumber 来表示不同类型的中断号。
#[derive(Copy, Clone)]
#[allow(missing_docs)]
pub enum IRQNumber {
    Local(LocalIRQ),
    Peripheral(PeripheralIRQ),
}

/// Representation of the Interrupt Controller.
pub struct InterruptController {
    periph: peripheral_ic::PeripheralIC,
}

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

impl PendingIRQs {
    pub fn new(bitmask: u64) -> Self {
        Self { bitmask }
    }
}

///
/// PendingIRQs 的实现：
///
/// new：构造函数，用于创建 PendingIRQs 实例。
/// Iterator 实现：提供迭代器功能，遍历所有挂起的中断号。
impl Iterator for PendingIRQs {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bitmask == 0 {
            return None;
        }

        let next = self.bitmask.trailing_zeros() as usize;
        self.bitmask &= self.bitmask.wrapping_sub(1);
        Some(next)
    }
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl fmt::Display for IRQNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Local(number) => write!(f, "Local({})", number),
            Self::Peripheral(number) => write!(f, "Peripheral({})", number),
        }
    }
}

impl InterruptController {
    // Restrict to 3 for now. This makes future code for local_ic.rs more straight forward.
    const MAX_LOCAL_IRQ_NUMBER: usize = 3;
    const MAX_PERIPHERAL_IRQ_NUMBER: usize = 63;

    pub const COMPATIBLE: &'static str = "BCM Interrupt Controller";

    /// Create an instance.
    ///
    /// # Safety
    ///
    /// - The user must ensure to provide a correct MMIO start address.
    pub const unsafe fn new(periph_mmio_start_addr: usize) -> Self {
        Self {
            periph: peripheral_ic::PeripheralIC::new(periph_mmio_start_addr),
        }
    }
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------
///
/// 实现了 DeviceDriver 和 IRQManager 接口，提供了中断管理的功能：
///
/// register_handler：注册中断处理程序。目前仅实现了外设中断处理程序的注册。
/// enable：启用中断。目前仅实现了外设中断的启用。
/// handle_pending_irqs：处理挂起的中断，调用相应的处理程序。
/// print_handler：打印已注册的中断处理程序信息。
///
impl driver::interface::DeviceDriver for InterruptController {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }
}

impl exception::asynchronous::interface::IRQManager for InterruptController {
    type IRQNumberType = IRQNumber;

    fn register_handler(
        &self,
        irq_handler_descriptor: exception::asynchronous::IRQHandlerDescriptor<Self::IRQNumberType>,
    ) -> Result<(), &'static str> {
        match irq_handler_descriptor.number() {
            IRQNumber::Local(_) => unimplemented!("Local IRQ controller not implemented."),
            IRQNumber::Peripheral(pirq) => {
                let periph_descriptor = IRQHandlerDescriptor::new(
                    pirq,
                    irq_handler_descriptor.name(),
                    irq_handler_descriptor.handler(),
                );

                self.periph.register_handler(periph_descriptor)
            }
        }
    }

    fn enable(&self, irq: &Self::IRQNumberType) {
        match irq {
            IRQNumber::Local(_) => unimplemented!("Local IRQ controller not implemented."),
            IRQNumber::Peripheral(pirq) => self.periph.enable(pirq),
        }
    }

    fn handle_pending_irqs<'irq_context>(
        &'irq_context self,
        ic: &exception::asynchronous::IRQContext<'irq_context>,
    ) {
        // It can only be a peripheral IRQ pending because enable() does not support local IRQs yet.
        self.periph.handle_pending_irqs(ic)
    }

    fn print_handler(&self) {
        self.periph.print_handler();
    }
}
