use crate::exclusive::Exclusive;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};
use linked_list_allocator::Heap;
pub struct Allocator(Exclusive<Heap>);
#[global_allocator]
pub static HEAP: Allocator = Allocator(Exclusive::new(Heap::empty()));
static LAST_USED: AtomicUsize = AtomicUsize::new(0);
// SAFETY: 同一固定 RAM 范围由 frames 独占保留且永不释放；所有堆操作受互斥守卫保护。
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut heap = self.0.lock();
        let result = heap
            .allocate_first_fit(layout)
            .map_or(core::ptr::null_mut(), NonNull::as_ptr);
        LAST_USED.store(heap.used(), Ordering::Relaxed);
        result
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        // SAFETY: GlobalAlloc 调用者保证该指针 / layout 来自此堆且仍独占，锁排除并发。
        unsafe {
            let mut heap = self.0.lock();
            heap.deallocate(NonNull::new(p).expect("non-null allocation"), layout);
            LAST_USED.store(heap.used(), Ordering::Relaxed);
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
pub fn last_used() -> usize {
    LAST_USED.load(Ordering::Relaxed)
}
pub fn stats() -> radishroot_kernel::runtime_contract::MemoryStats {
    use radishroot_kernel::runtime_contract::{MemoryScope, MemoryStats};
    let h = HEAP.0.lock();
    MemoryStats::new(
        h.size() as u64,
        h.size() as u64,
        h.used() as u64,
        MemoryScope::Heap,
    )
    .expect("heap accounting")
}
