# Raspberry-OS
本项目时跟随Rust嵌入式社区的同名项目的代码实现，只支持qemu模拟。

地址空间————开启MMU，使用虚拟内存，恒等映射和偏移映射。

# 流程
kernel_init()

调用memory::mmu模块的初始化函数（memory::mmu是抽象接口，定义了MMU trait，现在只有两个函数——1.enable_mmu_and_caching，用于开启mmu，2.is_enabled，用于确认mmu已经开启。）

这个MMU trait的实现基于架构————所以具体实现在_arch代码中。
_arch/aarch64/memory/mmu.rs中定义了一个结构体`struct MemoryManagementUnit;`

为MemoryManagementUnit实现mmu trait：
```rust
impl memory::mmu::interface::MMU for MemoryManagementUnit {
    unsafe fn enable_mmu_and_caching(&self) -> Result<(), MMUEnableError> {
        if unlikely(self.is_enabled()) {
            return Err(MMUEnableError::AlreadyEnabled);
        }

        // Fail early if translation granule is not supported.
        if unlikely(!ID_AA64MMFR0_EL1.matches_all(ID_AA64MMFR0_EL1::TGran64::Supported)) {
            return Err(MMUEnableError::Other(
                "Translation granule not supported in HW",
            ));
        }

        // Prepare the memory attribute indirection register.
        self.set_up_mair();

        // Populate translation tables.
        KERNEL_TABLES
            .populate_tt_entries()
            .map_err(MMUEnableError::Other)?;

        // Set the "Translation Table Base Register".
        TTBR0_EL1.set_baddr(KERNEL_TABLES.phys_base_address());

        self.configure_translation_control();

        // Switch the MMU on.
        //
        // First, force all previous changes to be seen before the MMU is enabled.
        barrier::isb(barrier::SY);

        // Enable the MMU and turn on data and instruction caching.
        SCTLR_EL1.modify(SCTLR_EL1::M::Enable + SCTLR_EL1::C::Cacheable + SCTLR_EL1::I::Cacheable);

        // Force MMU init to complete before next instruction.
        barrier::isb(barrier::SY);

        Ok(())
    }

    #[inline(always)]
    fn is_enabled(&self) -> bool {
        SCTLR_EL1.matches_all(SCTLR_EL1::M::Enable)
    }
}
```
具体来说，`enable_mmu_and_caching`函数，先确认mmu未开启，
再使用ID_AA64MMFR0_EL1.matches_all方法，检查ID_AA64MMFR0_EL1
寄存器中与TGran64::Supported相关的位是否被设置，来判断硬件是否支持64KB的翻译粒度。
（因为代码中我们只会支持64KB的页大小）。
再设置MAIR_EL1寄存器。（具体意义略）
再调用`KERNEL_TABLES`的初始化函数`populate_tt_entries`，初始化翻译表的所有页描述符和表描述符。
再设置`TTBR0_EL1`寄存器为`KERNEL_TABLES.phys_base_address()`，也就是KERNEL_TABLES的物理基地址。
再开启MMU，
再开启指令和数据cache，
再加一条屏障指令，强制mmu初始化完成，避免下一条指令错误。

KERNEL_TABLES是具体与架构的代码，所以再_arch代码中。
KERNEL_TABLES是KernelTranslationTable的实例
`static mut KERNEL_TABLES: KernelTranslationTable = KernelTranslationTable::new();`
`pub type KernelTranslationTable = FixedSizeTranslationTable<NUM_LVL2_TABLES>;`
FixedSizeTranslationTable在
```rust
impl<const NUM_TABLES: usize> FixedSizeTranslationTable<NUM_TABLES> {
    /// 初始化一个空的翻译表
    pub const fn new() -> Self {
        assert!(NUM_TABLES > 0);
        Self {
            lvl3: [[PageDescriptor::new_zeroed(); 8192]; NUM_TABLES],
            lvl2: [TableDescriptor::new_zeroed(); NUM_TABLES],
        }
    }
    pub unsafe fn populate_tt_entries(&mut self) -> Result<(), &'static str> {
        for (l2_nr, l2_entry) in self.lvl2.iter_mut().enumerate() {
            *l2_entry =
                TableDescriptor::from_next_lvl_table_addr(self.lvl3[l2_nr].phys_start_addr_usize());

            for (l3_nr, l3_entry) in self.lvl3[l2_nr].iter_mut().enumerate() {
                let virt_addr = (l2_nr << Granule512MiB::SHIFT) + (l3_nr << Granule64KiB::SHIFT);

                let (phys_output_addr, attribute_fields) =
                    bsp::memory::mmu::virt_mem_layout().virt_addr_properties(virt_addr)?;

                *l3_entry = PageDescriptor::from_output_addr(phys_output_addr, &attribute_fields);
            }
        }

        Ok(())
    }
}
```
初始化后，在根据具体的板级内存架构，初始化填表：
具体就是`bsp::memory::mmu::virt_mem_layout().virt_addr_properties(virt_addr)?;`
```rust
pub static LAYOUT: KernelVirtualLayout<NUM_MEM_RANGES> = KernelVirtualLayout::new(
    memory_map::END_INCLUSIVE,
    [
        TranslationDescriptor {
            name: "Kernel code and RO data",
            virtual_range: code_range_inclusive,
            physical_range_translation: Translation::Identity,
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::CacheableDRAM,
                acc_perms: AccessPermissions::ReadOnly,
                execute_never: false,
            },
        },
        TranslationDescriptor {
            name: "Remapped Device MMIO",
            virtual_range: remapped_mmio_range_inclusive,
            physical_range_translation: Translation::Offset(memory_map::mmio::START + 0x20_0000),
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::Device,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        },
        TranslationDescriptor {
            name: "Device MMIO",
            virtual_range: mmio_range_inclusive,
            physical_range_translation: Translation::Identity,
            attribute_fields: AttributeFields {
                mem_attributes: MemAttributes::Device,
                acc_perms: AccessPermissions::ReadWrite,
                execute_never: true,
            },
        },
    ],
);

pub fn virt_mem_layout() -> &'static KernelVirtualLayout<NUM_MEM_RANGES> {
    &LAYOUT
}

```
KernelVirtualLayout定义了一些虚拟地址的分配和其映射的规则。这里将虚拟内存0x80000开始内核长度大小的
虚拟内存恒等映射到内核的物理内存。mmio也是恒等映射，但是多了一段remap_mmio，这表示访问remap_mmio的细腻
地址也会被偏移映射到mmio的物理内存。

KernelVirtualLayout有一个私有方法`virt_addr_properties`，输入一个virt_addr，返回`Result<(usize, AttributeFields), &'static str>`，
表明，若在这个内存结构中找到虚拟地址的分配记录，就返回该物理地址和属性（根据映射方式）。

