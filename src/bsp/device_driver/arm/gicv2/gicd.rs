use crate::{
    bsp::device_driver::common::MMIODerefWrapper, state, synchronization,
    synchronization::IRQSafeNullLock,
};
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields, register_structs,
    registers::{ReadOnly, ReadWrite},
};

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

register_bitfields! {
    u32,

    /// Distributor Control Register
    CTLR [
        Enable OFFSET(0) NUMBITS(1) []
    ],

    /// Interrupt Controller Type Register
    TYPER [
        ITLinesNumber OFFSET(0)  NUMBITS(5) []
    ],

    /// Interrupt Processor Targets Registers
    ITARGETSR [
        Offset3 OFFSET(24) NUMBITS(8) [],
        Offset2 OFFSET(16) NUMBITS(8) [],
        Offset1 OFFSET(8)  NUMBITS(8) [],
        Offset0 OFFSET(0)  NUMBITS(8) []
    ]
}

register_structs! {
    #[allow(non_snake_case)]
    SharedRegisterBlock {
        (0x000 => CTLR: ReadWrite<u32, CTLR::Register>),
        (0x004 => TYPER: ReadOnly<u32, TYPER::Register>),
        (0x008 => _reserved1),
        (0x104 => ISENABLER: [ReadWrite<u32>; 31]),
        (0x180 => _reserved2),
        (0x820 => ITARGETSR: [ReadWrite<u32, ITARGETSR::Register>; 248]),
        (0xC00 => @END),
    }
}

register_structs! {
    #[allow(non_snake_case)]
    BankedRegisterBlock {
        (0x000 => _reserved1),
        (0x100 => ISENABLER: ReadWrite<u32>),
        (0x104 => _reserved2),
        (0x800 => ITARGETSR: [ReadOnly<u32, ITARGETSR::Register>; 8]),
        (0x820 => @END),
    }
}

/// Abstraction for the non-banked parts of the associated MMIO registers.
type SharedRegisters = MMIODerefWrapper<SharedRegisterBlock>;

/// Abstraction for the banked parts of the associated MMIO registers.
type BankedRegisters = MMIODerefWrapper<BankedRegisterBlock>;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// 定义了一个 GICD 结构体，表示 GIC 分配器。它包含两个字段：
/// shared_registers：通过锁保护的共享寄存器访问。
/// banked_registers：未保护的分组寄存器访问。
pub struct GICD {
    /// Access to shared registers is guarded with a lock.
    shared_registers: IRQSafeNullLock<SharedRegisters>,

    /// Access to banked registers is unguarded.
    banked_registers: BankedRegisters,
}

//--------------------------------------------------------------------------------------------------
// Private Code
//--------------------------------------------------------------------------------------------------

impl SharedRegisters {
    /// 返回该硬件实现的 IRQ 数量。
    #[inline(always)]
    fn num_irqs(&mut self) -> usize {
        // Query number of implemented IRQs.
        //
        // Refer to GICv2 Architecture Specification, Section 4.3.2.
        ((self.TYPER.read(TYPER::ITLinesNumber) as usize) + 1) * 32
    }

    /// 返回已实现的 ITARGETSR 寄存器的切片
    #[inline(always)]
    fn implemented_itargets_slice(&mut self) -> &[ReadWrite<u32, ITARGETSR::Register>] {
        assert!(self.num_irqs() >= 36);

        // Calculate the max index of the shared ITARGETSR array.
        //
        // The first 32 IRQs are private, so not included in `shared_registers`. Each ITARGETS
        // register has four entries, so shift right by two. Subtract one because we start
        // counting at zero.
        let spi_itargetsr_max_index = ((self.num_irqs() - 32) >> 2) - 1;

        // Rust automatically inserts slice range sanity check, i.e. max >= min.
        &self.ITARGETSR[0..spi_itargetsr_max_index]
    }
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------
use synchronization::interface::Mutex;

impl GICD {
    /// 创建 GICD 实例。用户需要提供一个正确的 MMIO 起始地址。
    pub const unsafe fn new(mmio_start_addr: usize) -> Self {
        Self {
            shared_registers: IRQSafeNullLock::new(SharedRegisters::new(mmio_start_addr)),
            banked_registers: BankedRegisters::new(mmio_start_addr),
        }
    }

    /// 使用分组 ITARGETSR 获取当前执行核心的 GIC 目标掩码。
    fn local_gic_target_mask(&self) -> u32 {
        self.banked_registers.ITARGETSR[0].read(ITARGETSR::Offset0)
    }

    /// 将所有 SPI 路由到启动核心并启用分配器。确保只有在内核初始化阶段调用。
    pub fn boot_core_init(&self) {
        assert!(
            state::state_manager().is_init(),
            "Only allowed during kernel init phase"
        );

        // Target all SPIs to the boot core only.
        let mask = self.local_gic_target_mask();

        self.shared_registers.lock(|regs| {
            for i in regs.implemented_itargets_slice().iter() {
                i.write(
                    ITARGETSR::Offset3.val(mask)
                        + ITARGETSR::Offset2.val(mask)
                        + ITARGETSR::Offset1.val(mask)
                        + ITARGETSR::Offset0.val(mask),
                );
            }

            regs.CTLR.write(CTLR::Enable::SET);
        });
    }

    /// 启用指定的中断。根据中断号决定是访问分组寄存器还是共享寄存器。
    pub fn enable(&self, irq_num: &super::IRQNumber) {
        let irq_num = irq_num.get();

        // Each bit in the u32 enable register corresponds to one IRQ number. Shift right by 5
        // (division by 32) and arrive at the index for the respective ISENABLER[i].
        let enable_reg_index = irq_num >> 5;
        let enable_bit: u32 = 1u32 << (irq_num % 32);

        // Check if we are handling a private or shared IRQ.
        match irq_num {
            // Private.
            0..=31 => {
                let enable_reg = &self.banked_registers.ISENABLER;
                enable_reg.set(enable_reg.get() | enable_bit);
            }
            // Shared.
            _ => {
                let enable_reg_index_shared = enable_reg_index - 1;

                self.shared_registers.lock(|regs| {
                    let enable_reg = &regs.ISENABLER[enable_reg_index_shared];
                    enable_reg.set(enable_reg.get() | enable_bit);
                });
            }
        }
    }
}
