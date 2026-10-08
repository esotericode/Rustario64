//! Compile the vendored CC0 decomp collision sources with strict IEEE single
//! precision: no FMA contraction or fast-math, so results can be compared bit
//! for bit with the Rust port on the host.
fn main() {
    let files = [
        "c/oracle.c",
        "c/decomp/src/engine/surface_load.c",
        "c/decomp/src/engine/surface_collision.c",
    ];
    let mut build = cc::Build::new();
    build
        .std("gnu99")
        .opt_level(2)
        .include("c/shim")
        .include("c/decomp/src/engine")
        .include("c/decomp/include")
        // MIPS integer arithmetic wraps; make that defined for the host compiler too.
        .flag_if_supported("-fwrapv")
        .flag_if_supported("-ffp-contract=off")
        .flag_if_supported("-fno-fast-math")
        .flag_if_supported("-fexcess-precision=standard")
        .flag_if_supported("-w")
        .warnings(false);
    for file in files {
        build.file(file);
        println!("cargo:rerun-if-changed={file}");
    }
    for dir in ["c/shim", "c/decomp"] {
        println!("cargo:rerun-if-changed={dir}");
    }
    build.compile("rustario64_collision_oracle");
}
