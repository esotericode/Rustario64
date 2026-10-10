/* Authored (MIT, Rustario64): the context the verbatim data/behavior_data.c
 * excerpt needs. As in behavior_data.c, object fields name their rawData
 * indices in this translation unit (the scripts only encode them). The
 * natives the excerpted scripts call are declared by the vendored headers;
 * bhv_mario_update and the debug natives are authored in tick.c and
 * runtime_glue.c. Animation tables the scripts load are host copies of the
 * ROM's (object_anims_unit.c). */
#ifndef ORACLE_BEHAVIOR_DATA_BOUNDARY_H
#define ORACLE_BEHAVIOR_DATA_BOUNDARY_H
#define OBJECT_FIELDS_INDEX_DIRECTLY
#include "sm64.h"
#include "object_constants.h"
#include "game/object_list_processor.h"
#include "game/interaction.h"
#include "game/behavior_actions.h"
#include "game/debug.h"
#include "game/object_helpers.h"
#include "behavior_data.h"
/* actors/common0.h's Bob-omb animation table; the harness fills a host
 * copy from the Rust side's ROM decode (object_anims_unit.c). */
extern const struct Animation *bobomb_seg8_anims_0802396C[];
#endif
