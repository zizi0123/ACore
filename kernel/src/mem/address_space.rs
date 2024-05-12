use super::frame_allocator::FrameTracker;
use super::page_table::{PTEFlags, PageTable, VirtAddr, PPN, VPN};
use crate::config::{MEMORY_END, TRAMPOLINE_VA, PAGE_SIZE};
use crate::mem::page_table::PhyAddr;
use crate::sync::UPSafeCell;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use bitflags::bitflags;
use core::cmp::min
use xmas_elf::program;

lazy_static! {
    /// a memory set instance through lazy_static! managing kernel space
    pub static ref KERNEL_SPACE: UPSafeCell<AddressSpace> =
        unsafe { UPSafeCell::new(kernel_space()) };
}

bitflags! {
    pub struct SectionPermisson: usize {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }

}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MapType {
    Identical,
    Framed,
}

pub struct Section {
    start: VPN,
    end: VPN,
    permisson: SectionPermisson,
    map_type: MapType,
    v2p: BTreeMap<VPN, FrameTracker>,
}

impl Section {
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        permisson: SectionPermisson,
        map_type: MapType,
    ) -> Self {
        Self {
            start: start_va.to_down_vpn(),
            end: end_va.to_up_vpn(),
            permisson,
            map_type,
            v2p: BTreeMap::new(),
        }
    }

    pub fn copy_data(&mut self, page_table: &mut PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let len = data.len();
        for vpn in self.start..self.end {
            let src = &data[start..min(start + PAGE_SIZE, len)];
            let dst = &mut page_table
                .find_pte(vpn)
                .unwrap()
                .ppn()
                .get_page()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
        }
    }
}

pub struct AddressSpace {
    page_table: PageTable,
    sections: Vec<Section>,
}

impl AddressSpace {
    pub fn new() -> Self {
        Self {
            page_table: PageTable::new(),
            sections: Vec::new(),
        }
    }

    pub fn add_section(
        &mut self,
        start: VirtAddr,
        end: VirtAddr,
        permisson: SectionPermisson,
        map_type: MapType,
        data: Option<&[u8]>
    ) {
        let mut section = Section::new(start, end, permisson, map_type);
        println!(
            "new section vpn range: [{:#x}, {:#x})",
            section.start.0, section.end.0
        );
        for vpn in section.start..section.end {
            match map_type {
                MapType::Identical => {
                    let ppn = PPN(vpn.0); // for kernel space, mapping is identical, and there's no need to allocate a new frame.
                    self.page_table
                        .map(vpn, ppn, PTEFlags::from_bits(permisson.bits).unwrap());
                }
                MapType::Framed => {
                    let frame = self
                        .page_table
                        .map_and_alloc(vpn, PTEFlags::from_bits(permisson.bits).unwrap());
                    section.v2p.insert(vpn, frame);
                }
            }
        }
        if let Some(data) = data {
            section.copy_data(&mut self.page_table, data)
        }
        self.sections.push(section);
    }

    //section trampoline is mapped, but not added in `sections` of each address space.
    pub fn map_trampoline(&mut self) {
        let trampoline_vpn: VPN = VirtAddr(TRAMPOLINE_VA).to_down_vpn();
        let trampoline_ppn: PPN = PhyAddr(strampoline as usize).to_down_ppn();
        self.page_table
            .map(trampoline_vpn, trampoline_ppn, PTEFlags::R | PTEFlags::X);
    }

    pub fn test_space(&self) {
        println!("pass kernel space test!");
    }
}

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss_with_stack();
    fn ebss();
    fn ekernel();
    fn strampoline();
}

// build kernel space
pub fn kernel_space() -> AddressSpace {
    let mut kernel_space = AddressSpace::new();

    println!("start build kernel space!");

    //add trampoline section
    kernel_space.map_trampoline();

    //add text section of kernel
    kernel_space.add_section(
        (stext as usize).into(),
        (etext as usize).into(),
        SectionPermisson::R | SectionPermisson::X,
        MapType::Identical,
        None
    );
    println!(
        "add section: .text [{:#x}, {:#x})",
        stext as usize, etext as usize
    );

    //add rodata section of kernel
    kernel_space.add_section(
        (srodata as usize).into(),
        (erodata as usize).into(),
        SectionPermisson::R,
        MapType::Identical,
        None
    );
    println!(
        "add section: .rodata [{:#x}, {:#x})",
        srodata as usize, erodata as usize
    );

    //add data section of kernel
    kernel_space.add_section(
        (sdata as usize).into(),
        (edata as usize).into(),
        SectionPermisson::R | SectionPermisson::W,
        MapType::Identical,
        None
    );
    println!(
        "add section: .data [{:#x}, {:#x})",
        sdata as usize, edata as usize
    );

    //add bss section of kernel
    kernel_space.add_section(
        (sbss_with_stack as usize).into(),
        (ebss as usize).into(),
        SectionPermisson::R | SectionPermisson::W,
        MapType::Identical,
        None
    );
    println!(
        "add section: .bss [{:#x}, {:#x})",
        sbss_with_stack as usize, ebss as usize
    );

    //add physical frames
    kernel_space.add_section(
        (ekernel as usize).into(),
        (MEMORY_END as usize).into(),
        SectionPermisson::R | SectionPermisson::W,
        MapType::Identical,
        None
    );
    println!(
        "add section: .kernel [{:#x}, {:#x})",
        ekernel as usize, MEMORY_END as usize
    );

    //todo map memory-mapped registers for I/O

    return kernel_space;
}

// build user space from elf data
// return user space, user stack start address, and entry point of elf
pub fn user_space_from_elf(elf_data: &[u8]) -> (AddressSpace, VirtAddr, usize) {
    let mut user_space = AddressSpace::new();

    println!("start build a user space!");

    //add trampoline section
    user_space.map_trampoline();

    // map program headers of elf, with U flag
    let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
    let elf_header = elf.header;
    let magic = elf_header.pt1.magic;
    assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!"); // check magic number to ensure it's a valid elf file

    // add sections
    let program_header_cnt = elf_header.pt2.ph_count();
    let mut max_end_vpn = VPN(0);
    for i in 0..program_header_cnt {
        let ph = elf.program_header(i).unwrap();
        if ph.get_type().unwrap() == xmas_elf::program::Type::Load { // this section should be loaded into memory
            let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
            let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
            let mut map_perm = SectionPermisson::U;
            let ph_flags = ph.flags();
            if ph_flags.is_read() {
                map_perm |= SectionPermisson::R;
            }
            if ph_flags.is_write() {
                map_perm |= SectionPermisson::W;
            }
            if ph_flags.is_execute() {
                map_perm |= SectionPermisson::X;
            }

            max_end_vpn = end_va.to_up_vpn();
            user_space.add_section(start_va, end_va, map_perm, MapType::Framed,Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]))
        }
    }

    // add guard page and user stack
    let guard_start : VirtAddr = max_end_vpn.into();
    let user_stack_start = VirtAddr::new(guard_start.0 + PAGE_SIZE);
    
    return (
        user_space,
        user_stack_start,
        elf.header.pt2.entry_point() as usize,
    );
}
