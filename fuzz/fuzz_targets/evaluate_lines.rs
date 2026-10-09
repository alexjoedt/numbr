#![no_main]

use libfuzzer_sys::fuzz_target;
use numbr_core::Engine;

fuzz_target!(|data: &str| {
    let mut engine = Engine::new();
    for line in data.lines() {
        engine.evaluate_line(line);
    }
});
