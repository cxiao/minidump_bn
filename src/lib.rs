use log::{LevelFilter, debug};

#[no_mangle]
#[allow(non_snake_case)]
pub extern "C" fn CorePluginInit() -> bool {
    binaryninja::logger::init(LevelFilter::Trace).expect("failed to initialize logging");
    
    debug!("Initializing binary view");
    true
}