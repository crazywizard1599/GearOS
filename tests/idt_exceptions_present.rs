#![no_std]
#![no_main]

use core::panic::PanicInfo;
use gear_os::{exit_qemu, serial_println, QemuExitCode};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    gear_os::init();
    idt_has_minimum_exception_handlers();
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[repr(C, packed)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    options: u16,
    offset_mid: u16,
    offset_high: u32,
    reserved: u32,
}

impl IdtEntry {
    fn handler_addr(&self) -> u64 {
        (u64::from(self.offset_low))
            | (u64::from(self.offset_mid) << 16)
            | (u64::from(self.offset_high) << 32)
    }
}

fn idt_entry(vector: u8) -> &'static IdtEntry {
    use x86_64::structures::DescriptorTablePointer;

    let idtr: DescriptorTablePointer = x86_64::instructions::tables::sidt();

    let base = idtr.base.as_u64() as *const IdtEntry;
    unsafe { 
        &*base.add(vector as usize) 
    }
}

fn assert_vector_has_handler(vector: u8, name: &str) {
    let addr = idt_entry(vector).handler_addr();
    gear_os::serial_print!("idt vector {} {}...\t", vector, name);
    assert_ne!(addr, 0, "IDT vector {} ({}) has null handler", vector, name);
    gear_os::serial_println!("[ok] @ {:#x}", addr);
}

fn idt_has_minimum_exception_handlers() {
    // Minimum required by the task:
    // #UD=6, #NP=11, #SS=12, #GP=13, #PF=14, #AC=17
    assert_vector_has_handler(6, "#UD");
    assert_vector_has_handler(11, "#NP");
    assert_vector_has_handler(12, "#SS");
    assert_vector_has_handler(13, "#GP");
    assert_vector_has_handler(14, "#PF");
    assert_vector_has_handler(17, "#AC");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    gear_os::test_panic_handler(info)
}

