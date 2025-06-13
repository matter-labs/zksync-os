use crate::talc::TalcWrapper;
use alloc::vec;
use core::alloc::GlobalAlloc;
use core::alloc::Layout;
use ruint::aliases::B160;
use talc::ClaimOnOom;
use talc::ErrOnOom;
use talc::Span;
use talc::Talc;
use zk_ee::utils::Bytes32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct WarmStorageKey {
    pub address: B160,
    pub key: Bytes32,
}

impl PartialOrd for WarmStorageKey {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WarmStorageKey {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        match self.address.as_limbs().cmp(&other.address.as_limbs()) {
            core::cmp::Ordering::Equal => self.key.cmp(&other.key),
            a => a,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct WarmStorageValue {
    pub initial_value: Bytes32,
    pub current_value: Bytes32,
    pub changes_stack_depth: usize,
    pub pubdata_diff_bytes_at_first_access: u8,
    pub pubdata_diff_bytes: u8,
    pub explicit_read_initial: bool,
    pub is_new_storage_slot: bool,
}

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
                        std::mem::align_of::<usize>() << fastrand::u16(..).trailing_zeros() / 2;

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
