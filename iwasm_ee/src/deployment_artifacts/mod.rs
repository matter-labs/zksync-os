//! Structure:
//!  - Index : IWasmDeploymentArtifactIndex
//!  - Data :
//!    - function_to_sidetable_compact_mapping
//!    - raw_sidetable_entries
//!    - immutables

pub mod fts_mapping;

use alloc::vec::Vec;
use core::alloc::Allocator;
use iwasm_interpreter::types::RawSideTableEntry;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct IWasmDeploymentArtifactIndex {
    pub function_to_sidetable_mapping_size: u16,
    // TODO: for this to work, we need to ensure that sidetable is populated in order of opcode
    // adderesses.
    pub function_to_sidetable_mapping_compact_size_cutoff: u16,
    pub function_to_sidetable_mapping_offset: u32,
    pub raw_sidetable_entries_count: u32,
    pub raw_sidetable_entries_offset: u32,
    pub immutables_size: u32,
    pub immutables_offset: u32,
}

#[repr(C)]
pub struct IWasmDeploymentArtifactAligned<'a> {
    // pub num_function_to_sidetable_mappings: u32,
    pub function_to_sidetable_compact_mapping: fts_mapping::FtsMapping<'a>,
    pub raw_sidetable_entries: &'a [RawSideTableEntry],
    pub immutables: &'a [u8],
}

impl<'a> IWasmDeploymentArtifactAligned<'a> {
    ///
    /// TODO: document safety
    ///
    /// # Safety
    ///
    pub unsafe fn from_slice(src: &'a [u8]) -> Self {
        assert!(src.as_ptr().addr() % 4 == 0);
        assert!(src.len() > 16);
        // so we can cast everything
        let base_ptr = src.as_ptr();
        let index = unsafe { &*(base_ptr.cast::<IWasmDeploymentArtifactIndex>()) };

        let function_to_sidetable_compact_mapping = unsafe {
            core::slice::from_raw_parts(
                base_ptr.add(index.function_to_sidetable_mapping_offset as usize),
                index.function_to_sidetable_mapping_size as usize,
            )
        };

        let function_to_sidetable_compact_mapping = fts_mapping::FtsMapping::new(
            function_to_sidetable_compact_mapping,
            index.function_to_sidetable_mapping_compact_size_cutoff,
        );

        let raw_sidetable_entries = unsafe {
            core::slice::from_raw_parts(
                base_ptr
                    .add(index.raw_sidetable_entries_offset as usize)
                    .cast::<RawSideTableEntry>(),
                index.raw_sidetable_entries_count as usize,
            )
        };

        let immutables = unsafe {
            core::slice::from_raw_parts(
                base_ptr.add(index.immutables_offset as usize),
                index.immutables_size as usize,
            )
        };

        Self {
            // num_function_to_sidetable_mappings: function_to_sidetable_compact_mapping.len() as u32,
            function_to_sidetable_compact_mapping,
            raw_sidetable_entries,
            immutables,
        }
    }
}

#[allow(dead_code)]
pub struct IWasmFunctionToSidetable<'a> {
    compact_size_cutoff: u16,
    data: &'a [u8],
}

pub struct IWasmDeploymentArtifact<A: Allocator> {
    pub function_to_sidetable_mapping: Vec<u32, A>,
    pub raw_sidetable_entries: Vec<RawSideTableEntry, A>,
    pub immutables: Vec<u8, A>,
}

impl<A: Allocator> IWasmDeploymentArtifact<A> {
    /// Calculates the length after serialization. *Note: doesn't include the reserved space for
    /// immutables.*
    pub fn serialization_len(&self) -> usize {
        use core::mem::size_of;

        // TODO: Add size_of_el() to `Iteratables` and size_in_memory() for vecs.
        let index = size_of::<IWasmDeploymentArtifactIndex>().next_multiple_of(4);
        let raw_sidetable_entries =
            (size_of::<RawSideTableEntry>() * self.raw_sidetable_entries.len()).next_multiple_of(4);

        let function_to_sidetable_mapping = {
            #[cfg(feature = "testing")]
            assert!(self.function_to_sidetable_mapping.is_sorted());

            // Looking for the boundary between two elements with different encode sizes.
            let compact_size_cutoff =
                match self.function_to_sidetable_mapping.binary_search_by(|x| {
                    match *x > u8::MAX as u32 {
                        true => core::cmp::Ordering::Greater,
                        _ => core::cmp::Ordering::Less,
                    }
                }) {
                    Err(x) => x,
                    _ => unreachable!(),
                };

            let (c8, c16) = {
                let total = self.function_to_sidetable_mapping.len();

                (compact_size_cutoff, total - compact_size_cutoff)
            };

            // In case of odd amount, the `u16` values coming after need padding.
            let c8_size = c8.next_multiple_of(2);
            let c16_size = c16 * size_of::<u16>();

            c8_size + c16_size
        }
        .next_multiple_of(4);

        index + raw_sidetable_entries + function_to_sidetable_mapping
    }

    /// Serializes the artifact into `dst`'s spare capacity.
    pub fn serialize_extend<AA: Allocator>(&self, dst: &mut Vec<u8, AA>) {
        use core::mem::size_of;

        let spare_capacity = dst.spare_capacity_mut();
        let expected_serialization_len = self.serialization_len();
        assert!(spare_capacity.len() >= expected_serialization_len);
        let ptr_range = spare_capacity.as_mut_ptr_range();
        assert!(ptr_range.start.addr() % 4 == 0);

        let base_ptr = ptr_range.start;

        let _end_ptr = ptr_range.end;

        let base_offset = (size_of::<IWasmDeploymentArtifactIndex>() as u32).next_multiple_of(4);

        assert!(self.function_to_sidetable_mapping.len() <= u16::MAX as usize);

        let raw_sidetable_entries_count = self.raw_sidetable_entries.len() as u32;
        let raw_sidetable_entries_offset = base_offset;

        let function_to_sidetable_mapping_offset = (raw_sidetable_entries_offset
            + (raw_sidetable_entries_count * size_of::<RawSideTableEntry>() as u32))
            .next_multiple_of(4);

        // Write raw_sidetable_entries.
        let tgt = unsafe { base_ptr.add(raw_sidetable_entries_offset as usize) }
            .cast::<RawSideTableEntry>();
        let src = self.raw_sidetable_entries.as_ptr();

        unsafe {
            core::ptr::copy_nonoverlapping(src, tgt, self.raw_sidetable_entries.len());
        }

        // Write function_to_sidetable_mapping.
        let (function_to_sidetable_mapping_size, function_to_sidetable_mapping_compact_size_cutoff) = {
            let start =
                unsafe { base_ptr.add(function_to_sidetable_mapping_offset as usize) }.cast::<u8>();

            let mut tgt = start;

            let mut iter = self
                .function_to_sidetable_mapping
                .iter()
                .enumerate()
                .peekable();
            if let Some(max_ix) = self.function_to_sidetable_mapping.last() {
                assert!(
                    *max_ix <= u16::MAX as u32,
                    "Some sidetable indices will not fit."
                );
            }

            // Write compacted values.
            let (have_outstanding_elems, compact_size_cutoff) = loop {
                if let Some((_, v)) = iter.next() {
                    unsafe {
                        #[cfg(feature = "testing")]
                        assert!(tgt.addr() < _end_ptr.addr());

                        tgt.write(*v as u8);
                        tgt = tgt.add(1);
                    }

                    if let Some((i, x)) = iter.peek() {
                        if **x > u8::MAX as u32 {
                            // The cutoff points to first item larger than `u8`.
                            break (true, *i as u16);
                        }
                    }
                } else {
                    // The cutoff never happened, so any value larger than the last index is fine.
                    break (false, u16::MAX);
                }
            };

            let mut tgt = tgt.cast::<u16>();

            // Align ptr. We can't do this unconditionally because the size of the table is
            // calculated based on this ptr.
            if have_outstanding_elems && tgt.is_aligned() == false {
                // Not using `align_offset` cause of the all the extra calculations we don't need
                // for our case: alignment is 2.
                tgt = unsafe { tgt.byte_add(1) };
            }

            // Write the less compacted values.
            for (_, v) in iter {
                unsafe {
                    #[cfg(feature = "testing")]
                    assert!(tgt.add(1).byte_sub(1).addr() < _end_ptr.addr());

                    let v = *v as u16;

                    tgt.write(v);
                    tgt = tgt.add(1);
                }
            }

            let size = unsafe { tgt.cast::<u8>().offset_from_unsigned(start) };
            assert!(
                size <= u16::MAX as usize,
                "Function to sidetable mapping is too long to be serialized."
            );

            (size as u16, compact_size_cutoff)
        };

        let immutables_offset = (function_to_sidetable_mapping_offset
            + function_to_sidetable_mapping_size as u32)
            .next_multiple_of(4);

        let index = IWasmDeploymentArtifactIndex {
            function_to_sidetable_mapping_size,
            function_to_sidetable_mapping_compact_size_cutoff,
            function_to_sidetable_mapping_offset,
            raw_sidetable_entries_count,
            raw_sidetable_entries_offset,
            immutables_size: 0, // We're going to set this later.
            immutables_offset,
        };

        // Write index.
        unsafe {
            base_ptr.cast::<IWasmDeploymentArtifactIndex>().write(index);
        }

        if expected_serialization_len != immutables_offset as usize {
            #[cfg(feature = "testing")]
            {
                println!(
                    "Error: Unexpected serialized length: expected: {}, got: {}",
                    expected_serialization_len, immutables_offset
                );

                println!(
                    "Sidetable mapping len {}",
                    self.function_to_sidetable_mapping.len()
                );
                println!(
                    "Sidetable mapping u8 indices count {}",
                    self.function_to_sidetable_mapping
                        .iter()
                        .filter(|x| **x <= u8::MAX as u32)
                        .count()
                );

                println!(
                    "Sidetable entries count {}",
                    self.raw_sidetable_entries.len()
                );
                println!("Sidetable entry size {}", size_of::<RawSideTableEntry>());

                println!("Index: {:#?}", index);
            }

            panic!("Unexpected serialized length");
        }

        let new_len = dst.len() + expected_serialization_len;
        unsafe {
            dst.set_len(new_len);
        }
    }

    ///
    /// # Safety
    /// `dst` must be a vector into which the bytecode and the artifacts are written.
    /// `index_offset` must point to `IWasmDeploymentArtifactIndex`.
    ///
    pub unsafe fn append_immutables(index_offset: usize, src: &[u8], dst: &mut Vec<u8, A>) -> u32 {
        assert!(dst.is_empty() == false);
        assert!(dst.spare_capacity_mut().len() >= src.len());

        // Safety: `dst` isn't empty.
        let base_ptr = unsafe { dst.as_mut_ptr().add(index_offset) };
        let index = unsafe { &mut *base_ptr.cast::<IWasmDeploymentArtifactIndex>() };

        index.immutables_size = src.len() as u32;

        // Safety: Copyint within capacity.
        unsafe {
            core::ptr::copy_nonoverlapping(
                src.as_ptr(),
                base_ptr.add(index.immutables_offset as usize),
                src.len(),
            )
        }

        unsafe { dst.set_len(dst.len() + index.immutables_size as usize) };

        (dst.len() - index_offset) as u32
    }
}

pub fn iwasm_full_preimage_len(bytecode_len: u32, scratch_space_len: u32) -> usize {
    let min_byte_length = bytecode_len.next_multiple_of(4) + scratch_space_len;

    min_byte_length as usize
}

#[cfg(test)]
mod artifact_serialization {
    use super::{IWasmDeploymentArtifact, IWasmDeploymentArtifactAligned};

    #[test]
    fn sidetable_mapping_even_packing() {
        let artifacts = IWasmDeploymentArtifact {
            function_to_sidetable_mapping: vec![1, 2, 3, 255, 256, 300, 301],
            raw_sidetable_entries: Vec::new(),
            immutables: Vec::new(),
        };

        let mut dst = Vec::with_capacity(1 << 10);

        artifacts.serialize_extend(&mut dst);

        let des = unsafe { IWasmDeploymentArtifactAligned::from_slice(dst.as_slice()) };

        let m = &des.function_to_sidetable_compact_mapping;

        assert_eq!(1, unsafe { m.get_unchecked(0) });
        assert_eq!(2, unsafe { m.get_unchecked(1) });
        assert_eq!(3, unsafe { m.get_unchecked(2) });
        assert_eq!(255, unsafe { m.get_unchecked(3) });
        assert_eq!(256, unsafe { m.get_unchecked(4) });
        assert_eq!(300, unsafe { m.get_unchecked(5) });
        assert_eq!(301, unsafe { m.get_unchecked(6) });
    }
}
