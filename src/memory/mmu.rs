#[cfg(target_arch = "aarch64")]
#[path = "../arch/aarch64/memory/mmu.rs"]
mod arch_mmu;

mod translation_table;

use crate::common;
use core::{fmt, ops::RangeInclusive};

pub use arch_mmu::mmu;

/// 表示启用内存管理单元（MMU）时可能遇到的错误
#[derive(Debug)]
pub enum MMUEnableError {
    AlreadyEnabled,
    Other(&'static str),
}

/// Memory Management interfaces.
pub mod interface {
    use super::*;

    /// MMU functions.
    pub trait MMU {
        /// Called by the kernel during early init. Supposed to take the translation tables from the
        /// `BSP`-supplied `virt_mem_layout()` and install/activate them for the respective MMU.
        ///
        /// # Safety
        ///
        /// - Changes the HW's global state.
        unsafe fn enable_mmu_and_caching(&self) -> Result<(), MMUEnableError>;
        /// Returns true if the MMU is enabled, false otherwise.
        fn is_enabled(&self) -> bool;
    }
}

/// 用于描述翻译粒度的特性
pub struct TranslationGranule<const GRANULE_SIZE: usize>;
/// 用于描述地址空间的特性,用于指定地址空间的大小
pub struct AddressSpace<const AS_SIZE: usize>;

///
/// Identity：表示地址转换的身份映射，即虚拟地址和物理地址相同。
/// Offset(usize)：表示地址转换通过一个固定的偏移量来完成，其中usize类型的参数offset指定了偏移量的值。
///
#[allow(dead_code)]
#[derive(Copy, Clone)]
pub enum Translation {
    Identity,
    Offset(usize),
}

///
/// CacheableDRAM：表示具有缓存功能的动态随机存取内存（DRAM）。
/// 这种内存类型通常可以进行缓存，以提高访问速度。
/// Device：表示设备内存。这种内存通常用于与外部硬件进行通信，可能不进行缓存，
/// 以确保数据的一致性和实时性。
///
#[derive(Copy, Clone)]
pub enum MemAttributes {
    CacheableDRAM,
    Device,
}

#[derive(Copy, Clone)]
pub enum AccessPermissions {
    ReadOnly,
    ReadWrite,
}

///
/// mem_attributes: MemAttributes：
/// 这是一个 MemAttributes 枚举类型的字段，表示内存的属性，如缓存类型或设备内存。
/// acc_perms: AccessPermissions：
/// 这是一个 AccessPermissions 类型的字段，表示内存访问权限，如读、写、执行权限。这里假设 AccessPermissions 是另一个定义好的类型或结构体。
/// execute_never: bool：
/// 这是一个布尔字段，表示是否禁止执行（Execute Never）。当设置为 true 时，表示该内存区域不能被执行。
///
#[derive(Copy, Clone)]
pub struct AttributeFields {
    pub mem_attributes: MemAttributes,
    pub acc_perms: AccessPermissions,
    pub execute_never: bool,
}

///
/// 定义了一个名为 TranslationDescriptor 的结构体，用于描述内存的翻译描述符
///
/// name:
/// 这个字段用于标识翻译描述符的名称，便于在调试或日志中识别和引用。
/// virtual_range:
/// 这个函数指针返回一个 RangeInclusive<usize>，指定了虚拟地址的范围。这个范围定义了描述符对应的虚拟地址空间的起始和结束地址。
/// physical_range_translation:
/// 这个字段定义了物理地址的翻译规则，可能涉及地址转换、偏移计算或直接映射等。使用 Translation 类型可以灵活定义各种翻译逻辑。
/// attribute_fields:
/// 这个字段包含了内存属性，如缓存属性、访问权限等。使用 AttributeFields 结构体可以方便地管理和设置这些属性
///
pub struct TranslationDescriptor {
    pub name: &'static str,
    pub virtual_range: fn() -> RangeInclusive<usize>,
    pub physical_range_translation: Translation,
    pub attribute_fields: AttributeFields,
}

///
/// 定义了一个名为 KernelVirtualLayout 的泛型结构体，它包含有关内核虚拟地址空间布局的信息
///
/// max_virt_addr_inclusive: usize:
/// 这是一个表示虚拟地址空间最大地址的字段。usize 类型通常用于表示地址或大小。
/// 这个字段存储虚拟地址空间的上限，包含在内。
/// inner: [TranslationDescriptor; NUM_SPECIAL_RANGES]:
/// 这是一个数组，包含 NUM_SPECIAL_RANGES 个 TranslationDescriptor 实例。每个 TranslationDescriptor 描述一个特定的内存区域的翻译信息。
/// 使用数组可以方便地存储多个描述符，管理不同的内存区域。
///
pub struct KernelVirtualLayout<const NUM_SPECIAL_RANGES: usize> {
    /// The last (inclusive) address of the address space.
    max_virt_addr_inclusive: usize,

    /// Array of descriptors for non-standard (normal cacheable DRAM) memory regions.
    inner: [TranslationDescriptor; NUM_SPECIAL_RANGES],
}

impl fmt::Display for MMUEnableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MMUEnableError::AlreadyEnabled => write!(f, "MMU is already enabled"),
            MMUEnableError::Other(x) => write!(f, "{}", x),
        }
    }
}

impl<const GRANULE_SIZE: usize> TranslationGranule<GRANULE_SIZE> {
    /// The granule's size.
    pub const SIZE: usize = Self::size_checked();
    /// The granule's shift, aka log2(size).
    pub const SHIFT: usize = Self::SIZE.trailing_zeros() as usize;

    const fn size_checked() -> usize {
        assert!(GRANULE_SIZE.is_power_of_two());

        GRANULE_SIZE
    }
}

impl<const AS_SIZE: usize> AddressSpace<AS_SIZE> {
    /// The address space size.
    pub const SIZE: usize = Self::size_checked();

    /// The address space shift, aka log2(size).
    pub const SIZE_SHIFT: usize = Self::SIZE.trailing_zeros() as usize;
    const fn size_checked() -> usize {
        assert!(AS_SIZE.is_power_of_two());

        // Check for architectural restrictions as well.
        Self::arch_address_space_size_sanity_checks();

        AS_SIZE
    }
}

impl Default for AttributeFields {
    fn default() -> AttributeFields {
        AttributeFields {
            mem_attributes: MemAttributes::CacheableDRAM,
            acc_perms: AccessPermissions::ReadWrite,
            execute_never: true,
        }
    }
}

/// Human-readable output of a TranslationDescriptor.
impl fmt::Display for TranslationDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Call the function to which self.range points, and dereference the result, which causes
        // Rust to copy the value.
        let start = *(self.virtual_range)().start();
        let end = *(self.virtual_range)().end();
        let size = end - start + 1;

        let (size, unit) = common::size_human_readable_ceil(size);

        let attr = match self.attribute_fields.mem_attributes {
            MemAttributes::CacheableDRAM => "C",
            MemAttributes::Device => "Dev",
        };

        let acc_p = match self.attribute_fields.acc_perms {
            AccessPermissions::ReadOnly => "RO",
            AccessPermissions::ReadWrite => "RW",
        };

        let xn = if self.attribute_fields.execute_never {
            "PXN"
        } else {
            "PX"
        };

        write!(
            f,
            "      {:#010x} - {:#010x} | {: >3} {} | {: <3} {} {: <3} | {}",
            start, end, size, unit, attr, acc_p, xn, self.name
        )
    }
}

impl<const NUM_SPECIAL_RANGES: usize> KernelVirtualLayout<{ NUM_SPECIAL_RANGES }> {
    /// Create a new instance.
    pub const fn new(max: usize, layout: [TranslationDescriptor; NUM_SPECIAL_RANGES]) -> Self {
        Self {
            max_virt_addr_inclusive: max,
            inner: layout,
        }
    }

    /// For a virtual address, find and return the physical output address and corresponding
    /// attributes.
    ///
    /// If the address is not found in `inner`, return an identity mapped default with normal
    /// cacheable DRAM attributes.
    ///
    pub fn virt_addr_properties(
        &self,
        virt_addr: usize,
    ) -> Result<(usize, AttributeFields), &'static str> {
        if virt_addr > self.max_virt_addr_inclusive {
            return Err("Address out of range");
        }

        for i in self.inner.iter() {
            if (i.virtual_range)().contains(&virt_addr) {
                let output_addr = match i.physical_range_translation {
                    Translation::Identity => virt_addr,
                    Translation::Offset(a) => a + (virt_addr - (i.virtual_range)().start()),
                };

                return Ok((output_addr, i.attribute_fields));
            }
        }

        Ok((virt_addr, AttributeFields::default()))
    }

    /// Print the memory layout.
    pub fn print_layout(&self) {
        use crate::info;

        for i in self.inner.iter() {
            info!("{}", i);
        }
    }
}
