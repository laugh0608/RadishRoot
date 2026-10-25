use crate::exclusive::Exclusive;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};
use linked_list_allocator::Heap;
pub struct Allocator(Exclusive<Heap>);
#[global_allocator]
pub static HEAP: Allocator = Allocator(Exclusive::new(Heap::empty()));
// SAFETY: 同一固定 RAM 范围由 frames 独占保留且永不释放；所有堆操作受互斥守卫保护。
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .lock()
            .allocate_first_fit(layout)
            .map_or(core::ptr::null_mut(), NonNull::as_ptr)
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        // SAFETY: GlobalAlloc 调用者保证该指针 / layout 来自此堆且仍独占，锁排除并发。
        unsafe {
            self.0
                .lock()
                .deallocate(NonNull::new(p).expect("non-null allocation"), layout);
        }
    }
}
pub unsafe fn init(base: *mut u8, bytes: usize) {
    let mut h = HEAP.0.lock();
    assert_eq!(h.size(), 0);
    // SAFETY: 调用方提供已映射、独占且永久保留的连续 RAM；范围尚无活动引用。
    unsafe {
        h.init(base, bytes);
    }
}
pub fn used() -> usize {
    HEAP.0.lock().used()
}
