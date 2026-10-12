use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use linked_list_allocator::LockedHeap;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

const HEAP_START: u64 = 0x4444_4444_0000;
pub const HEAP_SIZE: usize = 32 * 1024 * 1024;
const FRAME_SIZE: u64 = 4096;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

struct RegionFrameAllocator {
    regions: &'static MemoryRegions,
    region: usize,
    next: u64,
}

unsafe impl FrameAllocator<Size4KiB> for RegionFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        loop {
            let region = self.regions.get(self.region)?;
            let start = region.start.max(self.next).next_multiple_of(FRAME_SIZE);
            if region.kind != MemoryRegionKind::Usable || start + FRAME_SIZE > region.end {
                self.region += 1;
                self.next = 0;
                continue;
            }
            self.next = start + FRAME_SIZE;
            return Some(PhysFrame::containing_address(PhysAddr::new(start)));
        }
    }
}

pub fn init_heap(physical_memory_offset: u64, regions: &'static MemoryRegions) {
    let mut mapper = unsafe { page_mapper(VirtAddr::new(physical_memory_offset)) };
    let mut frames = RegionFrameAllocator {
        regions,
        region: 0,
        next: 0,
    };
    let first = Page::containing_address(VirtAddr::new(HEAP_START));
    let last = Page::containing_address(VirtAddr::new(HEAP_START + HEAP_SIZE as u64 - 1));
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
    for page in Page::range_inclusive(first, last) {
        let frame = frames
            .allocate_frame()
            .expect("out of physical memory for the heap");
        unsafe {
            mapper
                .map_to(page, frame, flags, &mut frames)
                .expect("heap mapping failed")
                .flush();
        }
    }
    unsafe { ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE) };
}

pub fn heap_used() -> usize {
    ALLOCATOR.lock().used()
}

unsafe fn page_mapper(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let (level_4_frame, _) = Cr3::read();
    let table_address = physical_memory_offset + level_4_frame.start_address().as_u64();
    let level_4_table = &mut *table_address.as_mut_ptr::<PageTable>();
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}
