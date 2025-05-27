# Memory subsystem

The memory subsystem provides execution environments with heap buffers in an efficient manner. The heaps are valid for the lifetime of the execution environment, so return data needs to be moved to a dedicated memory area.

The current implementation is hard to use safely and is subject to change. It is likely that there won't be a memory subsystem in the future and splittable heaps are borrowed around instead.

## Execution environment facing interface

Execution environments (EEs) are normally only allowed to grow an existing heap and access heap contents.

## Additional methods

These are accessible to the bootloader and to EEs when they deploy a new contract.

- start a new heap / deallocate an old one
- copy some memory in a heap into the return data area
- make a static slice look like a slice of heap

The last one is an artifact of the current implementation; it doesn't return normal slices to heaps in order to circumvent lifetimes.

## Implementation

Because only the most recent execution environment can modify its heap, it is possible to store all the heaps in one array. The most recent heap is is stored last so it is free to grow into the remaining memory.

Returndata is copied to another array because after returning, a previous execution environment may grow its heap on top of the returndata.
