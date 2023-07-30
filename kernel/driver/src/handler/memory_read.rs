use alloc::vec::Vec;
use valthrun_driver_shared::{requests::{RequestRead, ResponseRead}, IO_MAX_DEREF_COUNT};
use winapi::km::wdm::PEPROCESS;

use crate::{kdef::{PsLookupProcessByProcessId, ProbeForRead}, kapi::{attach_process_stack, self, NTStatusEx}};

pub fn handler_read(req: &RequestRead, res: &mut ResponseRead) -> anyhow::Result<()> {
    let mut process: PEPROCESS = core::ptr::null_mut();
    if let Err(_status) = unsafe { PsLookupProcessByProcessId(req.process_id, &mut process) }.ok() {
        *res = ResponseRead::UnknownProcess;
        return Ok(());
    }
    
    if req.offset_count > IO_MAX_DEREF_COUNT || req.offset_count > req.offsets.len() {
        anyhow::bail!("offset count is not valid")
    }
    
    let mut read_buffer = Vec::with_capacity(req.count);
    read_buffer.resize(req.count, 0u8);

    let local_offsets = Vec::from(&req.offsets[0..req.offset_count]);
    let mut target_address = unsafe { core::mem::transmute::<_, *const u8>(local_offsets[0]) };
    let mut resolved_offsets = [0u64; IO_MAX_DEREF_COUNT];
    let mut offset_index = 1usize;

    let attach_guard = attach_process_stack(process);
    let read_result = kapi::try_seh(|| {
        while offset_index < local_offsets.len() {
            let deref_address = unsafe {
                ProbeForRead(target_address as *const (), 8, 1);

                target_address
                    .cast::<*const u8>() // Target address is trated as ptr
                    .read() // dereference ptr
            };
    
            resolved_offsets[offset_index - 1] = deref_address as u64;
            target_address = deref_address.wrapping_offset(local_offsets[offset_index] as isize); // add the next offset
            offset_index += 1;
        }

        let read_source = unsafe {
            ProbeForRead(target_address as *const (), read_buffer.len(), 1);
            core::slice::from_raw_parts(target_address, read_buffer.len())
        };
        read_buffer.copy_from_slice(read_source);
    });

    drop(attach_guard);
    if !read_result.is_ok() {
        *res = ResponseRead::InvalidAddress { resolved_offsets, resolved_offset_count: offset_index - 1  };
        return Ok(());
    }

    /* Copy result to output */
    let out_buffer = unsafe {
        core::slice::from_raw_parts_mut(req.buffer, req.count)
    };
    out_buffer.copy_from_slice(read_buffer.as_slice());
    *res = ResponseRead::Success;
    Ok(())
}