use spin::LazyLock;
use x86_64::instructions::segmentation::{Segment, CS, DS, SS};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
const DOUBLE_FAULT_STACK_SIZE: usize = 4096 * 5;

static mut DOUBLE_FAULT_STACK: [u8; DOUBLE_FAULT_STACK_SIZE] = [0; DOUBLE_FAULT_STACK_SIZE];

static TSS: LazyLock<TaskStateSegment> = LazyLock::new(|| {
    let mut tss = TaskStateSegment::new();
    let stack_start = VirtAddr::from_ptr(&raw const DOUBLE_FAULT_STACK);
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] =
        stack_start + DOUBLE_FAULT_STACK_SIZE as u64;
    tss
});

static GDT: LazyLock<(GlobalDescriptorTable, SegmentSelector, SegmentSelector)> =
    LazyLock::new(|| {
        let mut gdt = GlobalDescriptorTable::new();
        let code = gdt.append(Descriptor::kernel_code_segment());
        let tss = gdt.append(Descriptor::tss_segment(&TSS));
        (gdt, code, tss)
    });

pub fn init() {
    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1);
        SS::set_reg(SegmentSelector::NULL);
        DS::set_reg(SegmentSelector::NULL);
        load_tss(GDT.2);
    }
}
