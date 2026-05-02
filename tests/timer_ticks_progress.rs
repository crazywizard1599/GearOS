#![no_std]
#![no_main]

use core::panic::PanicInfo;
use gear_os::{exit_qemu, serial_print, serial_println, QemuExitCode, interrupts};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    serial_print!("timer_ticks_progress...\t");
    gear_os::init();

    let start = interrupts::timer_ticks();

    // Wait for a few timer IRQs. The PIT defaults to ~18.2 Hz under QEMU,
    // so this should complete quickly without relying on wall-clock time.
    let target = start + 2;
    let mut spins: u64 = 0;
    while interrupts::timer_ticks() < target && spins < 5_000_000 {
        x86_64::instructions::hlt();
        spins += 1;
    }

    let end = interrupts::timer_ticks();
    assert!(end >= target, "timer ticks did not advance: start={} end={}", start, end);

    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    gear_os::test_panic_handler(info)
}

