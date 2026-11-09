//! 单 BSP 的短临界区。禁中断后尝试占有；递归 / 竞争立即停止，不无界自旋。
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};
pub struct Exclusive<T> {
    held: AtomicBool,
    value: UnsafeCell<T>,
}
// SAFETY: 只经互斥 guard 解引用；guard 禁中断且禁止递归，跨 CPU 由原子占有拒绝并发。
unsafe impl<T: Send> Sync for Exclusive<T> {}
impl<T> Exclusive<T> {
    pub const fn new(value: T) -> Self {
        Self {
            held: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }
    pub fn lock(&self) -> Guard<'_, T> {
        let restore = x86_64::instructions::interrupts::are_enabled();
        x86_64::instructions::interrupts::disable();
        if self
            .held
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            panic!("T03 recursive/concurrent resource access");
        }
        Guard {
            owner: self,
            restore,
        }
    }
}
pub struct Guard<'a, T> {
    owner: &'a Exclusive<T>,
    restore: bool,
}
impl<T> Deref for Guard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // SAFETY: guard 唯一持有本锁；不能借出超过 guard 生命周期的引用。
        unsafe { &*self.owner.value.get() }
    }
}
impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: 同上，唯一 guard 且自身可变借用排除同时访问。
        unsafe { &mut *self.owner.value.get() }
    }
}
impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.owner.held.store(false, Ordering::Release);
        if self.restore {
            x86_64::instructions::interrupts::enable();
        }
    }
}
