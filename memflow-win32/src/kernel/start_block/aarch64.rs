use crate::kernel::StartBlock;

use std::convert::TryInto;

use memflow::architecture::arm::aarch64;
use memflow::error::{Error, ErrorKind, ErrorOrigin, Result};
use memflow::types::{size, umem, Address};

pub const PHYS_BASE: umem = size::gb(1);

// mem here has to be a single page (4kb sized)
fn find_pt(addr: Address, mem: &[u8]) -> Option<Address> {
    // TODO: global define / config setting
    let max_mem = size::gb(512) as u64;

    let pte = u64::from_le_bytes(mem[0..8].try_into().unwrap());

    if (pte & 0x0000_0000_0000_0fff) != 0xf03 || (pte & 0x0000_ffff_ffff_f000) > max_mem {
        return None;
    }

    // Second half must have a self ref entry
    // This is usually enough to filter wrong data out
    mem[0x800..]
        .chunks(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .find(|a| (a ^ 0xf03) & (!0u64 >> 12) == addr.to_umem())?;

    // A page table does need to have some entries, right? Particularly, kernel-side page table
    // entries must exist
    mem[0x800..]
        .chunks(8)
        .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
        .filter(|a| (a & 0xfff) == 0x703)
        .nth(5)?;

    Some(addr)
}

pub fn find(mem: &[u8]) -> Result<StartBlock> {
    mem.chunks_exact(aarch64::ARCH.page_size().try_into().unwrap())
        .enumerate()
        .filter_map(|(i, c)| {
            find_pt(
                Address::from(PHYS_BASE) + (i as umem * aarch64::ARCH.page_size()),
                c,
            )
        })
        .map(|addr| StartBlock {
            arch: aarch64::ARCH.ident(),
            kernel_hint: Address::NULL,
            dtb: addr,
        })
        .next()
        .ok_or_else(|| {
            Error(ErrorOrigin::OsLayer, ErrorKind::NotFound)
                .log_warn("unable to find aarch64 dtb in lowstub < 16M")
        })
}
