use std::ops::Deref;
use std::sync::Arc;

use log::{debug, error};
use minidump::{Minidump, MinidumpSystemInfo};

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
            // MinidumpSystemInfo
            if let Ok(minidump_system_info) = minidump_obj.get_stream::<MinidumpSystemInfo>() {
                if let Some(platform) = MinidumpBinaryView::translate_minidump_platform(
                    minidump_system_info.cpu,
                    minidump_obj.endian,
                    minidump_system_info.os,
                ) {
                    self.set_default_platform(&platform);
                }
            } else {
                error!("Could not parse system information from minidump: could not find a valid MinidumpSystemInfo stream");
                return Err(());
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
