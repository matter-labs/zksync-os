#[cfg(feature = "allocator_api")]
mod with_allocator;

#[cfg(not(feature = "allocator_api"))]
mod global_alloc;
