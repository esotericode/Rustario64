/* Authored (MIT, Rustario64): the context of the verbatim render traversal
 * excerpt (excerpts/rendering_traversal.c) in c/mario_render_unit.c,
 * standing in for rendering_graph_node.c's own includes and the file-local
 * processors the excerpt does not copy. The matrix stack, the current-node
 * globals and the animation globals keep their existing definitions
 * (shadow_unit.c, object_render_unit.c, excerpts/rendering_graph_node.c);
 * display-list allocations come from the per-frame arena
 * (oracle_frame_alloc_display_list in runtime_glue.c). */
#ifndef ORACLE_RENDERING_TRAVERSAL_BOUNDARY_H
#define ORACLE_RENDERING_TRAVERSAL_BOUNDARY_H
#include <PR/ultratypes.h>
void *oracle_frame_alloc_display_list(u32 size);
#define alloc_display_list oracle_frame_alloc_display_list
#include "sm64.h"
#include "engine/graph_node.h"
#include "engine/math_util.h"
#include "game/game_init.h"
#include "game/memory.h"
#include "game/rendering_graph_node.h"

extern s16 gMatStackIndex;
extern Mat4 gMatStack[32];
extern Mtx *gMatStackFixed[32];
extern u8 gCurrAnimType;
extern u8 gCurrAnimEnabled;
extern s16 gCurrAnimFrame;
extern f32 gCurrAnimTranslationMultiplier;
extern u16 *gCurrAnimAttribute;
extern s16 *gCurrAnimData;
void geo_set_animation_globals(struct AnimInfo *node, s32 hasAnimation);
/* shadow.c's, read by the shadow node (excerpts/shadow.c defines it). */
extern s8 gShadowAboveWaterOrLava;

/* rendering_graph_node.c's processors outside the excerpt: the shadow's comes
 * from excerpts/shadow_geo.c and obj_is_in_view from excerpts/rendering_view.c;
 * the rest only process scene nodes above the objects, so the oracle's
 * versions abort (c/mario_render_unit.c). */
static void geo_process_ortho_projection(struct GraphNodeOrthoProjection *node);
static void geo_process_perspective(struct GraphNodePerspective *node);
static void geo_process_master_list(struct GraphNodeMasterList *node);
static void geo_process_camera(struct GraphNodeCamera *node);
static void geo_process_background(struct GraphNodeBackground *node);
static void geo_process_object_parent(struct GraphNodeObjectParent *node);
static void geo_process_shadow(struct GraphNodeShadow *node);
static s32 obj_is_in_view(struct GraphNodeObject *node, Mat4 matrix);
#endif
