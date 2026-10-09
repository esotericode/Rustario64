//! Compile the vendored CC0 decomp collision sources with strict IEEE single
//! precision: no FMA contraction or fast-math, so results can be compared bit
//! for bit with the Rust port on the host.
fn main() {
    let files = [
        "c/oracle.c",
        "c/input_reference.c",
        "c/decomp/src/engine/surface_load.c",
        "c/decomp/src/engine/surface_collision.c",
        "c/decomp/src/engine/math_util.c",
        "c/decomp/src/game/mario_step.c",
    ];
    let mut build = cc::Build::new();
    build
        .std("gnu99")
        .opt_level(2)
        .include("c/shim")
        .include("c/decomp/src/engine")
        .include("c/decomp/src")
        .include("c/decomp/include")
        .include("c/decomp")
        // The decomp's macros.h requires these for non-IDO compilers.
        .define("NON_MATCHING", None)
        .define("VERSION_US", "1")
        // Standard-compliant table access (gCosineTable = gSineTable + 0x400).
        .define("AVOID_UB", None)
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
    println!("cargo:rerun-if-changed=c/input_reference.h");
    for dir in ["c/shim", "c/decomp"] {
        println!("cargo:rerun-if-changed={dir}");
    }
    build.compile("rustario64_collision_oracle");
}
