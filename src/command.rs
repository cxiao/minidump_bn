use std::str;

use log::{debug, info};
use minidump::{Minidump, MinidumpMemoryInfoList};

use binaryninja::binaryview::{BinaryView, BinaryViewBase, BinaryViewExt};

use crate::view::DataBufferWrapper;

pub fn print_memory_information(bv: &BinaryView) {
    debug!("Printing memory information");
    if let Ok(minidump_bv) = bv.parent_view() {
        if let Ok(read_buffer) = minidump_bv.read_buffer(0, minidump_bv.len()) {
            let read_buffer = DataBufferWrapper::new(read_buffer);
            if let Ok(minidump_obj) = Minidump::read(read_buffer) {
                if let Ok(memory_info_list) = minidump_obj.get_stream::<MinidumpMemoryInfoList>() {
                    let mut memory_info_list_writer = Vec::new();
                    memory_info_list
                        .print(&mut memory_info_list_writer)
                        .unwrap();
                    info!("{}", str::from_utf8(&memory_info_list_writer).unwrap());
                }
            }
        }
    }
}
