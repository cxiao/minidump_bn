use binaryninja::binaryview::{BinaryView, BinaryViewBase, BinaryViewExt};
use binaryninja::databuffer::DataBuffer;
use log::debug;
use minidump::{Minidump, MinidumpMemoryInfoList};
use std::ops::Deref;
use std::str;
use std::sync::Arc;

#[derive(Clone)]
struct DataBufferWrapper {
    inner: Arc<DataBuffer>,
}

impl DataBufferWrapper {
    fn new(buf: DataBuffer) -> Self {
        DataBufferWrapper {
            inner: Arc::new(buf),
        }
    }
}

impl Deref for DataBufferWrapper {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.inner.get_data()
    }
}

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
                    debug!("{}", str::from_utf8(&memory_info_list_writer).unwrap());
                }
            }
        }
    }
}
