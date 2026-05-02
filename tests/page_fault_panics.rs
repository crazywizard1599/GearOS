#![no_std]
#![no_main]

use core::panic::PanicInfo;
use gear_os::{exit_qemu, serial_print, serial_println, QemuExitCode};
use x86_64::registers::control::Cr2;

const FAULT_ADDR: u64 = 0xdead_beaf_000;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    serial_print!("page_fault_panics...\t");
    gear_os::init();

    // Trigger a page fault by reading from an unmapped address.
    unsafe {
        let ptr = FAULT_ADDR as *const u64;
        core::ptr::read_volatile(ptr);
    }

    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    let cr2 = Cr2::read().as_u64();
    assert_eq!(
        cr2, FAULT_ADDR,
        "CR2 didn't match faulting address: CR2={:#x} expected={:#x}",
        cr2, FAULT_ADDR
    );
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

