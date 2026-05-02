#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::fmt::{self, Write};
use gear_os::{exit_qemu, serial_print, serial_println, QemuExitCode, interrupts};

struct FixedBuf {
    buf: [u8; 512],
    len: usize,
}

impl FixedBuf {
    const fn new() -> Self {
        Self { buf: [0; 512], len: 0 }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

impl Write for FixedBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.buf.len().saturating_sub(self.len);
        let to_copy = core::cmp::min(remaining, bytes.len());
        self.buf[self.len..self.len + to_copy].copy_from_slice(&bytes[..to_copy]);
        self.len += to_copy;
        Ok(())
    }
}

fn idx(hay: &str, needle: &str) -> usize {
    hay.find(needle).unwrap_or(usize::MAX)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    serial_print!("exception_report_format...\t");
    gear_os::init();

    let mut out = FixedBuf::new();
    let frame = interrupts::StackFrameSnapshot {
        instruction_pointer: 0x1111,
        code_segment: 0x8,
        cpu_flags: 0x202,
        stack_pointer: 0x2222,
        stack_segment: 0x0,
    };

    interrupts::write_exception_report(
        &mut out,
        "GENERAL PROTECTION FAULT (#GP)",
        interrupts::ExceptionOrigin::Kernel,
        Some(interrupts::ExceptionErrorCode::Hex(0xdead)),
        &frame,
    );

    let s = out.as_str();
    // Enforce predictable ordering.
    let a = idx(s, "EXCEPTION:");
    let b = idx(s, "Origin:");
    let c = idx(s, "Error Code:");
    let d = idx(s, "Frame:");
    assert!(a < b && b < c && c < d, "unexpected report order:\n{}", s);
    assert!(s.contains("EXCEPTION: GENERAL PROTECTION FAULT (#GP)"));
    assert!(s.contains("Origin: kernel"));
    assert!(s.contains("Error Code: 0xdead"));

    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    gear_os::test_panic_handler(info)
}

