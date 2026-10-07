#![no_std]
#![allow(static_mut_refs)]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn compute_heavy_loop() -> u64 {
    let mut acc: u64 = 0;
    let mut i: u64 = 0;
    while i < 1000 {
        acc = acc.wrapping_add(i.wrapping_mul(3));
        i += 1;
    }
    acc
}

#[no_mangle]
pub extern "C" fn memory_heavy_loop() -> u64 {
    let mut buf = [0u64; 64];
    let mut i = 0usize;
    while i < 64 {
        buf[i] = (i as u64).wrapping_mul(7);
        i += 1;
    }
    let mut sum = 0u64;
    let mut j = 0usize;
    while j < 64 {
        sum = sum.wrapping_add(buf[j]);
        j += 1;
    }
    sum
}

#[no_mangle]
pub extern "C" fn caller_of_heavy() -> u64 {
    compute_heavy_loop().wrapping_add(memory_heavy_loop())
}
