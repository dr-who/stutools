use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};
thread_local! {static ACTIVE:Cell<bool>=const{Cell::new(false)};static COUNT:Cell<u64>=const{Cell::new(0)};}
pub struct Audited;
fn record() {
    let _ = ACTIVE.try_with(|active| {
        if active.get() {
            let _ = COUNT.try_with(|n| n.set(n.get() + 1));
        }
    });
}
unsafe impl GlobalAlloc for Audited {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        record();
        System.alloc(l)
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        record();
        System.alloc_zeroed(l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        record();
        System.realloc(p, l, n)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
}
pub struct Guard;
impl Guard {
    pub fn start() -> Self {
        COUNT.with(|n| n.set(0));
        ACTIVE.with(|a| a.set(true));
        Self
    }
    pub fn count(&self) -> u64 {
        COUNT.with(Cell::get)
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|a| a.set(false));
    }
}
