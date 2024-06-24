use super::frame_allocator::FrameTracker;
use super::page_table::{PTEFlags, PageTable, PageTableEntry, VirtAddr, PPN, VPN};
use crate::config::{
    GREEN, MEMORY_END, MM_DERICT_MAP, PAGE_SIZE, RESET, TRAMPOLINE_START_VA, TRAP_CONTEXT_START_VA,
    USER_STACK_SIZE,
};
use crate::mem::page_table::PhyAddr;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::bitflags;
use core::arch::asm;
use core::cmp::min;
use riscv::register::satp;
use sync::UPSafeCell;

lazy_static! {
    // a memory set instance through lazy_static! managing kernel space
    // Arc is an atomic reference counted type, which can be shared between threads
    pub static ref KERNEL_SPACE: Arc<UPSafeCell<AddressSpace>> = Arc::new(
        unsafe { UPSafeCell::new(kernel_space()) });
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
            let dst =
                &mut page_table.find_and_alloc_pte(vpn).unwrap().ppn().get_page()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
        }
    }
}

fn copy_section(section: &Section, new_address_space: &mut AddressSpace) -> Section {
    println!("copy section: vpn range [{:#x}, {:#x})", section.start.0, section.end.0);
    let mut new_section = Section::new(
        section.start.into(),
        section.end.into(),
        section.permisson,
        section.map_type,
    );

    for (vpn, old_frame) in section.v2p.iter() {
        // map on page table
        let new_frame = new_address_space
            .page_table
            .map_and_alloc(*vpn, PTEFlags::from_bits(section.permisson.bits).unwrap());
        let src_data = old_frame.ppn.get_page();
        let dst_dara = new_frame.ppn.get_page();
        // copy data
        dst_dara.copy_from_slice(src_data);
        new_section.v2p.insert(*vpn, new_frame);
    }
    return new_section;
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
        data: Option<&[u8]>,
    ) {
        let mut section = Section::new(start, end, permisson, map_type);
        // println!(
        //     "new section va range: [{:#x}, {:#x})",
        //     start.0, end.0
        // );
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
            // let ppn = self.translate(vpn).unwrap();
            // println!("vpn {:#x} -> ppn {:#x}", vpn.0, ppn.0);
        }
        if let Some(data) = data {
            section.copy_data(&mut self.page_table, data)
        }
        self.sections.push(section);
    }

    pub fn delete_section(&mut self, start: VirtAddr) {
        let (idx, section) = self
            .sections
            .iter_mut()
            .enumerate()
            .find(|(_, section)| section.start == start.to_down_vpn())
            .unwrap();
        for vpn in section.start..section.end {
            let ppn = self.page_table.translate(vpn).unwrap();
            ppn.get_page().iter_mut().for_each(|x| *x = 0); // clear the page
            self.page_table.unmap(vpn);
        }
        if section.map_type == MapType::Framed {
            section.v2p.clear(); // free physical frames
        }
        self.sections.remove(idx);
    }

    //section trampoline is mapped, but not added in `sections` of each address space.
    pub fn map_trampoline(&mut self) {
        let trampoline_vpn: VPN = VirtAddr::from(TRAMPOLINE_START_VA).to_down_vpn();
        let trampoline_ppn: PPN = PhyAddr::from(strampoline as usize).to_down_ppn();
        self.page_table
            .map(trampoline_vpn, trampoline_ppn, PTEFlags::R | PTEFlags::X);
    }

    //add a trap context section in the address space. Allocate physical frames trap context.
    pub fn add_trap_context(&mut self) {
        let trap_context_start = VirtAddr::from(TRAP_CONTEXT_START_VA);
        let trap_context_end = VirtAddr::from(TRAMPOLINE_START_VA);
        self.add_section(
            trap_context_start,
            trap_context_end,
            SectionPermisson::R | SectionPermisson::W,
            MapType::Framed,
            None,
        );
    }

    pub fn get_pte(&self, vpn: VPN) -> Option<PageTableEntry> {
        return self.page_table.get_pte(vpn);
    }

    pub fn translate(&self, vpn: VPN) -> Option<PPN> {
        return self.page_table.translate(vpn);
    }

    //set satp register
    #[no_mangle]
    pub fn activate(&self) {
        println!("activate kernel space!");
        // set the MODE of satp to 8 to activate SV39 mode, and set the ppn of the root page table
        let satp = self.page_table.get_satp();
        print!("satp: -> {:#x} ", satp);
        unsafe {
            satp::write(satp);
            asm!("sfence.vma"); // flush TLB
        }
        println!("{}finish activate kernel space!{}", GREEN, RESET);
    }

    pub fn clear(&mut self) {
        self.sections.clear(); // each v2p in sections will be cleared, and the physical frames will be freed.
        self.page_table.clear(); // attention is this correct?
    }

    pub fn get_satp(&self) -> usize {
        return self.page_table.get_satp();
    }

    #[allow(unused)]
    pub fn shrink_heap_to(&mut self, heap_bottom: VirtAddr, new_brk: VirtAddr) -> bool {
        if let Some(heap) = self
            .sections
            .iter_mut()
            .find(|section| section.start == heap_bottom.to_down_vpn())
        {
            let new_brk_vpn = new_brk.to_up_vpn();
            for vpn in new_brk_vpn..heap.end {
                if heap.map_type == MapType::Framed {
                    heap.v2p.remove(&vpn);
                }
                self.page_table.unmap(vpn);
            }
            heap.end = new_brk_vpn;
            return true;
        } else {
            return false;
        }
    }
    #[allow(unused)]
    pub fn append_heap_to(&mut self, heap_bottom: VirtAddr, new_brk: VirtAddr) -> bool {
        if let Some(heap) = self
            .sections
            .iter_mut()
            .find(|section| section.start == heap_bottom.to_down_vpn())
        {
            let new_brk_vpn = new_brk.to_up_vpn();
            for vpn in heap.end..new_brk_vpn {
                match heap.map_type {
                    MapType::Identical => {
                        let ppn = PPN(vpn.0); // for kernel space, mapping is identical, and there's no need to allocate a new frame.
                        self.page_table.map(
                            vpn,
                            ppn,
                            PTEFlags::from_bits(heap.permisson.bits).unwrap(),
                        );
                    }
                    MapType::Framed => {
                        let frame = self
                            .page_table
                            .map_and_alloc(vpn, PTEFlags::from_bits(heap.permisson.bits).unwrap());
                        heap.v2p.insert(vpn, frame);
                    }
                }
            }
            heap.end = new_brk_vpn;
            return true;
        } else {
            return false;
        }
    }
}

pub fn test_space() {
    let kernel_space = KERNEL_SPACE.exclusive_access();

    let mid_text: VirtAddr = ((stext as usize + etext as usize) / 2).into();
    let mid_rodata: VirtAddr = ((srodata as usize + erodata as usize) / 2).into();
    let mid_data: VirtAddr = ((sdata as usize + edata as usize) / 2).into();
    assert_eq!(
        kernel_space
            .get_pte(mid_text.to_down_vpn())
            .unwrap()
            .is_writable(),
        false
    );
    assert_eq!(
        kernel_space
            .get_pte(mid_rodata.to_down_vpn())
            .unwrap()
            .is_writable(),
        false,
    );
    assert_eq!(
        kernel_space
            .get_pte(mid_data.to_down_vpn())
            .unwrap()
            .is_executable(),
        false,
    );
    println!("{}remap_test passed!{}", GREEN, RESET);
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

    println!("{}start build kernel space!{}", GREEN, RESET);

    //add trampoline section
    kernel_space.map_trampoline();

    //add text section of kernel

    print!(
        "{}add section: .text [{:#x}, {:#x})  {}",
        GREEN, stext as usize, etext as usize, RESET
    );
    kernel_space.add_section(
        (stext as usize).into(),
        (etext as usize).into(),
        SectionPermisson::R | SectionPermisson::X,
        MapType::Identical,
        None,
    );

    //add rodata section of kernel

    print!(
        "{}add section: .rodata [{:#x}, {:#x}) {}",
        GREEN, srodata as usize, erodata as usize, RESET
    );
    kernel_space.add_section(
        (srodata as usize).into(),
        (erodata as usize).into(),
        SectionPermisson::R,
        MapType::Identical,
        None,
    );

    //add data section of kernel
    print!(
        "add section: .data [{:#x}, {:#x}) ",
        sdata as usize, edata as usize
    );
    kernel_space.add_section(
        (sdata as usize).into(),
        (edata as usize).into(),
        SectionPermisson::R | SectionPermisson::W,
        MapType::Identical,
        None,
    );

    //add bss section of kernel
    print!(
        "add section: .bss [{:#x}, {:#x}) ",
        sbss_with_stack as usize, ebss as usize
    );
    kernel_space.add_section(
        (sbss_with_stack as usize).into(),
        (ebss as usize).into(),
        SectionPermisson::R | SectionPermisson::W,
        MapType::Identical,
        None,
    );

    // map physical frames [ekernel, MEMORY_END), so that the kernel can access the whole physical memory
    print!(
        "map physical frames [{:#x}, {:#x}) ",
        ekernel as usize, MEMORY_END as usize
    );
    kernel_space.add_section(
        (ekernel as usize).into(),
        (MEMORY_END as usize).into(),
        SectionPermisson::R | SectionPermisson::W,
        MapType::Identical,
        None,
    );

    //map IO space
    // print!("map IO space! ");
    for pair in MM_DERICT_MAP {
        kernel_space.add_section(
            pair.0.into(),
            (pair.0 + pair.1 - 1).into(),
            SectionPermisson::R | SectionPermisson::W,
            MapType::Identical,
            None,
        );
    }

    // kernel stack is allocated for each app when creating a new task

    println!("{}finish build kernel space!{}", GREEN, RESET);

    return kernel_space;
}

// build user space from elf data
// return user space, user stack start address, and entry point of elf
pub fn user_space_from_elf(elf_data: &[u8]) -> (AddressSpace, VirtAddr, usize) {
    // println!("start build a user space!");
    let mut user_space = AddressSpace::new();

    //map trampoline to the highest page
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
        if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
            // this section should be loaded into memory
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
            user_space.add_section(
                start_va,
                end_va,
                map_perm,
                MapType::Framed,
                Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
            )
        }
    }

    // add guard page
    let guard_start: VirtAddr = max_end_vpn.into();
    let user_stack_start = VirtAddr::from(guard_start.0 + PAGE_SIZE);

    // add stack
    let user_stack_end = VirtAddr::from(user_stack_start.0 + USER_STACK_SIZE);
    // println!("add stack");
    user_space.add_section(
        user_stack_start,
        user_stack_end,
        SectionPermisson::U | SectionPermisson::R | SectionPermisson::W,
        MapType::Framed,
        None,
    );

    // add heap
    let user_heap_start = user_stack_end;
    let user_heap_end = user_heap_start;
    // println!("add heap");
    user_space.add_section(
        user_heap_start,
        user_heap_end,
        SectionPermisson::U | SectionPermisson::R | SectionPermisson::W,
        MapType::Framed,
        None,
    );

    // add trap context
    // println!("add trap context");
    user_space.add_trap_context();

    return (
        user_space,
        user_stack_end,
        elf.header.pt2.entry_point() as usize,
    );
}

pub fn copy_address_space(parent_address_space: &AddressSpace) -> AddressSpace {
    let mut address_space = AddressSpace::new();
    address_space.map_trampoline();
    // println!("finish map trampoline");
    for section in parent_address_space.sections.iter() {
        let new_section = copy_section(section, &mut address_space);
        address_space.sections.push(new_section);
    }
    return address_space;
}
