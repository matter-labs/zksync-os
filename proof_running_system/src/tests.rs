#![allow(dead_code)]

use crate::talc::TalcWrapper;
use alloc::vec;
use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use std::time::Instant;
use talc::ClaimOnOom;
use talc::ErrOnOom;
use talc::Span;
use talc::Talc;

#[test]
fn test_talc_huge_allocation() {
    let mut backing = vec![0usize; 1 << 27];
    for dst in backing.iter_mut() {
        *dst = 0;
    }
    let size = backing.len() * core::mem::size_of::<usize>();
    unsafe {
        let oom_handler = ClaimOnOom::new(Span::from_base_size(backing.as_mut_ptr().cast(), size));
        let allocator = Talc::new(oom_handler);
        let allocator = TalcWrapper::new(allocator);
        // let mut huge_vec: Vec<u8, _> = Vec::try_with_capacity_in(1 << 27, &allocator).unwrap();
        let mut huge_vec: Vec<u8, _> = Vec::with_capacity_in(1 << 27, &allocator);
        assert!(huge_vec.capacity() == 1 << 27);
        huge_vec.push(0);
    }
}

fn benchmark_allocator(allocator: &dyn GlobalAlloc, name: &str) {
    const BENCH_DURATION: f64 = 5.0;

    let mut active_allocations = Vec::new();

    let mut alloc_ticks_vec = Vec::new();
    let mut dealloc_ticks_vec = Vec::new();

    for i in 1..10000 {
        let layout = Layout::from_size_align(i * 8, 8).unwrap();
        let ptr = unsafe { allocator.alloc(layout) };
        assert!(!ptr.is_null());
        unsafe {
            let _ = ptr.read_volatile();
        }
        unsafe {
            allocator.dealloc(ptr, layout);
        }
    }

    let bench_timer = Instant::now();
    for i in 0.. {
        if i % 0x10000 == 0 && (Instant::now() - bench_timer).as_secs_f64() > BENCH_DURATION {
            break;
        }

        let size = fastrand::usize(1..1 << 22);
        let align = 8 << (fastrand::u16(..).trailing_zeros() / 2);
        let layout = Layout::from_size_align(size, align).unwrap();

        let alloc_begin = Instant::now();
        let alloc = unsafe { allocator.alloc(layout) };
        let alloc_ticks = alloc_begin.elapsed();

        if !alloc.is_null() {
            alloc_ticks_vec.push(alloc_ticks);
            active_allocations.push((alloc, layout));
        } else {
            for (ptr, layout) in active_allocations.drain(..) {
                let dealloc_begin = Instant::now();
                unsafe {
                    allocator.dealloc(ptr, layout);
                }
                let dealloc_ticks = dealloc_begin.elapsed();
                dealloc_ticks_vec.push(dealloc_ticks);
            }
            continue;
        }

        if active_allocations.len() > 10 && fastrand::usize(..10) == 0 {
            for _ in 0..8 {
                let index = fastrand::usize(..active_allocations.len());
                let allocation = active_allocations.swap_remove(index);

                let dealloc_begin = Instant::now();
                unsafe {
                    allocator.dealloc(allocation.0, allocation.1);
                }
                let dealloc_ticks = dealloc_begin.elapsed();
                dealloc_ticks_vec.push(dealloc_ticks);
            }
        }
    }

    alloc_ticks_vec.sort_unstable();
    dealloc_ticks_vec.sort_unstable();
    let alloc_ticks = alloc_ticks_vec
        .into_iter()
        .map(|x| x.as_nanos() as f64)
        .collect::<Vec<_>>();
    let dealloc_ticks = dealloc_ticks_vec
        .into_iter()
        .map(|x| x.as_nanos() as f64)
        .collect::<Vec<_>>();

    let alloc_ticks = remove_outliers_sorted(&alloc_ticks);
    let dealloc_ticks = remove_outliers_sorted(&dealloc_ticks);

    println!(
        "|{:>22} | {:>42} | {:>42} |",
        name,
        format!("{} ({})", mean(alloc_ticks), stddev(alloc_ticks)),
        format!("{} ({})", mean(dealloc_ticks), stddev(dealloc_ticks))
    );
}

fn mean(data: &[f64]) -> f64 {
    data.iter().sum::<f64>() / data.len() as f64
}

fn var(data: &[f64]) -> f64 {
    let mean = mean(data);
    data.iter().map(|&x| (x - mean) * (x - mean)).sum::<f64>() / data.len() as f64
}

fn stddev(data: &[f64]) -> f64 {
    var(data).sqrt()
}

fn remove_outliers_sorted(data: &[f64]) -> &[f64] {
    // assert!(data.is_sorted());
    let upper_bound = mean(data) + 50.0 * stddev(data);

    let mut i = data.len();
    while i > 0 {
        i -= 1;
        if data[i] < upper_bound {
            return &data[..=i];
        }
    }

    data
}

#[test]
fn test_heap_efficiency() {
    println!("|             Allocator | Average Random Actions Heap Efficiency |");
    println!("| --------------------- | -------------------------------------- |");

    const CAPACITY: usize = 1 << 24;
    let mut backing = vec![0u8; CAPACITY];

    let start = backing.as_mut_ptr_range().start;
    let end = backing.as_mut_ptr_range().end;

    let talc = Talc::new(ErrOnOom).lock::<talc::locking::AssumeUnlockable>();
    unsafe { talc.lock().claim(Span::from(start..end)) }.unwrap();

    let efficiency = heap_efficiency(&talc, CAPACITY);
    println!("|{:>22} | {:>38} |", "Talc", format!("{:2.2}%", efficiency));
}

fn heap_efficiency(allocator: &dyn GlobalAlloc, heap_size: usize) -> f64 {
    let mut v = Vec::with_capacity(100000);
    let mut used = 0;
    let mut total = 0;

    for _ in 0..300 {
        loop {
            let action = fastrand::usize(0..6);

            match action {
                0..=4 => {
                    let size = fastrand::usize(1..1 << 21);
                    let align =
                        std::mem::align_of::<usize>() << (fastrand::u16(..).trailing_zeros() / 2);

                    if let Some(allocation) = AllocationWrapper::new(size, align, allocator) {
                        v.push(allocation);
                    } else {
                        used += v.iter().map(|a| a.layout.size()).sum::<usize>();
                        total += heap_size;
                        v.clear();
                        break;
                    }
                }
                5 => {
                    if !v.is_empty() {
                        let index = fastrand::usize(0..v.len());
                        v.swap_remove(index);
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    used as f64 / total as f64 * 100.0
}

struct AllocationWrapper<'a> {
    ptr: *mut u8,
    layout: Layout,
    allocator: &'a dyn GlobalAlloc,
}
impl<'a> AllocationWrapper<'a> {
    fn new(size: usize, align: usize, allocator: &'a dyn GlobalAlloc) -> Option<Self> {
        let layout = Layout::from_size_align(size, align).unwrap();

        let ptr = unsafe { (*allocator).alloc(layout) };

        if ptr.is_null() {
            return None;
        }

        Some(Self {
            ptr,
            layout,
            allocator,
        })
    }
}

impl<'a> Drop for AllocationWrapper<'a> {
    fn drop(&mut self) {
        unsafe { (*self.allocator).dealloc(self.ptr, self.layout) }
    }
}
