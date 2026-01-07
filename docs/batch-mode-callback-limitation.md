# Batch Mode and Nested Callbacks - Resolved

## Summary

This document describes a limitation that existed with batch mode and deep callback nesting, and how it was resolved using coordinated ID allocation.

## The Original Problem

When batch mode was enabled, operations that return `JsValue` (or types wrapping it) used **placeholder IDs** instead of waiting for the actual result from JavaScript. This optimization allows multiple operations to be batched together and executed in a single round-trip.

The placeholder ID allocation worked as follows:
1. Rust pre-allocates the next sequential heap ID (e.g., 132, 133, 134...)
2. JavaScript was expected to allocate IDs in the same order
3. When the batch executed, the placeholder IDs should match the actual JS heap slots

### Where It Broke

This assumption broke when **callbacks were involved**. Consider this sequence:

```
1. Batch starts (is_batching = true)
2. Create obj1 -> placeholder ID = 132
3. Create cb1 (Closure) -> placeholder ID = 133
4. Call level1(&obj1, cb1) -> placeholder ID = 134 for result
5. JS starts executing the batch:
   - Creates obj1 at JS heap 132 OK
   - Creates cb1 at JS heap 133 OK
   - Calls level1(heap[132], heap[133])
6. Inside level1, JS calls cb1 (triggers Rust callback)
7. Rust callback executes (batching is disabled during callbacks)
8. Inside callback, Rust calls level2 -> This FLUSHES immediately
   - JS allocates level2's inner objects at heap 134, 135, ...
9. Callback returns, level1 continues
10. level1 finally returns its result object
    - JS allocates it at heap 140 (or later)
11. But Rust's placeholder for level1's result is still 134!
```

The placeholder ID (134) pointed to `level2`'s inner object instead of `level1`'s result.

## The Solution: Coordinated ID Allocation

The fix involves having Rust communicate the count of reserved placeholder IDs to JavaScript before batch execution. Since IDs are always sequential, JS can calculate the reserved range from its current `maxId` and skip those IDs during nested callback allocations.

### Implementation Details

**Rust Side (`batch.rs`):**
- Added `reserved_placeholder_count: u32` to `BatchState`
- Added `get_next_placeholder_id()` which tracks reserved slots for return values
- `take_message()` prepends the reserved count to the message

**Rust Side (`ipc.rs`):**
- Added `prepend_u32()` to `EncodedData` for inserting the reserved count

**JavaScript Side (`heap.ts`):**
- Added a reservation scope stack: `{ start, count, nextIndex }[]`
- `pushReservationScope(count)`: Advances `maxId` by count, creating reserved range
- `popReservationScope()`: Removes the current scope
- `fillNextReserved(value)`: Fills reserved slots sequentially for return values

**JavaScript Side (`ipc.ts`):**
- Reads `reserved_count` at the start of Evaluate messages
- Pushes/pops reservation scopes around batch processing
- Uses `fillNextReserved()` for HeapRef return values instead of `encode()`

### Message Format Change

```
Before: [msg_type: u8] [op1...] [op2...] ...
After:  [msg_type: u8] [reserved_count: u32] [op1...] [op2...] ...
```

The reserved count is only present in Rust-to-JS Evaluate messages. JS-to-Rust Evaluate messages (for callbacks) do not include it.

## Key Insight

When decoding JsValue parameters from JS callbacks, Rust must use `get_next_heap_id()` (NOT `get_next_placeholder_id()`) because these are incoming parameters, not return value placeholders. Only return values from JS functions should increment the placeholder count.

## Tests

The `test_borrowed_ref_deep_nesting` test now runs successfully in both batch and non-batch modes, validating that:
- 4 levels of nested callbacks work correctly
- Borrowed references remain valid after inner calls return
- Placeholder IDs correctly match their intended objects

All tests including `test_join_many_callbacks_async` (100 concurrent callbacks) pass in batch mode.
