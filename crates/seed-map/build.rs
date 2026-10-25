// SPDX-License-Identifier: MIT
fn main() {
    let mut build = cc::Build::new();
    build
        .include("vendor/cubiomes")
        .warnings(false)
        .opt_level(2);
    build.define("_USE_MATH_DEFINES", None);
    build.flag_if_supported("-fwrapv");
    build.flag_if_supported("-fno-strict-aliasing");
    build.flag_if_supported("/std:c11");
    build.flag_if_supported("/fp:precise");
    for source in [
        "biomes",
        "biomenoise",
        "finders",
        "generator",
        "layers",
        "noise",
        "util",
    ] {
        build.file(format!("vendor/cubiomes/{source}.c"));
    }
    build.file("src/bridge.c").compile("cobble_cubiomes");
    if std::env::var("CARGO_CFG_TARGET_FAMILY").as_deref() == Ok("unix") {
        println!("cargo:rustc-link-lib=m");
    }
    println!("cargo:rerun-if-changed=vendor/cubiomes");
    println!("cargo:rerun-if-changed=src/bridge.c");
}
