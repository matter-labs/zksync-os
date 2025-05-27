use super::System;
use super::*;
use crate::system::MAX_SCRATCH_SPACE_USIZE_WORDS;
use core::ops::Deref;
use core::ops::Range;

/// range for `T` with offset at.
pub fn range_for_at<T>(offset: usize) -> Result<Range<usize>, ()> {
    if offset % core::mem::align_of::<T>() != 0 {
        return Err(());
    }

    Ok(Range {
        start: offset,
        end: offset + core::mem::size_of::<T>(),
    })
}

pub fn get_pieces_of_slice<const N: usize, T>(
    slice: &mut [T],
    immutable: [Range<usize>; N],
    mutable: Range<usize>,
) -> Option<([&[T]; N], &mut [T])> {
    let (tmp, after) = slice.split_at_mut(mutable.end);
    let (before, mutable_slice) = tmp.split_at_mut(mutable.start);

    let mut immutable_slices: [&[T]; N] = [&[]; N];
    for (i, range) in immutable.into_iter().enumerate() {
        immutable_slices[i] = before.get(range.clone()).or_else(|| {
            after.get(Range {
                start: range.start.checked_sub(mutable.end)?,
                end: range.end.checked_sub(mutable.end)?,
            })
        })?;
    }

    Some((immutable_slices, mutable_slice))
}

///
/// Return values from a call.
///
pub struct ReturnValues<S: SystemTypes> {
    pub returndata:
        <<S::Memory as MemorySubsystem>::ManagedRegion as OSManagedRegion>::OSManagedImmutableSlice,
    pub return_scratch_space:
        Option<alloc::boxed::Box<[usize; MAX_SCRATCH_SPACE_USIZE_WORDS], OSAllocator<S>>>,
}

impl<S: SystemTypes> core::fmt::Debug for ReturnValues<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReturnValues")
            .field("returndata", &self.returndata.deref())
            .field("return_scratch_space", &self.return_scratch_space)
            .finish()
    }
}

impl<S: SystemTypes> ReturnValues<S> {
    pub fn empty(system: &mut System<S>) -> Self {
        Self {
            returndata: system.memory.empty_immutable_slice(),
            return_scratch_space: None,
        }
    }

    pub fn from_immutable_slice(region: OSImmutableSlice<S>) -> Self {
        Self {
            returndata: region,
            return_scratch_space: None,
        }
    }

    pub fn returndata(&self) -> Option<&OSImmutableSlice<S>> {
        Some(&self.returndata)
    }
}

///
/// Result after requesting to execute a call.
///
pub enum CallResult<S: SystemTypes> {
    /// Call preparations failed.
    CallFailedToExecute,
    /// Call failed after preparation.
    Failed { return_values: ReturnValues<S> },
    /// Call succeeded.
    Successful { return_values: ReturnValues<S> },
}

impl<S: SystemTypes> core::fmt::Debug for CallResult<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CallFailedToExecute => f.debug_struct("CallResult::CallFailedToExecute").finish(),
            Self::Failed { return_values } => f
                .debug_struct("CallResult::Failed")
                .field("return_values", return_values)
                .finish(),
            Self::Successful { return_values } => f
                .debug_struct("CallResult::Successful")
                .field("return_values", return_values)
                .finish(),
        }
    }
}

impl<S: SystemTypes> CallResult<S> {
    pub fn has_scratch_space(&self) -> bool {
        match self {
            CallResult::CallFailedToExecute => false,
            CallResult::Failed { return_values } | CallResult::Successful { return_values } => {
                return_values.return_scratch_space.is_some()
            }
        }
    }
}
