---
name: kernel-test
description: Use when adding or modifying integration test files under tests/. Covers the #![no_std] test skeleton, QEMU exit-code signaling, and the should_panic pattern.
---

## Adding a new integration test

1. Create `tests/<name>.rs` and register it in `Cargo.toml`:
   ```toml
   [[test]]
   name = "<name>"
   harness = false
   ```
2. Use the `#![no_std]` / `#![no_main]` skeleton. Tests are standalone kernel binaries, not Rust test harnesses.

## Test skeleton

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;
use gear_os::{QemuExitCode, exit_qemu, serial_print, serial_println};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    gear_os::init();
    // your test logic here
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    gear_os::test_panic_handler(info)
}
```

## QEMU exit codes

```rust
QemuExitCode::Success  // = 0x10 -> QEMU exits with code 33 (pass)
QemuExitCode::Failed   // = 0x11 -> QEMU exits with code 34 (fail)
```

- On success: call `exit_qemu(QemuExitCode::Success); loop {}`
- On failure: call `exit_qemu(QemuExitCode::Failed); loop {}`
- The panic handler normally calls `test_panic_handler` which exits with `Failed`.

## should_panic tests

Test passes by panicking -- the `_start` function should reach an `assert_eq!` or
similar that triggers a panic. The panic handler exits with `Success`:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    should_fail();  // will panic
    // If we reach here, the test FAILED (expected panic didn't happen)
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

fn should_fail() {
    assert_eq!(0, 1);  // triggers panic
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
```

## Output

- Use `serial_print!` / `serial_println!` for test output (visible on CLI).
- `println!` / `print!` goes to VGA (visible in QEMU window, not CLI).
- Each test should print its purpose + `[ok]` or `[failed]` on a single line.
