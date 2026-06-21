//! Optional global allocation counting for the benchmark harness.
//!
//! A binary installs [`CountingAlloc`] as its `#[global_allocator]`; query
//! benchmarks then bracket a call with [`reset`] / [`read`] to attribute the
//! allocation count and bytes to that query. Binaries that keep the default
//! allocator leave the counters at zero, so the same library code is unaffected.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

/// A pass-through `System` allocator that tallies allocation count and requested
/// bytes (allocations only; frees are not subtracted).
pub struct CountingAlloc;

// SAFETY: every method delegates to `System`, only adding relaxed atomic tallies.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        System.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        System.realloc(ptr, layout, new_size)
    }
}

/// Zero the allocation and byte tallies before a measured region.
pub fn reset() {
    ALLOCS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
}

/// Read the `(allocations, bytes)` tallied since the last [`reset`].
pub fn read() -> (u64, u64) {
    (
        ALLOCS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    )
}
