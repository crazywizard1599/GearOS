# GearOS — Agent guide

This is a `#![no_std]` x86_64 kernel (blog_os-style). It runs on **nightly Rust only** and boots in QEMU via `bootimage`.

## Build & run

```powershell
cargo run                       # builds bootloader + kernel, runs in QEMU
cargo test                      # runs all integration tests in QEMU
cargo test <name>               # single test (e.g. `cargo test basic_boot`)
cargo check                     # checks kernel crate (no bootimage)
```

- Builds use `build-std` to compile `core` + `compiler_builtins` from source.
- `cargo run` and `cargo test` go through `bootimage runner` → QEMU.
- Check-only without QEMU: `cargo check` (no bootloader build).
- `RUSTFLAGS` / extra cargo flags are rarely needed; config is in `.cargo/config.toml`.

## Nightly requirements

- `rust-toolchain.toml` pins `nightly` with `rust-src` + `llvm-tools-preview`.
- `rustup component add llvm-tools-preview` is required (not auto-installed by cargo).
- Uses `#![feature(custom_test_frameworks)]`, `#![feature(abi_x86_interrupt)]`.

## Custom target

- Target spec: `x86_64-GearOS.json` (custom, not built-in).
- Requires `json-target-spec = true` in `.cargo/config.toml` `[unstable]` (nightly 1.98+).
- **Global config** `~\.cargo\config.toml` must also have `[unstable] json-target-spec = true` for cargo invocations outside the project dir (rust-analyzer sysroot metadata).

## Testing quirks

- All tests are `harness = false` integration tests under `tests/`.
- Each test is a standalone kernel binary that boots in QEMU.
- Success/failure is communicated via QEMU `isa-debug-exit` device at I/O port `0xf4`.
  - Exit code `0x10` = pass, `0x11` = fail.
- Tests use `exit_qemu(QemuExitCode::*)` to signal completion.
- `should_panic` tests pass by panicking and returning `0x10` from the panic handler.
- Tests output via `serial_print!`/`serial_println!` (serial port, visible in CLI).
- `cargo test -- --nocapture` style flags don't apply; output is always visible.

## Output channels

- **VGA buffer** at `0xb8000`: `print!`/`println!` (yellow on black).
- **Serial port** COM1 (`0x3F8`): `serial_print!`/`serial_println!` (used for test output).
- Both use `try_lock()` + `without_interrupts` to avoid deadlocks in exception/IRQ context.

## PIC / interrupts

- Timer IRQ (PIC1 offset 0) is unmasked; all other 15 IRQs are masked.
- Every IRQ has a minimal EOI-only handler to prevent lockups from spurious interrupts.
- `init()` calls `gdt::init()`, `interrupts::init_idt()`, PIC init/masking, then enables interrupts.

## Known pitfalls

| Issue | Cause | Fix |
|---|---|---|
| `bootloader` build fails | bootloader 0.9.31's `x86_64-bootloader.json` uses `"64"` / `"32"` strings for numeric fields | Edit file in registry: `~\.cargo\registry\src\...\bootloader-0.9.31\x86_64-bootloader.json` — change `target-pointer-width` and `target-c-int-width` from strings to numbers |
| `llvm-tools not found` | `llvm-tools-preview` rustup component missing | `rustup component add llvm-tools-preview` |
| `Unknown binary 'rust-analyzer.exe'` | rust-analyzer not in the nightly toolchain | `rustup component add rust-analyzer --toolchain nightly-x86_64-pc-windows-msvc` |
| rust-analyzer fails on sysroot metadata | `json-target-spec` not set globally | Add `[unstable] json-target-spec = true` to `~\.cargo\config.toml` |

## rust-analyzer

Required settings (in `.vscode/settings.json`):
```json
{
    "rust-analyzer.cargo.target": "x86_64-GearOS.json",
    "rust-analyzer.cargo.buildStd": ["core", "compiler_builtins"],
    "rust-analyzer.cargo.extraEnv": {
        "CARGO_UNSTABLE_JSON_TARGET_SPEC": "true"
    }
}
```

## Entrypoints

| File | Role |
|---|---|
| `src/main.rs` | Kernel entry (`_start`), calls `gear_os::init()` |
| `src/lib.rs` | Library root, `init()`, test framework, QEMU exit |
| `src/gdt.rs` | GDT + TSS setup (double-fault IST) |
| `src/interrupts.rs` | IDT, exception/IRQ handlers, exception report formatting |
| `src/vga_buffer.rs` | VGA text-mode writer (`println!`) |
| `src/serial.rs` | Serial port writer (`serial_println!`) |

## Ponytail, lazy senior dev mode (Only use for tasks involving writing or modifying code)
You are a lazy senior developer. Lazy means efficient, not careless. The best code is the code never written.

Before writing any code, stop at the first rung that holds:

- Does this need to be built at all? (YAGNI)
- Does it already exist in this codebase? Reuse the helper, util, or pattern that's already here, don't re-write it.
- Does the standard library already do this? Use it.
- Does a native platform feature cover it? Use it.
- Does an already-installed dependency solve it? Use it.
- Can this be one line? Make it one line.
- Only then: write the minimum code that works.

The ladder runs after you understand the problem, not instead of it: read the task and the code it touches, trace the real flow end to end, then climb.

Bug fix = root cause, not symptom: a report names a symptom. Grep every caller of the function you touch and fix the shared function once — one guard there is a smaller diff than one per caller, and patching only the path the ticket names leaves a sibling caller still broken.

Rules:

- No abstractions that weren't explicitly requested.
- No new dependency if it can be avoided.
- No boilerplate nobody asked for.
- Deletion over addition. Boring over clever. Fewest files possible.
- Shortest working diff wins, but only once you understand the problem. The smallest change in the wrong place isn't lazy, it's a second bug.
- Question complex requests: "Do you actually need X, or does Y cover it?"
- Pick the edge-case-correct option when two stdlib approaches are the same size, lazy means less code, not the flimsier algorithm.
- Mark intentional simplifications with a ponytail: comment. If the shortcut has a known ceiling (global lock, O(n²) scan, naive heuristic), the comment names the ceiling and the upgrade path.

Not lazy about: understanding the problem (read it fully and trace the real flow before picking a rung, a small diff you don't understand is just laziness dressed up as efficiency), input validation at trust boundaries, error handling that prevents data loss, security, accessibility, the calibration real hardware needs (the platform is never the spec ideal, a clock drifts, a sensor reads off), anything explicitly requested. Lazy code without its check is unfinished: non-trivial logic leaves ONE runnable check behind, the smallest thing that fails if the logic breaks (an assert-based demo/self-check or one small test file; no frameworks, no fixtures). Trivial one-liners need no test.