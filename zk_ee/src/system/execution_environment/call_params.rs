use super::*;
use crate::system::MAX_SCRATCH_SPACE_USIZE_WORDS;
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

/// Return values from a call.
pub struct ReturnValues<'a, S: SystemTypes> {
    pub returndata: &'a [u8],
    pub return_scratch_space:
        Option<alloc::boxed::Box<[usize; MAX_SCRATCH_SPACE_USIZE_WORDS], S::Allocator>>,
}

impl<S: SystemTypes> core::fmt::Debug for ReturnValues<'_, S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReturnValues")
            .field("returndata", &self.returndata)
            .field("return_scratch_space", &self.return_scratch_space)
            .finish()
    }
}

impl<S: SystemTypes> ReturnValues<'_, S> {
    pub fn empty() -> Self {
        Self {
            returndata: &[],
            return_scratch_space: None,
        }
    }
}

///
/// Result after requesting to execute a call.
///
pub enum CallResult<'a, S: SystemTypes> {
    /// Call preparations failed.
    CallFailedToExecute,
    /// Call failed after preparation.
    Failed { return_values: ReturnValues<'a, S> },
    /// Call succeeded.
    Successful { return_values: ReturnValues<'a, S> },
}

impl<S: SystemTypes> core::fmt::Debug for CallResult<'_, S> {
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

impl<S: SystemTypes> CallResult<'_, S> {
    pub fn has_scratch_space(&self) -> bool {
        match self {
            CallResult::CallFailedToExecute => false,
            CallResult::Failed { return_values } | CallResult::Successful { return_values } => {
                return_values.return_scratch_space.is_some()
            }
        }
    }
}
