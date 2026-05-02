#![no_std]
#![no_main]

use core::panic::PanicInfo;
use gear_os::{QemuExitCode, exit_qemu, println, serial_print, serial_println};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    serial_print!("basic_boot::test_println...\t");
    gear_os::init();
    test_println();
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    gear_os::test_panic_handler(info)
}

fn test_println() {
    println!("test_println output");
}
