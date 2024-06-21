mod gicc;
mod gicd;

use crate::{
    bsp::{self, device_driver::common::BoundedUsize},
    cpu, driver, exception, synchronization,
    synchronization::InitStateLock,
};

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------
/// 定义了一个类型 HandlerTable，表示中断处理程序表，用于存储注册的中断处理程序。
type HandlerTable = [Option<exception::asynchronous::IRQHandlerDescriptor<IRQNumber>>;
    IRQNumber::MAX_INCLUSIVE + 1];

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Used for the associated type of trait [`exception::asynchronous::interface::IRQManager`].
/// 定义了一个类型 IRQNumber，表示受限范围的中断号。
pub type IRQNumber = BoundedUsize<{ GICv2::MAX_IRQ_NUMBER }>;

/// GICv2 结构体表示 GIC v2，包括分配器（gicd）、CPU 接口（gicc）和中断处理程序表（handler_table）。
pub struct GICv2 {
    /// The Distributor.
    gicd: gicd::GICD,

    /// The CPU Interface.
    gicc: gicc::GICC,

    /// Stores registered IRQ handlers. Writable only during kernel init. RO afterwards.
    handler_table: InitStateLock<HandlerTable>,
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl GICv2 {
    const MAX_IRQ_NUMBER: usize = 300; // Normally 1019, but keep it lower to save some space.

    pub const COMPATIBLE: &'static str = "GICv2 (ARM Generic Interrupt Controller v2)";

    /// Create an instance.
    ///
    /// # Safety
    ///
    /// - The user must ensure to provide a correct MMIO start address.
    pub const unsafe fn new(gicd_mmio_start_addr: usize, gicc_mmio_start_addr: usize) -> Self {
        Self {
            gicd: gicd::GICD::new(gicd_mmio_start_addr),
            gicc: gicc::GICC::new(gicc_mmio_start_addr),
            handler_table: InitStateLock::new([None; IRQNumber::MAX_INCLUSIVE + 1]),
        }
    }
}

//------------------------------------------------------------------------------
// OS Interface Code
//------------------------------------------------------------------------------
use synchronization::interface::ReadWriteEx;

impl driver::interface::DeviceDriver for GICv2 {
    type IRQNumberType = IRQNumber;

    fn compatible(&self) -> &'static str {
        Self::COMPATIBLE
    }

    unsafe fn init(&self) -> Result<(), &'static str> {
        if bsp::cpu::BOOT_CORE_ID == cpu::smp::core_id() {
            self.gicd.boot_core_init();
        }

        self.gicc.priority_accept_all();
        self.gicc.enable();

        Ok(())
    }
}

///
/// 实现了 IRQManager 接口，用于中断管理。
/// register_handler：注册中断处理程序。
/// enable：启用指定的中断。
/// handle_pending_irqs：处理挂起的中断，调用相应的中断处理程序。
/// print_handler：打印已注册的中断处理程序信息。
///
impl exception::asynchronous::interface::IRQManager for GICv2 {
    type IRQNumberType = IRQNumber;

    fn register_handler(
        &self,
        irq_handler_descriptor: exception::asynchronous::IRQHandlerDescriptor<Self::IRQNumberType>,
    ) -> Result<(), &'static str> {
        self.handler_table.write(|table| {
            let irq_number = irq_handler_descriptor.number().get();

            if table[irq_number].is_some() {
                return Err("IRQ handler already registered");
            }

            table[irq_number] = Some(irq_handler_descriptor);

            Ok(())
        })
    }

    fn enable(&self, irq_number: &Self::IRQNumberType) {
        self.gicd.enable(irq_number);
    }

    fn handle_pending_irqs<'irq_context>(
        &'irq_context self,
        ic: &exception::asynchronous::IRQContext<'irq_context>,
    ) {
        // Extract the highest priority pending IRQ number from the Interrupt Acknowledge Register
        // (IAR).
        let irq_number = self.gicc.pending_irq_number(ic);

        // Guard against spurious interrupts.
        if irq_number > GICv2::MAX_IRQ_NUMBER {
            return;
        }

        // Call the IRQ handler. Panic if there is none.
        self.handler_table.read(|table| {
            match table[irq_number] {
                None => panic!("No handler registered for IRQ {}", irq_number),
                Some(descriptor) => {
                    // Call the IRQ handler. Panics on failure.
                    descriptor.handler().handle().expect("Error handling IRQ");
                }
            }
        });

        // Signal completion of handling.
        self.gicc.mark_comleted(irq_number as u32, ic);
    }

    fn print_handler(&self) {
        use crate::info;

        info!("      Peripheral handler:");

        self.handler_table.read(|table| {
            for (i, opt) in table.iter().skip(32).enumerate() {
                if let Some(handler) = opt {
                    info!("            {: >3}. {}", i + 32, handler.name());
                }
            }
        });
    }
}
