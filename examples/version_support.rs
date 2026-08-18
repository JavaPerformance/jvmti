//! Inspect the crate's verified JDK range and a selected release profile.

use jvmti_bindings::prelude::*;

fn main() {
    println!("verified JDK range: {MIN_SUPPORTED_JDK}..={MAX_VERIFIED_JDK}");
    let profile = release_profile(MAX_VERIFIED_JDK);
    println!("latest verified profile: {profile:?}");
}
