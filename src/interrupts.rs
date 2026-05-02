use crate::gdt;
use crate::serial_println;
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use core::fmt;
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{
    InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode,
};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
pub const PIC_IRQ_COUNT: u8 = 16;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

pub fn timer_ticks() -> u64 {
    TIMER_TICKS.load(Ordering::Relaxed)
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard = PIC_1_OFFSET + 1,
    Pic2Start = PIC_2_OFFSET,
}

impl InterruptIndex {
    #[inline]
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
        idt.general_protection_fault
            .set_handler_fn(general_protection_fault_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.segment_not_present
            .set_handler_fn(segment_not_present_handler);
        idt.stack_segment_fault
            .set_handler_fn(stack_segment_fault_handler);
        idt.alignment_check.set_handler_fn(alignment_check_handler);

        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);

        // Minimal handlers for all remaining PIC IRQs to avoid lockups
        // from unexpected/unmasked interrupts. They just send an EOI.
        idt[(PIC_1_OFFSET + 2) as usize].set_handler_fn(irq2_handler);
        idt[(PIC_1_OFFSET + 3) as usize].set_handler_fn(irq3_handler);
        idt[(PIC_1_OFFSET + 4) as usize].set_handler_fn(irq4_handler);
        idt[(PIC_1_OFFSET + 5) as usize].set_handler_fn(irq5_handler);
        idt[(PIC_1_OFFSET + 6) as usize].set_handler_fn(irq6_handler);
        idt[(PIC_1_OFFSET + 7) as usize].set_handler_fn(irq7_handler);
        idt[(PIC_2_OFFSET + 0) as usize].set_handler_fn(irq8_handler);
        idt[(PIC_2_OFFSET + 1) as usize].set_handler_fn(irq9_handler);
        idt[(PIC_2_OFFSET + 2) as usize].set_handler_fn(irq10_handler);
        idt[(PIC_2_OFFSET + 3) as usize].set_handler_fn(irq11_handler);
        idt[(PIC_2_OFFSET + 4) as usize].set_handler_fn(irq12_handler);
        idt[(PIC_2_OFFSET + 5) as usize].set_handler_fn(irq13_handler);
        idt[(PIC_2_OFFSET + 6) as usize].set_handler_fn(irq14_handler);
        idt[(PIC_2_OFFSET + 7) as usize].set_handler_fn(irq15_handler);

        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

#[derive(Debug, Clone, Copy)]
pub struct StackFrameSnapshot {
    pub instruction_pointer: u64,
    pub code_segment: u64,
    pub cpu_flags: u64,
    pub stack_pointer: u64,
    pub stack_segment: u64,
}

impl From<&InterruptStackFrame> for StackFrameSnapshot {
    fn from(f: &InterruptStackFrame) -> Self {
        Self {
            instruction_pointer: f.instruction_pointer.as_u64(),
            code_segment: f.code_segment,
            cpu_flags: f.cpu_flags,
            stack_pointer: f.stack_pointer.as_u64(),
            stack_segment: f.stack_segment,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExceptionOrigin {
    Kernel,
    User,
}

impl ExceptionOrigin {
    fn as_str(self) -> &'static str {
        match self {
            ExceptionOrigin::Kernel => "kernel",
            ExceptionOrigin::User => "user",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExceptionErrorCode {
    Hex(u64),
    PageFault(PageFaultErrorCode),
}

pub fn write_exception_report<W: fmt::Write>(
    out: &mut W,
    name: &'static str,
    origin: ExceptionOrigin,
    error_code: Option<ExceptionErrorCode>,
    frame: &StackFrameSnapshot,
) {
    let _ = writeln!(out, "EXCEPTION: {name}");
    let _ = writeln!(out, "Origin: {}", origin.as_str());
    if let Some(code) = error_code {
        match code {
            ExceptionErrorCode::Hex(v) => {
                let _ = writeln!(out, "Error Code: {:#x}", v);
            }
            ExceptionErrorCode::PageFault(v) => {
                let _ = writeln!(out, "Error Code: {:?}", v);
            }
        }
    }
    let _ = writeln!(
        out,
        "Frame: rip={:#x} cs={:#x} rflags={:#x} rsp={:#x} ss={:#x}",
        frame.instruction_pointer,
        frame.code_segment,
        frame.cpu_flags,
        frame.stack_pointer,
        frame.stack_segment
    );
}

struct SerialWriter;

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        crate::serial::_print(format_args!("{s}"));
        Ok(())
    }
}

/// Mask all PIC IRQ lines except the timer by default.
///
/// Policy: keep the system stable (no unexpected IRQ lockups) until we have
/// real device drivers and handlers. Unmask more IRQs later as needed.
pub unsafe fn init_pic_masks() {
    // Mask bit = 1 disables the line.
    // PIC1 IRQ0(timer) unmasked, all other IRQs masked.
    let pic1_mask: u8 = 0b1111_1110;
    // Mask all PIC2 lines for now.
    let pic2_mask: u8 = 0b1111_1111;
    unsafe {
        PICS.lock().write_masks(pic1_mask, pic2_mask);
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    let mut out = SerialWriter;
    let snap = StackFrameSnapshot::from(&stack_frame);
    write_exception_report(
        &mut out,
        "BREAKPOINT",
        exception_origin(&stack_frame),
        None,
        &snap,
    );
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    let mut out = SerialWriter;
    let snap = StackFrameSnapshot::from(&stack_frame);
    write_exception_report(
        &mut out,
        "DOUBLE FAULT",
        exception_origin(&stack_frame),
        None,
        &snap,
    );
    halt_loop();
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

macro_rules! irq_eoi_handler {
    ($name:ident, $vector:expr) => {
        extern "x86-interrupt" fn $name(_stack_frame: InterruptStackFrame) {
            unsafe {
                PICS.lock().notify_end_of_interrupt($vector);
            }
        }
    };
}

irq_eoi_handler!(irq2_handler, PIC_1_OFFSET + 2);
irq_eoi_handler!(irq3_handler, PIC_1_OFFSET + 3);
irq_eoi_handler!(irq4_handler, PIC_1_OFFSET + 4);
irq_eoi_handler!(irq5_handler, PIC_1_OFFSET + 5);
irq_eoi_handler!(irq6_handler, PIC_1_OFFSET + 6);
irq_eoi_handler!(irq7_handler, PIC_1_OFFSET + 7);
irq_eoi_handler!(irq8_handler, PIC_2_OFFSET + 0);
irq_eoi_handler!(irq9_handler, PIC_2_OFFSET + 1);
irq_eoi_handler!(irq10_handler, PIC_2_OFFSET + 2);
irq_eoi_handler!(irq11_handler, PIC_2_OFFSET + 3);
irq_eoi_handler!(irq12_handler, PIC_2_OFFSET + 4);
irq_eoi_handler!(irq13_handler, PIC_2_OFFSET + 5);
irq_eoi_handler!(irq14_handler, PIC_2_OFFSET + 6);
irq_eoi_handler!(irq15_handler, PIC_2_OFFSET + 7);

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: InterruptStackFrame) {
    let mut out = SerialWriter;
    let snap = StackFrameSnapshot::from(&stack_frame);
    write_exception_report(
        &mut out,
        "INVALID OPCODE (#UD)",
        exception_origin(&stack_frame),
        None,
        &snap,
    );
    halt_loop();
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    let mut out = SerialWriter;
    let snap = StackFrameSnapshot::from(&stack_frame);
    write_exception_report(
        &mut out,
        "GENERAL PROTECTION FAULT (#GP)",
        exception_origin(&stack_frame),
        Some(ExceptionErrorCode::Hex(error_code)),
        &snap,
    );
    halt_loop();
}

extern "x86-interrupt" fn segment_not_present_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    let mut out = SerialWriter;
    let snap = StackFrameSnapshot::from(&stack_frame);
    write_exception_report(
        &mut out,
        "SEGMENT NOT PRESENT (#NP)",
        exception_origin(&stack_frame),
        Some(ExceptionErrorCode::Hex(error_code)),
        &snap,
    );
    halt_loop();
}

extern "x86-interrupt" fn stack_segment_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    let mut out = SerialWriter;
    let snap = StackFrameSnapshot::from(&stack_frame);
    write_exception_report(
        &mut out,
        "STACK SEGMENT FAULT (#SS)",
        exception_origin(&stack_frame),
        Some(ExceptionErrorCode::Hex(error_code)),
        &snap,
    );
    halt_loop();
}

extern "x86-interrupt" fn alignment_check_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    let mut out = SerialWriter;
    let snap = StackFrameSnapshot::from(&stack_frame);
    write_exception_report(
        &mut out,
        "ALIGNMENT CHECK (#AC)",
        exception_origin(&stack_frame),
        Some(ExceptionErrorCode::Hex(error_code)),
        &snap,
    );
    halt_loop();
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let accessed = Cr2::read();
    let mut out = SerialWriter;
    let snap = StackFrameSnapshot::from(&stack_frame);
    write_exception_report(
        &mut out,
        "PAGE FAULT (#PF)",
        exception_origin(&stack_frame),
        Some(ExceptionErrorCode::PageFault(error_code)),
        &snap,
    );
    serial_println!("CR2: {:?}", accessed);
    serial_println!(
        "PF_DECODE: protection_violation={} write={} user={} reserved_write={} instruction_fetch={}",
        error_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION),
        error_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE),
        error_code.contains(PageFaultErrorCode::USER_MODE),
        error_code.contains(PageFaultErrorCode::MALFORMED_TABLE),
        error_code.contains(PageFaultErrorCode::INSTRUCTION_FETCH),
    );

    // Policy: kernel currently has no user-mode recovery path, so treat all #PF as fatal.
    // Later: when user-mode exists, this is the branch point to deliver a signal/exception
    // or terminate the current task instead of panicking the kernel.
    panic!("fatal page fault");
}

fn exception_origin(stack_frame: &InterruptStackFrame) -> ExceptionOrigin {
    if (stack_frame.code_segment & 0b11) == 0b11 {
        ExceptionOrigin::User
    } else {
        ExceptionOrigin::Kernel
    }
}

fn halt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
