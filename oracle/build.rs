//! Compile the vendored CC0 decomp sources and the authored oracle glue with
//! strict IEEE single precision: no FMA contraction or fast-math, so results
//! can be compared bit for bit with the Rust port on the host.
fn main() {
    let files = [
        "c/oracle.c",
        "c/tick.c",
        "c/camera_unit.c",
        "c/rng_unit.c",
        "c/shadow_unit.c",
        "c/constants.c",
        "c/runtime_glue.c",
        "c/interaction_unit.c",
        "c/behavior_data_unit.c",
        "c/object_render_unit.c",
        "c/object_step_unit.c",
        "c/obj_behaviors_unit.c",
        "c/object_anims_unit.c",
        "c/excerpts/spawn_sound.c",
        "c/excerpts/sound_spawner.c",
        "c/excerpts/coin.c",
        "c/excerpts/geo_layout.c",
        "c/excerpts/game_init.c",
        "c/excerpts/graph_node.c",
        "c/excerpts/level_update.c",
        "c/excerpts/rendering_graph_node.c",
        "c/excerpts/macro_special_objects.c",
        "c/excerpts/object_helpers.c",
        "c/excerpts/object_list_processor.c",
        "c/excerpts/platform_displacement.c",
        "c/decomp/src/engine/surface_load.c",
        "c/decomp/src/engine/surface_collision.c",
        "c/decomp/src/engine/math_util.c",
        "c/decomp/src/game/mario.c",
        "c/decomp/src/game/mario_actions_airborne.c",
        "c/decomp/src/game/mario_actions_automatic.c",
        "c/decomp/src/game/mario_actions_moving.c",
        "c/decomp/src/game/mario_actions_object.c",
        "c/decomp/src/game/mario_actions_stationary.c",
        "c/decomp/src/game/mario_step.c",
        "c/decomp/src/game/object_collision.c",
        "c/decomp/src/game/spawn_object.c",
    ];
    let mut build = cc::Build::new();
    build
        .std("gnu99")
        .opt_level(2)
        // Authored libultra stand-ins come first; the SDK headers are not vendored.
        .include("c/shim")
        .include("c/decomp/include")
        .include("c/decomp/src")
        .include("c/decomp")
        .include("c")
        // The decomp's macros.h requires these for non-IDO compilers.
        .define("NON_MATCHING", None)
        .define("VERSION_US", "1")
        .define("_LANGUAGE_C", None)
        // Standard-compliant table access (gCosineTable = gSineTable + 0x400)
        // and the decomp's defined return values for missing-return functions.
        .define("AVOID_UB", None)
        // Host pointers: segment/physical address conversions are identities.
        .define("NO_SEGMENTED_MEMORY", None)
        // MIPS integer arithmetic wraps; make that defined for the host compiler too.
        .flag_if_supported("-fwrapv")
        .flag_if_supported("-ffp-contract=off")
        .flag_if_supported("-fno-fast-math")
        .flag_if_supported("-fexcess-precision=standard")
        // An implicit declaration silently passes floats as doubles and reads
        // an int result; every call must see its prototype.
        .flag("-Werror=implicit-function-declaration")
        .flag("-Werror=implicit-int")
        // The decomp is not written for host compilers' default warnings;
        // keep them out of cargo's output (errors still fail the build).
        // Not `warnings(false)`: its -w also silences the -Werror
        // diagnostics above, so implicit declarations would compile.
        .cargo_warnings(false);
    for file in files {
        build.file(file);
        println!("cargo:rerun-if-changed={file}");
    }
    for dir in [
        "c/shim",
        "c/decomp",
        "c/excerpts",
        "c/runtime.h",
        "c/constants.inc.c",
        "c/lakitu_state.inc.c",
        "c/object_step_boundary.h",
        "c/obj_behaviors_boundary.h",
    ] {
        println!("cargo:rerun-if-changed={dir}");
    }
    build.compile("rustario64_collision_oracle");
}
