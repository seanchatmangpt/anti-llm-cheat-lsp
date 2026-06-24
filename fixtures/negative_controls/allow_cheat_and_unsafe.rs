#[allow(dead_code)]
pub struct UnusedStruct {
    data: Vec<u8>,
}

#[allow(unused_variables)]
pub fn process(x: i32) -> i32 {
    let result = x * 2;
    42
}

pub fn raw_ptr_access(ptr: *const u8) -> u8 {
    unsafe { *ptr }
}

pub unsafe fn raw_access() {}
