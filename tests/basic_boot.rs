#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(GearOS::test_runner)]
#![reexport_test_harness_main = "test_main"]


use core::panic::PanicInfo;
use GearOS::println;

#[test_case]
fn test_println() {
    println!("test_println output");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    GearOS::test_panic_handler(info)
}