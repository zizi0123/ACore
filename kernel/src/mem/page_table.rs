use core::{iter::Step, result};

use super::frame_allocator::{alloc_frame, FrameTracker};
use crate::config::*;
use alloc::vec::Vec;
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
        self.0
    }
}

impl PhyAddr {
    pub fn to_down_ppn(&self) -> PPN {
        return PPN(self.0 >> PAGE_SIZE_BITS);
    }

    pub fn to_up_ppn(&self) -> PPN {
        return PPN((self.0 + (1 << PAGE_SIZE_BITS) - 1) >> PAGE_SIZE_BITS);
    }
}

impl VirtAddr {
    pub fn new(val: usize) -> Self {
        VirtAddr(val)
    }
    
    pub fn to_down_vpn(&self) -> VPN {
        return VPN(self.0 >> PAGE_SIZE_BITS);
    }

    pub fn to_up_vpn(&self) -> VPN {
        return VPN((self.0 + (1 << PAGE_SIZE_BITS) - 1) >> PAGE_SIZE_BITS);
    }
}

impl From<VPN> for VirtAddr {
    fn from(v: VPN) -> Self {
        Self(v.0 << PAGE_SIZE_BITS)
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

    pub fn ppn(&self) -> PPN {
        return PPN(self.bits >> 10);
    }


    pub fn is_valid(&self) -> bool {
        return self.bits & PTEFlags::V.bits != 0;
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
        println!("alloc frame {} at ppn: {:#x} for root",frames.len()+1 ,root_ppn.0);
        frames.push(root_frame);
        Self { root_ppn, frames }
    }
    pub fn load_from_satp(satp: usize) -> Self {
        let root_ppn = satp & ((1 << 44) - 1);
        Self {
            root_ppn: PPN(root_ppn),
            frames: Vec::new(),
        }
    }

    //find the pte of the given vpn, if a node on the page table tree dosen't exist, allocate a new frame for it.
    pub fn find_pte(&mut self, vpn: VPN) -> Option<&mut PageTableEntry> {
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
            if !pte.is_valid() && i != 2 {
                //allocate a new frame, and set the pte
                let frame = alloc_frame().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                println!("alloc frame {} at ppn: {:#x} for level {},valid = {}", self.frames.len()+1,frame.ppn.0, i + 1, pte.is_valid());
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

    //construct map between vpn and ppn (used for identity mapping in the kernel)
    pub fn map(&mut self, vpn: VPN, ppn: PPN, flags: PTEFlags) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:#x} has been mapped already", vpn.0);
        *pte = PageTableEntry::new(ppn, flags);
    }

    //and allocate a physical frame for virtual page, and construct map between vpn and ppn (used for user space mapping)
    pub fn map_and_alloc(&mut self, vpn: VPN, flags: PTEFlags) -> FrameTracker {
        let frame = alloc_frame().unwrap();
        self.map(vpn, frame.ppn, flags);
        return frame;
    }
}
