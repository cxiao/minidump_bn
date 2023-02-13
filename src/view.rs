use std::ops::{Deref, Range};
use std::sync::Arc;

use binaryninja::segment::Segment;
use log::{debug, error};
use minidump::{
    Minidump, MinidumpMemory64List, MinidumpMemoryList, MinidumpStream, MinidumpSystemInfo,
};

use binaryninja::binaryview::{BinaryView, BinaryViewBase, BinaryViewExt};
use binaryninja::custombinaryview::{
    BinaryViewType, BinaryViewTypeBase, CustomBinaryView, CustomBinaryViewType, CustomView,
};
use binaryninja::databuffer::DataBuffer;
use binaryninja::platform::Platform;
use binaryninja::Endianness;

#[derive(Clone)]
pub struct DataBufferWrapper {
    inner: Arc<DataBuffer>,
}

impl DataBufferWrapper {
    pub fn new(buf: DataBuffer) -> Self {
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

pub struct MinidumpBinaryViewType {
    view_type: BinaryViewType,
}

impl MinidumpBinaryViewType {
    pub fn new(view_type: BinaryViewType) -> Self {
        MinidumpBinaryViewType {
            view_type: view_type,
        }
    }
}

impl AsRef<BinaryViewType> for MinidumpBinaryViewType {
    fn as_ref(&self) -> &BinaryViewType {
        &self.view_type
    }
}

impl BinaryViewTypeBase for MinidumpBinaryViewType {
    fn is_deprecated(&self) -> bool {
        false
    }

    fn is_valid_for(&self, data: &BinaryView) -> bool {
        let mut magic_number = Vec::<u8>::new();
        data.read_into_vec(&mut magic_number, 0, 4);

        magic_number == b"MDMP"
    }
}

impl CustomBinaryViewType for MinidumpBinaryViewType {
    fn create_custom_view<'builder>(
        &self,
        data: &BinaryView,
        builder: binaryninja::custombinaryview::CustomViewBuilder<'builder, Self>,
    ) -> binaryninja::binaryview::Result<CustomView<'builder>> {
        debug!("Creating MinidumpBinaryView from registered MinidumpBinaryViewType");
        debug!(
            "Creating MinidumpBinaryView with passed data length {}",
            data.len()
        );

        let binary_view = builder.create::<MinidumpBinaryView>(data, ());
        binary_view
    }
}

#[derive(Debug)]
struct SegmentData {
    rva_range: Range<u64>,
    mapped_addr_range: Range<u64>,
}

impl SegmentData {
    fn from_addresses_and_size(rva: u64, mapped_addr: u64, size: u64) -> Self {
        SegmentData {
            rva_range: Range {
                start: rva,
                end: rva + size,
            },
            mapped_addr_range: Range {
                start: mapped_addr,
                end: mapped_addr + size,
            },
        }
    }
}

pub struct MinidumpBinaryView {
    inner: binaryninja::rc::Ref<BinaryView>,
}

impl MinidumpBinaryView {
    fn new(view: &BinaryView) -> Self {
        MinidumpBinaryView {
            inner: view.to_owned(),
        }
    }

    fn init(&self) -> binaryninja::binaryview::Result<()> {
        let parent_view = self.parent_view()?;
        let read_buffer = parent_view.read_buffer(0, parent_view.len())?;
        let read_buffer = DataBufferWrapper::new(read_buffer);

        if let Ok(minidump_obj) = Minidump::read(read_buffer) {
            // Architecture, platform information
            if let Ok(minidump_system_info) = minidump_obj.get_stream::<MinidumpSystemInfo>() {
                if let Some(platform) = MinidumpBinaryView::translate_minidump_platform(
                    minidump_system_info.cpu,
                    minidump_obj.endian,
                    minidump_system_info.os,
                ) {
                    self.set_default_platform(&platform);
                } else {
                    error!(
                        "Could not parse valid system information from minidump: could not map system information in MinidumpSystemInfo stream (arch {:?}, endian {:?}, os {:?}) to a known architecture",
                        minidump_system_info.cpu,
                        minidump_obj.endian,
                        minidump_system_info.os,
                    );
                    return Err(());
                }
            } else {
                error!("Could not parse system information from minidump: could not find a valid MinidumpSystemInfo stream");
                return Err(());
            }

            // Memory segments
            let mut segment_data = Vec::<SegmentData>::new();

            // 32-bit segments
            if let Ok(minidump_memory_list) = minidump_obj.get_stream::<MinidumpMemoryList>() {
                for memory_segment in minidump_memory_list.by_addr() {
                    debug!(
                        "Found 32-bit memory segment at RVA {:#x} with virtual address {:#x} and size {:#x}",
                        memory_segment.desc.memory.rva,
                        memory_segment.base_address,
                        memory_segment.size
                    );
                    segment_data.push(SegmentData::from_addresses_and_size(
                        memory_segment.desc.memory.rva as u64,
                        memory_segment.base_address,
                        memory_segment.size,
                    ));
                }
            } else {
                error!("Could not read 32-bit memory list from minidump: could not find a valid MinidumpMemoryList stream");
            }

            // 64-bit segments
            // Grab the shared base RVA for all entries in the MinidumpMemory64List,
            // since the minidump crate doesn't expose this to us
            if let Ok(raw_stream) = minidump_obj.get_raw_stream(MinidumpMemory64List::STREAM_TYPE) {
                let base_rva = u64::from_le_bytes(raw_stream[8..16].try_into().unwrap());
                debug!("Found BaseRVA value {:#x}", base_rva);

                if let Ok(minidump_memory_list) = minidump_obj.get_stream::<MinidumpMemory64List>()
                {
                    let mut current_rva = base_rva;
                    for memory_segment in minidump_memory_list.iter() {
                        debug!(
                            "Found 64-bit memory segment at RVA {:#x} with virtual address {:#x} and size {:#x}",
                            current_rva,
                            memory_segment.base_address,
                            memory_segment.size
                        );
                        segment_data.push(SegmentData::from_addresses_and_size(
                            current_rva.clone(),
                            memory_segment.base_address,
                            memory_segment.size,
                        ));
                        current_rva = current_rva + memory_segment.size;
                    }
                } else {
                    error!("Could not read 64-bit memory list from minidump: could not find a valid MinidumpMemoryList stream");
                }
            }

            for segment in segment_data.iter() {
                self.add_segment(
                    Segment::builder(segment.mapped_addr_range.clone())
                        .parent_backing(segment.rva_range.clone())
                        .is_auto(true),
                );
            }
        } else {
            error!("Could not parse data as minidump");
            return Err(());
        }
        Ok(())
    }

    fn translate_minidump_platform(
        minidump_cpu_arch: minidump::system_info::Cpu,
        minidump_endian: minidump::Endian,
        minidump_os: minidump::system_info::Os,
    ) -> Option<binaryninja::rc::Ref<Platform>> {
        match minidump_os {
            minidump::system_info::Os::Windows => match minidump_cpu_arch {
                minidump::system_info::Cpu::Arm64 => Platform::by_name("windows-aarch64"),
                minidump::system_info::Cpu::Arm => Platform::by_name("windows-armv7"),
                minidump::system_info::Cpu::X86 => Platform::by_name("windows-x86"),
                minidump::system_info::Cpu::X86_64 => Platform::by_name("windows-x86_64"),
                _ => None,
            },
            minidump::system_info::Os::MacOs => match minidump_cpu_arch {
                minidump::system_info::Cpu::Arm64 => Platform::by_name("mac-aarch64"),
                minidump::system_info::Cpu::Arm => Platform::by_name("mac-armv7"),
                minidump::system_info::Cpu::X86 => Platform::by_name("mac-x86"),
                minidump::system_info::Cpu::X86_64 => Platform::by_name("mac-x86_64"),
                _ => None,
            },
            minidump::system_info::Os::Linux => match minidump_cpu_arch {
                minidump::system_info::Cpu::Arm64 => Platform::by_name("linux-aarch64"),
                minidump::system_info::Cpu::Arm => Platform::by_name("linux-armv7"),
                minidump::system_info::Cpu::X86 => Platform::by_name("linux-x86"),
                minidump::system_info::Cpu::X86_64 => Platform::by_name("linux-x86_64"),
                minidump::system_info::Cpu::Ppc => match minidump_endian {
                    minidump::Endian::Little => Platform::by_name("linux-ppc32_le"),
                    minidump::Endian::Big => Platform::by_name("linux-ppc32"),
                },
                minidump::system_info::Cpu::Ppc64 => match minidump_endian {
                    minidump::Endian::Little => Platform::by_name("linux-ppc64_le"),
                    minidump::Endian::Big => Platform::by_name("linux-ppc64"),
                },
                _ => None,
            },
            minidump::system_info::Os::NaCl => None,
            minidump::system_info::Os::Android => None,
            minidump::system_info::Os::Ios => None,
            minidump::system_info::Os::Ps3 => None,
            minidump::system_info::Os::Solaris => None,
            _ => None,
        }
    }
}

impl AsRef<BinaryView> for MinidumpBinaryView {
    fn as_ref(&self) -> &BinaryView {
        &self.inner
    }
}

impl BinaryViewBase for MinidumpBinaryView {
    fn address_size(&self) -> usize {
        0
    }

    fn default_endianness(&self) -> Endianness {
        Endianness::LittleEndian
    }

    fn entry_point(&self) -> u64 {
        0
    }
}

unsafe impl CustomBinaryView for MinidumpBinaryView {
    type Args = ();

    fn new(handle: &BinaryView, _args: &Self::Args) -> binaryninja::binaryview::Result<Self> {
        Ok(MinidumpBinaryView::new(handle))
    }

    fn init(&self, _args: Self::Args) -> binaryninja::binaryview::Result<()> {
        self.init()
    }
}
