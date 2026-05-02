#![no_std]
#![no_main]

use core::panic::PanicInfo;
use gear_os::{exit_qemu, serial_print, serial_println, QemuExitCode};
use x86_64::instructions::port::Port;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    serial_print!("pic_masks_and_irqs...\t");
    gear_os::init();

    // Verify PIC masks: only timer (IRQ0) unmasked on PIC1, all masked on PIC2.
    let pic1_mask = unsafe { Port::<u8>::new(0x21).read() };
    let pic2_mask = unsafe { Port::<u8>::new(0xA1).read() };
    assert_eq!(pic1_mask, 0xFE, "PIC1 mask expected 0xFE, got {:#x}", pic1_mask);
    assert_eq!(pic2_mask, 0xFF, "PIC2 mask expected 0xFF, got {:#x}", pic2_mask);

    // Prove the IDT has callable handlers for PIC vectors (32..=47) that return.
    // We use software INT to invoke them; they must not hang/triple-fault.
    unsafe {
        core::arch::asm!("int 32");
        core::arch::asm!("int 33");
        core::arch::asm!("int 40");
        core::arch::asm!("int 47");
    }

    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    gear_os::test_panic_handler(info)
}

