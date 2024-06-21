use crate::{bsp::device_driver::common::MMIODerefWrapper, exception};
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields, register_structs,
    registers::ReadWrite,
};

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

register_bitfields! {
    u32,

    /// CPU Interface Control Register
    CTLR [
        Enable OFFSET(0) NUMBITS(1) []
    ],

    /// Interrupt Priority Mask Register
    PMR [
        Priority OFFSET(0) NUMBITS(8) []
    ],

    /// Interrupt Acknowledge Register
    IAR [
        InterruptID OFFSET(0) NUMBITS(10) []
    ],

    /// End of Interrupt Register
    EOIR [
        EOIINTID OFFSET(0) NUMBITS(10) []
    ]
}

register_structs! {
    #[allow(non_snake_case)]
    pub RegisterBlock {
        (0x000 => CTLR: ReadWrite<u32, CTLR::Register>),
        (0x004 => PMR: ReadWrite<u32, PMR::Register>),
        (0x008 => _reserved1),
        (0x00C => IAR: ReadWrite<u32, IAR::Register>),
        (0x010 => EOIR: ReadWrite<u32, EOIR::Register>),
        (0x014  => @END),
    }
}

/// Abstraction for the associated MMIO registers.
type Registers = MMIODerefWrapper<RegisterBlock>;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Representation of the GIC CPU interface.
pub struct GICC {
    registers: Registers,
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl GICC {
    /// 构造函数 new
    pub const unsafe fn new(mmio_start_addr: usize) -> Self {
        Self {
            registers: Registers::new(mmio_start_addr),
        }
    }
    /// 将优先级掩码寄存器（PMR）设置为 255，接受所有优先级的中断
    pub fn priority_accept_all(&self) {
        self.registers.PMR.write(PMR::Priority.val(255)); // Comment in arch spec.
    }
    /// 方法启用 GICC 接口，开始接受 IRQ
    pub fn enable(&self) {
        self.registers.CTLR.write(CTLR::Enable::SET);
    }
    /// 提取最高优先级的挂起中断的编号。它只能在 IRQ 上下文中调用。
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn pending_irq_number<'irq_context>(
        &self,
        _ic: &exception::asynchronous::IRQContext<'irq_context>,
    ) -> usize {
        self.registers.IAR.read(IAR::InterruptID) as usize
    }
    /// 标记当前活动的 IRQ 处理完成。它也只能在 IRQ 上下文中调用，
    /// 并在 pending_irq_number 之后调用。
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn mark_comleted<'irq_context>(
        &self,
        irq_number: u32,
        _ic: &exception::asynchronous::IRQContext<'irq_context>,
    ) {
        self.registers.EOIR.write(EOIR::EOIINTID.val(irq_number));
    }
}
