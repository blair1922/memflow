use crate::util::*;
use memflow_core::process::*;
use std::slice::from_raw_parts_mut;

use memflow_core::architecture::ArchitectureObj;
use memflow_core::types::Address;

pub type OsProcessInfoObj = &'static dyn OsProcessInfo;

#[no_mangle]
pub extern "C" fn os_process_info_address(obj: &OsProcessInfoObj) -> Address {
    obj.address()
}

#[no_mangle]
pub extern "C" fn os_process_info_pid(obj: &OsProcessInfoObj) -> PID {
    obj.pid()
}

/// Retreive name of the process
///
/// This will copy at most `max_len` characters (including the null terminator) into `out` of the
/// name.
///
/// # Safety
///
/// `out` must be a buffer with at least `max_len` size
#[no_mangle]
pub unsafe extern "C" fn os_process_info_name(
    obj: &OsProcessInfoObj,
    out: *mut u8,
    max_len: usize,
) -> usize {
    let name = obj.name();
    let name_bytes = name.as_bytes();
    let out_bytes = from_raw_parts_mut(out, std::cmp::min(max_len, name.len()));
    let len = out_bytes.len();
    out_bytes[..(len - 1)].copy_from_slice(&name_bytes[..(len - 1)]);
    *out_bytes.iter_mut().last().unwrap() = 0;
    len
}

#[no_mangle]
pub extern "C" fn os_process_info_sys_arch(obj: &OsProcessInfoObj) -> &ArchitectureObj {
    to_heap(obj.sys_arch())
}

#[no_mangle]
pub extern "C" fn os_process_info_proc_arch(obj: &OsProcessInfoObj) -> &ArchitectureObj {
    to_heap(obj.proc_arch())
}

/// Free a OsProcessInfoObj reference
///
/// # Safety
///
/// `obj` must point to a valid `OsProcessInfoObj`, and was created using one of the API's
/// functions.
#[no_mangle]
pub unsafe extern "C" fn free_os_process_info(obj: &'static mut OsProcessInfoObj) {
    let _ = Box::from_raw(obj);
}