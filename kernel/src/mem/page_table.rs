use core::{iter::Step, usize};

use super::frame_allocator::{alloc_frame, FrameTracker};
use crate::config::*;
use alloc::{string::String, vec::Vec};
use bitflags::bitflags;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhyAddr(pub usize);

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtAddr(pub usize);

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PPN(pub usize);

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VPN(pub usize);

impl From<usize> for PhyAddr {
    //take the lower 56 bits
    fn from(addr: usize) -> Self {
        return PhyAddr(addr & ((1 << PA_WIDTH_SV39) - 1));
    }
}

impl From<usize> for VirtAddr {
    //take the lower 39 bits
    fn from(addr: usize) -> Self {
        return VirtAddr(addr & ((1 << VA_WIDTH_SV39) - 1));
    }
}

impl Into<usize> for PhyAddr {
    fn into(self) -> usize {
        self.0
    }
}

impl Into<usize> for VirtAddr {
    fn into(self) -> usize {
        if self.0 >= (1 << (VA_WIDTH_SV39 - 1)) {
            return self.0 | (!((1 << VA_WIDTH_SV39) - 1));
        } else {
            return self.0;
        }
    }
}

impl PhyAddr {
    pub fn to_down_ppn(&self) -> PPN {
        return PPN(self.0 / PAGE_SIZE);
    }

    pub fn to_up_ppn(&self) -> PPN {
        if self.0 == 0 {
            return PPN(0);
        }
        return PPN((self.0 + PAGE_SIZE - 1) / PAGE_SIZE);
    }
}

impl VirtAddr {
    pub fn to_down_vpn(&self) -> VPN {
        return VPN(self.0 / PAGE_SIZE);
    }

    pub fn to_up_vpn(&self) -> VPN {
        if self.0 == 0 {
            return VPN(0);
        }
        return VPN((self.0 + PAGE_SIZE - 1) / PAGE_SIZE);
    }

    pub fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }
}

impl From<VPN> for VirtAddr {
    fn from(v: VPN) -> Self {
        Self(v.0 << PAGE_SIZE_BITS)
    }
}

impl From<PPN> for PhyAddr {
    fn from(p: PPN) -> Self {
        Self(p.0 << PAGE_SIZE_BITS)
    }
}

impl PPN {
    pub fn new() -> Self {
        PPN(0)
    }

    fn get_pte(&self, idx: usize) -> &'static mut PageTableEntry {
        let addr = (self.0 << PAGE_SIZE_BITS) + idx * 8;
        let ptr = addr as *mut PageTableEntry;
        return unsafe { &mut *(ptr) };
    }

    pub fn get_page(&self) -> &'static mut [u8] {
        let addr = self.0 << PAGE_SIZE_BITS;
        return unsafe { core::slice::from_raw_parts_mut(addr as *mut u8, 4096) };
    }
}

impl Step for VPN {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if end.0 >= start.0 {
            return Some(end.0 - start.0);
        } else {
            return None;
        }
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        return Some(VPN(start.0 + count));
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        return Some(VPN(start.0 - count));
    }
}

impl VPN {
    pub fn step(&mut self) {
        self.0 += 1;
    }
}

bitflags! {
    pub struct PTEFlags: usize {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
    }
}

#[derive(Copy, Clone)]
pub struct PageTableEntry {
    pub bits: usize,
}

impl PageTableEntry {
    pub fn new(ppn: PPN, flags: PTEFlags) -> Self {
        Self {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }

    pub fn empty() -> Self {
        //for clear a pte
        PageTableEntry { bits: 0 }
    }

    pub fn ppn(&self) -> PPN {
        return PPN(self.bits >> 10);
    }

    pub fn is_valid(&self) -> bool {
        return self.bits & PTEFlags::V.bits != 0;
    }

    pub fn is_writable(&self) -> bool {
        return self.bits & PTEFlags::W.bits != 0;
    }

    pub fn is_executable(&self) -> bool {
        return self.bits & PTEFlags::X.bits != 0;
    }
}

pub struct PageTable {
    root_ppn: PPN,
    //the frames of the nodes on page table tree. When a PageTable is dropped, all the frames will be recycled.
    frames: Vec<FrameTracker>,
}

impl PageTable {
    pub fn new() -> Self {
        //allocate a frame for the root node
        let mut frames = Vec::new();
        let root_frame = alloc_frame().unwrap();
        let root_ppn = root_frame.ppn;
        // println!(
        //     "alloc frame {} at ppn: {:#x} for root",
        //     frames.len() + 1,
        //     root_ppn.0
        // );
        frames.push(root_frame);
        Self { root_ppn, frames }
    }

    // switch to a page table specified by satp
    // we only use traslation in a page table build by this function.
    // the frames dosn't matter, because all translation are done by memory address.
    pub fn new_from_satp(satp: usize) -> Self {
        let root_ppn = PPN(satp & ((1 << 44) - 1));
        Self {
            root_ppn,
            frames: Vec::new(),
        }
    }

    pub fn get_satp(&self) -> usize {
        return 8usize << 60 | self.root_ppn.0;
    }

    //find the pte of the given vpn, if a node on the page table tree dosen't exist, allocate a new frame for it.
    pub fn find_and_alloc_pte(&mut self, vpn: VPN) -> Option<&mut PageTableEntry> {
        let idx3: usize = vpn.0 & 511;
        let idx2: usize = (vpn.0 >> 9) & 511;
        let idx1: usize = (vpn.0 >> 18) & 511;
        let idx: [usize; 3] = [idx1, idx2, idx3];
        // println!(
        //     "find pte for vpn: {:#x}, idx: [{:#x}, {:#x}, {:#x}]",
        //     vpn.0, idx1, idx2, idx3
        // );
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for i in 0..3 {
            let pte = ppn.get_pte(idx[i]);
            // println!(
            //     "fine pte of offset {:#x} at page of ppn {:#x}, and valid = {}",
            //     idx[i],
            //     ppn.0,
            //     pte.is_valid()
            // );
            if !pte.is_valid() && i != 2 {
                //allocate a new frame, and set the pte
                let frame = alloc_frame().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                // println!(
                //     "alloc frame {} at ppn: {:#x} for level {},valid = {}",
                //     self.frames.len() + 1,
                //     frame.ppn.0,
                //     i + 1,
                //     pte.is_valid()
                // );
                self.frames.push(frame);
            }
                if i == 2 {
                result = Some(pte);
                break;
            }
            ppn = pte.ppn();
        }
        return result;
    }

    //find the pte of the given vpn, if a node on the page table tree dosen't exist, return None.
    pub fn find_pte(&self, vpn: VPN) -> Option<&mut PageTableEntry> {
        let idx3: usize = vpn.0 & 511;
        let idx2: usize = (vpn.0 >> 9) & 511;
        let idx1: usize = (vpn.0 >> 18) & 511;
        let idx: [usize; 3] = [idx1, idx2, idx3];
        // println!("find pte for vpn: {:#x}, idx: [{:#x}, {:#x}, {:#x}]", vpn.0, idx1, idx2, idx3);
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for i in 0..3 {
            let pte = ppn.get_pte(idx[i]);
            // println!("fine pte of offset {} at ppn {}, and valid = {}", idx[i], ppn.0, pte.is_valid());
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        return result;
    }

    //construct map between vpn and ppn 
    pub fn map(&mut self, vpn: VPN, ppn: PPN, flags: PTEFlags) {
        let pte = self.find_and_alloc_pte(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:#x} has been mapped already", vpn.0);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    //allocate a physical frame for virtual page, and construct map between vpn and ppn 
    pub fn map_and_alloc(&mut self, vpn: VPN, flags: PTEFlags) -> FrameTracker {
        let frame = alloc_frame().unwrap();
        self.map(vpn, frame.ppn, flags | PTEFlags::V);
        return frame;
    }

    #[allow(unused)]
    pub fn unmap(&mut self, vpn: VPN) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(
            pte.is_valid(),
            "vpn {:?} is invalid before unmapping",
            vpn.0
        );
        *pte = PageTableEntry::empty();
    }

    // get the value from find_pte(), from a reference to a value
    pub fn get_pte(&self, vpn: VPN) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }

    pub fn translate(&self, vpn: VPN) -> Option<PPN> {
        let pte = self.get_pte(vpn);
        return pte.map(|pte| pte.ppn());
    }

    pub fn clear(&mut self) {
        self.frames.clear();
    }
}

// from a start ptr get bytes with length len
pub fn physical_bytes_of_user_ptr(satp: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::new_from_satp(satp);
    let mut start = ptr as usize;
    let end = start + len;
    let mut bytes = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.to_down_vpn();
        let ppn = page_table.translate(vpn).unwrap();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            bytes.push(&mut ppn.get_page()[start_va.page_offset()..]);
        } else {
            bytes.push(&mut ppn.get_page()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    return bytes;
}

// from a start ptr get a string (read until '\0')
pub fn get_string(satp: usize, ptr: *const u8) -> String {
    let page_table = PageTable::new_from_satp(satp);
    let mut string = String::new();
    let va: VirtAddr = (ptr as usize).into();
    let mut vpn: VPN = va.to_down_vpn();
    let mut offset = va.page_offset();
    loop {
        let ppn = page_table.translate(vpn);
        let data = ppn.unwrap().get_page();
        while offset < PAGE_SIZE {
            let ch = data[offset] as char;
            if ch != '\0' {
                string.push(ch);
                offset += 1;
            } else {
                return string;
            }
        }
        vpn.step();
        offset = 0;
    }
}

pub fn write_into<T>(satp: usize, ptr: *mut T, value: T) {
    let page_table = PageTable::new_from_satp(satp);
    let va: VirtAddr = (ptr as usize).into();
    let vpn: VPN = va.to_down_vpn();
    let offset = va.page_offset();
    let ppn = page_table.translate(vpn).unwrap();
    let pa_aligned: PhyAddr = ppn.into();
    let pa = pa_aligned.0 + offset;
    let ptr = pa as *mut T;
    unsafe {
        ptr.write(value);
    }
}
   
    
