#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // PDF reader accepts raw bytes, should never panic
    let _ = rescribe_read_pdf::parse(data);
});
