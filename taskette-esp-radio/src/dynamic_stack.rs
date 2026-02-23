use core::alloc::Layout;

use taskette::arch::StackAllocation;

extern crate alloc;

pub struct DynamicStack {
    ptr: *mut u8,
    layout: Layout,
}

impl DynamicStack {
    pub fn new(size: usize) -> Self {
        unsafe {
            let layout = alloc::alloc::Layout::from_size_align_unchecked(size, 16);
            Self {
                ptr: alloc::alloc::alloc(layout),
                layout,
            }
        }
    }
}

impl Drop for DynamicStack {
    fn drop(&mut self) {
        unsafe {
            alloc::alloc::dealloc(self.ptr, self.layout);
        }
    }
}

impl StackAllocation for DynamicStack {
    fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.layout.size()) }
    }
}
