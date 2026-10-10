/* Rustario64 oracle (authored, MIT). Compiles the verbatim behavior_data.c
 * excerpt and lists its scripts with their lengths, in the order the Rust
 * side passes their segmented addresses (oracle/src/object_trace.rs), so
 * snapshots can name behavior pointers by segmented address. */
#include "excerpts/behavior_data.c"

typedef struct {
    const BehaviorScript *start;
    s32 count;
} OracleScript;

#define SCRIPT(x) { x, (s32) (sizeof(x) / sizeof(x[0])) }
static const OracleScript sScripts[] = {
    SCRIPT(bhvCoinFormationSpawn), SCRIPT(bhvCoinFormation),      SCRIPT(bhvYellowCoin),
    SCRIPT(bhvCoinSparkles),       SCRIPT(bhvGoldenCoinSparkles), SCRIPT(bhvMario),
    SCRIPT(bhvSpinAirborneWarp),   SCRIPT(bhvSoundSpawner),       SCRIPT(bhvMovingYellowCoin),
    SCRIPT(bhvBobomb),             SCRIPT(bhvBobombFuseSmoke),    SCRIPT(bhvCarrySomething3),
    SCRIPT(bhvCarrySomething4),    SCRIPT(bhvCarrySomething5),    SCRIPT(bhvExplosion),
    SCRIPT(bhvBobombBullyDeathSmoke), SCRIPT(bhvRespawner),
};

s32 oracle_script_count(void) {
    return (s32) (sizeof(sScripts) / sizeof(sScripts[0]));
}

const OracleScript *oracle_script(s32 i) {
    return &sScripts[i];
}
