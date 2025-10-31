# Stability & Security Improvements TODO for chonk-note

Based on the stability fixes applied to chonk-stract, here's what needs to be done for chonk-note.

## 📋 Priority List

### 🔴 Critical (High Priority)

#### 1. **Terminal State Recovery**
**Issue**: If chonk-note panics, terminal may be left in raw mode with hidden cursor.

**Fix Needed**:
- Add panic hook to restore terminal state (same as chonk-stract)
- Ensure raw mode is always disabled on exit
- Test with forced panic (Ctrl+C, kill signal)

**Files to Modify**: `main.rs`

---

#### 2. **Buffer Size Limits**
**Issue**: `chunked_grid.rs` may allow unbounded growth in text buffers.

**Current State**:
- 9 buffer operations found in chunked_grid.rs
- Need to audit for unbounded allocation

**Fix Needed**:
- Add maximum buffer dimensions (e.g., 100,000 rows × 10,000 columns)
- Add maximum paste size limit (e.g., 1MB)
- Validate before allocation

**Files to Modify**: `chunked_grid.rs`, `keyboard.rs` (paste operations)

**Safety Limits to Add**:
```rust
const MAX_BUFFER_ROWS: usize = 100_000;
const MAX_BUFFER_COLS: usize = 10_000;
const MAX_PASTE_SIZE: usize = 1_000_000; // 1MB
```

---

#### 3. **Temp File Management**
**Issue**: Screenshot and PDF rendering may create temp files.

**Current State**:
- 15 file operations found
- Need to audit for proper cleanup

**Fix Needed**:
- Add `tempfile` crate to Cargo.toml
- Replace manual temp file management with `NamedTempFile`
- Ensure automatic cleanup via RAII

**Files to Modify**: Any files using `File::create`, `std::fs::write`, etc.

---

### 🟡 High Priority

#### 4. **Error Message Improvements**
**Fix Needed**:
- Use `anyhow::Context` throughout for better error messages
- Add context to database operations
- Add context to file I/O operations
- Add context to graphics operations

**Files to Modify**: `notes_database.rs`, `main.rs`, `kitty_native.rs`

---

#### 5. **Mutex Poisoning Handling**
**Issue**: Two instances of `.lock().unwrap()` on mutexes in `kitty_native.rs`.

**Current Code**:
```rust
// kitty_native.rs:235
let mut buffer_guard = INPUT_BUFFER.lock().unwrap();

// kitty_native.rs:280
let mut buffer_guard = INPUT_BUFFER.lock().unwrap();
```

**Fix Needed**:
- Handle potential mutex poisoning
- Use `.lock().unwrap_or_else()` or proper error handling
- Document panic behavior

**Files to Modify**: `kitty_native.rs`

---

### 🟢 Medium Priority

#### 6. **Input Validation**
**Fix Needed**:
- Validate viewport dimensions
- Validate scroll positions
- Validate mouse coordinates
- Add bounds checking before array access

**Files to Modify**: `viewport.rs`, `mouse.rs`, `chunked_grid.rs`

---

#### 7. **Database Connection Resilience**
**Fix Needed**:
- Add retry logic for database operations
- Handle connection failures gracefully
- Validate database schema on startup
- Add automatic backup before risky operations

**Files to Modify**: `notes_database.rs`

---

#### 8. **Undo Stack Limits**
**Issue**: Undo system in `undo.rs` may grow unboundedly.

**Fix Needed**:
- Add maximum undo history size (e.g., 1000 operations)
- Add memory-based limit (e.g., 100MB)
- Automatically trim old history

**Files to Modify**: `undo.rs`

---

## 🔍 Audit Checklist

### Code Quality
- [ ] Search for all `.unwrap()` and `.expect()` calls
- [ ] Add bounds checking before array/vec indexing
- [ ] Validate all user input (mouse, keyboard, clipboard)
- [ ] Check for integer overflow in calculations
- [ ] Review all file I/O for proper error handling

### Resource Management
- [ ] Audit temp file creation and cleanup
- [ ] Check for memory leaks in long-running operations
- [ ] Review buffer allocations for size limits
- [ ] Check database connection pooling/cleanup

### Error Handling
- [ ] Add context to all error propagation
- [ ] Ensure no silent error suppression (`let _ = ...`)
- [ ] Test error paths (database failure, file I/O errors)
- [ ] Add user-friendly error messages

---

## 🎯 Implementation Plan

### Phase 1: Critical Fixes (This Week)
1. ✅ Add todo list items
2. ⏳ Terminal panic guard
3. ⏳ Buffer size limits
4. ⏳ Temp file management

### Phase 2: High Priority (Next Week)
5. ⏳ Error message improvements
6. ⏳ Mutex poisoning handling
7. ⏳ Input validation

### Phase 3: Medium Priority (Future)
8. ⏳ Database resilience
9. ⏳ Undo stack limits
10. ⏳ Comprehensive audit

---

## 📊 Success Metrics

After fixes are applied, chonk-note should:
- ✅ Never leave terminal in broken state (even on panic)
- ✅ Reject inputs that would cause memory exhaustion
- ✅ Clean up all temp files automatically
- ✅ Provide clear error messages for all failures
- ✅ Handle database errors gracefully
- ✅ Validate all user input before processing

---

## 🔗 Related Documentation

See `/Users/jack/chonk-stract/STABILITY.md` for reference implementation of these fixes.

---

## 📝 Notes

**Key Differences from chonk-stract**:
- chonk-note has database operations (new concern)
- chonk-note has more complex buffer management (chunked grid)
- chonk-note has undo system (memory concern)
- chonk-note has native Kitty protocol (graphics state)

**Estimated Effort**: 2-3 days for critical + high priority fixes
