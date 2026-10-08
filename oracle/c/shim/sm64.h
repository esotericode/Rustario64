/* Authored shim (MIT, Rustario64). Layers host declarations over the pinned
 * decomp's real include/sm64.h, so original constants come from the original
 * headers. object_fields.h is stubbed: these shim structs use plain fields. */
#ifndef ORACLE_SM64_H
#define ORACLE_SM64_H
#include <math.h>
#include "types.h"
#include_next "sm64.h"
#include "level_table.h"

#define GRAPH_RENDER_ACTIVE (1 << 0)             /* src/engine/graph_node.h */
#define TIME_STOP_ACTIVE (1 << 6)                /* src/game/object_list_processor.h */
#define MEMORY_POOL_LEFT 0                       /* src/game/memory.h */
#define O_POS_INDEX 0x06                         /* include/object_fields.h */
#define O_FACE_ANGLE_INDEX 0x12                  /* include/object_fields.h */

struct NumTimesCalled {
    s16 floor;
    s16 ceil;
    s16 wall;
};

extern s16 gCheckingSurfaceCollisionsForCamera;
extern s16 gFindFloorIncludeSurfaceIntangible;
extern TerrainData *gEnvironmentRegions;
extern s32 gEnvironmentLevels[20];
extern s16 gCCMEnteredSlide;
extern struct MarioState *gMarioState;
extern s32 gNumFindFloorMisses;
extern u32 gTimeStopState;
extern struct Object gMacroObjectDefaultParent;
extern struct Object *gMarioObject;
extern struct Object *gCurrentObject;
extern s32 gSurfaceNodesAllocated;
extern s32 gSurfacesAllocated;
extern s32 gNumStaticSurfaceNodes;
extern s32 gNumStaticSurfaces;
extern struct NumTimesCalled gNumCalls;
extern const BehaviorScript bhvDDDWarp[];
extern u32 gGlobalTimer;
extern s16 gCurrLevelNum;

void *main_pool_alloc(u32 size, u32 side);
void *segmented_to_virtual(const void *addr);
void reset_red_coins_collected(void);
f32 dist_between_objects(struct Object *obj1, struct Object *obj2);
void obj_build_transform_from_pos_and_angle(struct Object *obj, s16 posIndex, s16 angleIndex);
void obj_apply_scale_to_matrix(struct Object *obj, Mat4 dst, Mat4 src);
void spawn_special_objects(s16 areaIndex, TerrainData **specialObjList);
void spawn_macro_objects(s16 areaIndex, s16 *macroObjList);
void spawn_macro_objects_hardcoded(s16 areaIndex, s16 *macroObjList);
void set_text_array_x_y(s32 xOffset, s32 yOffset);
void print_debug_top_down_mapinfo(const char *str, s32 number);
#endif
