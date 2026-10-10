/* Rustario64 oracle (authored, MIT). Host copies of the object animation
 * tables the verbatim scripts load, built from the Rust importer's decode of
 * the owner's ROM (or an authored fixture): each struct Animation has its
 * index and values arrays in host memory, as geo_obj_init_animation and the
 * render pass read them. Snapshots name tables and animations by the
 * segmented addresses the Rust side uses. No game data is stored here. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "sm64.h"

/* actors/common0.h's table: NULL-terminated. The definition drops the
 * declaration's second const so the harness can fill it. */
const struct Animation *bobomb_seg8_anims_0802396C[8];
const struct Animation *king_bobomb_seg5_anims_0500FE30[32];

typedef struct {
    u32 segmented;
    s16 flags, yTransDivisor, startFrame, loopStart, loopEnd, boneCount;
    const u16 *index;
    s32 indexCount;
    const s16 *values;
    s32 valueCount;
} OracleAnimation;

#define MAX_ANIMATIONS 32
typedef struct {
    struct Animation animations[MAX_ANIMATIONS];
    u32 addresses[MAX_ANIMATIONS];
    s32 count;
    u32 segmented;
} HostAnimationTable;
static HostAnimationTable sTables[2];

static void fail(const char *what) {
    fprintf(stderr, "object animation oracle: %s\n", what);
    abort();
}

/* Both imported tables are copied independently, including their arrays.
 * Zero entries clears an unused table between scenarios. */
void oracle_set_object_animations(s32 kind, u32 table, const OracleAnimation *anims, s32 count) {
    s32 i;
    HostAnimationTable *t;
    const struct Animation **pointers;
    s32 capacity;
    if (kind < 0 || kind >= 2) fail("table kind");
    t = &sTables[kind];
    pointers = kind == 0 ? bobomb_seg8_anims_0802396C : king_bobomb_seg5_anims_0500FE30;
    capacity = kind == 0 ? 8 : 32;
    if (count < 0 || count >= capacity) fail("table size");
    for (i = 0; i < t->count; i++) {
        free((void *) t->animations[i].index);
        free((void *) t->animations[i].values);
    }
    memset(t, 0, sizeof(*t));
    memset(pointers, 0, sizeof(*pointers) * capacity);
    for (i = 0; i < count; i++) {
        struct Animation *a = &t->animations[i];
        u16 *index = malloc(sizeof(u16) * (size_t) anims[i].indexCount);
        s16 *values = malloc(sizeof(s16) * (size_t) anims[i].valueCount);
        if (!index || !values) fail("allocation");
        memcpy(index, anims[i].index, sizeof(u16) * (size_t) anims[i].indexCount);
        memcpy(values, anims[i].values, sizeof(s16) * (size_t) anims[i].valueCount);
        a->flags = anims[i].flags;
        a->animYTransDivisor = anims[i].yTransDivisor;
        a->startFrame = anims[i].startFrame;
        a->loopStart = anims[i].loopStart;
        a->loopEnd = anims[i].loopEnd;
        a->unusedBoneCount = anims[i].boneCount;
        a->index = index;
        a->values = values;
        t->addresses[i] = anims[i].segmented;
        pointers[i] = a;
    }
    t->count = count;
    t->segmented = table;
}

u32 oracle_object_animation_address(const void *p) {
    s32 kind, i;
    for (kind = 0; kind < 2; kind++) {
        for (i = 0; i < sTables[kind].count; i++) {
            if (p == &sTables[kind].animations[i]) return sTables[kind].addresses[i];
        }
    }
    return 0;
}

u32 oracle_object_animation_table_address(const void *p) {
    if (p == (const void *) bobomb_seg8_anims_0802396C) return sTables[0].segmented;
    if (p == (const void *) king_bobomb_seg5_anims_0500FE30) return sTables[1].segmented;
    return 0;
}
