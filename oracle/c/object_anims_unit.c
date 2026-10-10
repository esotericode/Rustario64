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

typedef struct {
    u32 segmented;
    s16 flags, yTransDivisor, startFrame, loopStart, loopEnd, boneCount;
    const u16 *index;
    s32 indexCount;
    const s16 *values;
    s32 valueCount;
} OracleAnimation;

#define MAX_ANIMATIONS 32
static struct Animation sAnimations[MAX_ANIMATIONS];
static u32 sAnimationAddresses[MAX_ANIMATIONS];
static s32 sAnimationCount;
static u32 sBobombTable;

static void fail(const char *what) {
    fprintf(stderr, "object animation oracle: %s\n", what);
    abort();
}

/* The Bob-omb's table at segmented address `table`, with its animations in
 * entry order. */
void oracle_set_bobomb_animations(u32 table, const OracleAnimation *anims, s32 count) {
    s32 i;
    for (i = 0; i < sAnimationCount; i++) {
        free((void *) sAnimations[i].index);
        free((void *) sAnimations[i].values);
    }
    memset(sAnimations, 0, sizeof(sAnimations));
    memset(bobomb_seg8_anims_0802396C, 0, sizeof(bobomb_seg8_anims_0802396C));
    if (count < 0 || count >= MAX_ANIMATIONS || count >= 8) {
        fail("table size");
    }
    for (i = 0; i < count; i++) {
        struct Animation *a = &sAnimations[i];
        u16 *index = malloc(sizeof(u16) * (size_t) anims[i].indexCount);
        s16 *values = malloc(sizeof(s16) * (size_t) anims[i].valueCount);
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
        a->length = 0;
        sAnimationAddresses[i] = anims[i].segmented;
        bobomb_seg8_anims_0802396C[i] = a;
    }
    sAnimationCount = count;
    sBobombTable = table;
}

/* A loaded animation's segmented address; 0 if it is not one. */
u32 oracle_object_animation_address(const void *p) {
    s32 i;
    for (i = 0; i < sAnimationCount; i++) {
        if (p == &sAnimations[i]) {
            return sAnimationAddresses[i];
        }
    }
    return 0;
}

/* A table pointer's segmented address; 0 if it is not one. */
u32 oracle_object_animation_table_address(const void *p) {
    if (p == (const void *) bobomb_seg8_anims_0802396C && sBobombTable != 0) {
        return sBobombTable;
    }
    return 0;
}
