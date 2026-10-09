#![no_main]

use libfuzzer_sys::fuzz_target;
use numbr_core::Engine;

fuzz_target!(|data: &str| {
    let _ = Engine::new().evaluate(data);
});
