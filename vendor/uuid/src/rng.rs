//! Dependency-free source of randomness for version 4 UUIDs.
//!
//! This implementation reads from `/dev/urandom` on the host platform, so it
//! requires the standard library and a Unix-like environment (matching the
//! crate's only configured target, `x86_64-unknown-linux-gnu`).

use std::io::Read;

pub(crate) fn u128() -> u128 {
    let mut bytes = [0u8; 16];

    fill(&mut bytes);

    u128::from_ne_bytes(bytes)
}

fn fill(dest: &mut [u8]) {
    // NOTE: `File::open` lazily opens `/dev/urandom` on each call. The kernel
    // guarantees that reads from `/dev/urandom` never block once the device is
    // available, and `read_exact` takes care of short reads.
    let mut file = std::fs::File::open("/dev/urandom")
        .expect("could not open `/dev/urandom` for uuid randomness");
    file.read_exact(dest)
        .expect("could not read random bytes for uuid from `/dev/urandom`");
}
