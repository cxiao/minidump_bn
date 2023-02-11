use binaryninja::binaryview::BinaryView;
use binaryninja::command::{register, Command};
use log::{debug, LevelFilter};

mod loader;

struct PrintMemoryInformationCommand;

impl Command for PrintMemoryInformationCommand {
    fn action(&self, binary_view: &BinaryView) {
        loader::print_memory_information(binary_view);
    }

    fn valid(&self, _binary_view: &BinaryView) -> bool {
        true // TODO: Of course, the command will not always be valid!
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn CorePluginInit() -> bool {
    binaryninja::logger::init(LevelFilter::Trace).expect("failed to initialize logging");

    debug!("Registering minidump plugin commands");
    register(
        "Minidump\\[DEBUG] Print memory information",
        "",
        PrintMemoryInformationCommand {},
    );

    true
}
