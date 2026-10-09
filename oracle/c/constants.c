/* Authored (MIT, Rustario64): exports the generated constant table. Object
 * fields evaluate to their rawData indices in this translation unit only. */
#define OBJECT_FIELDS_INDEX_DIRECTLY
#include "sm64.h"
#include "dialog_ids.h"
#include "surface_terrains.h"
#include "sounds.h"
#include "mario_animation_ids.h"
#include "level_table.h"
#include "object_constants.h"
#include "object_fields.h"
#include "game/camera.h"
#include "game/interaction.h"
#include "game/level_update.h"
#include "game/mario.h"
#include "game/save_file.h"
#include "engine/graph_node.h"
#include "constants.inc.c"

int oracle_constant_count(void) {
    return (int) (sizeof(sConstants) / sizeof(sConstants[0]));
}

const char *oracle_constant(int i, long long *value) {
    *value = sConstants[i].value;
    return sConstants[i].name;
}
