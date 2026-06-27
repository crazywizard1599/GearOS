---
name: interrupt-handler
description: Use when adding or editing interrupt and exception handlers, IDT entries, or IRQ handlers in src/interrupts.rs.
---

## Adding an interrupt/exception handler

1. Define the handler function using `extern "x86-interrupt" fn name(...)`.
2. **Know whether the handler takes an error code** — check Intel manual or a reference:
   - With error code: `#DF` (0), `#TS` (10), `#NP` (11), `#SS` (12), `#GP` (13), `#PF` (14), `#AC` (17)
   - Without error code: all others (`#DE`, `#DB`, `NMI`, `#BP`, `#OF`, `#BR`, `#UD`, `#NM`, `#MF`, `#MC`, `#XM`, `#VE`, `#CP`)
3. If it's an exception that should halt: call `halt_loop()` at the end (defined in `interrupts.rs`).
4. If it's an IRQ: send EOI via `PICS.lock().notify_end_of_interrupt(vector)`.
5. Register the handler in the `IDT` lazy_static in `src/interrupts.rs`.
6. If a handler uses the IST (e.g. double fault), add `.set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX)`.

## Handler signature reference

```rust
// No error code
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) { ... }

// With error code
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64,
) -> ! { ... }

// Page fault (special error code type)
extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame, error_code: PageFaultErrorCode,
) { ... }

// IRQ handler (e.g. timer)
extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8()); }
}
```

## IRQ handler shortcut

For a minimal EOI-only handler (masked IRQs that shouldn't fire, but might):
```rust
extern "x86-interrupt" fn irqN_handler(_stack_frame: InterruptStackFrame) {
    unsafe { PICS.lock().notify_end_of_interrupt(PIC_1_OFFSET + N); }
}
```
Or use the `irq_eoi_handler!` macro already in `interrupts.rs`.

## Exception report

For exceptions that should log before halting, use the existing pattern:
```rust
let mut out = SerialWriter;
let snap = StackFrameSnapshot::from(&stack_frame);
write_exception_report(&mut out, "NAME (#XX)", exception_origin(&stack_frame), error_code, &snap);
halt_loop();
```
