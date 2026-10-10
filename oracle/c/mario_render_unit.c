/* Rustario64 oracle (authored, MIT). Mario's object node in the render pass,
 * run through the verbatim traversal: geo_process_object and every node
 * processor his model and a held object's reach (excerpts/
 * rendering_traversal.c), geo_process_shadow (excerpts/shadow_geo.c),
 * obj_is_in_view (excerpts/rendering_view.c) and his verbatim geo callbacks
 * (excerpts/mario_misc.c, excerpts/behavior_actions.c). His graph is built
 * with the verbatim graph_node.c constructors from a node list the Rust side
 * sends: its decode of the ROM's mario_geo (checked independently by
 * tools/check_mario_model_reference.py) or an authored Mario-shaped
 * stand-in. The authored parts are the graph construction, the camera
 * node's matrix (geo_process_camera without its display lists, as
 * object_render_unit.c computes it), aborting processors for nodes no object
 * contains, and the castle mirror's two callbacks, which change nothing
 * outside the mirror room. Display lists are not built: no master list is
 * active, so geo_append_display_list appends nothing. Development
 * comparison tool only. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "rendering_traversal_boundary.h"
#include "game/level_update.h"
#include "game/mario_misc.h"
#include "game/object_list_processor.h"
#include "excerpts/rendering_view.c"
#include "excerpts/rendering_traversal.c"
#include "excerpts/shadow_geo.c"

Gfx *geo_move_mario_part_from_parent(s32 run, struct GraphNode *node, Mat4 mtx);
Gfx *geo_switch_anim_state(s32 callContext, struct GraphNode *node, void *context);
void oracle_camera_graph_nodes(struct GraphNodeCamera **camera, struct GraphNodePerspective **perspective);
extern struct GraphNode **gLoadedGraphNodes;

static void fail(const char *what) {
    fprintf(stderr, "Mario render oracle: %s\n", what);
    abort();
}

static void geo_process_ortho_projection(struct GraphNodeOrthoProjection *node) {
    (void) node;
    fail("an orthographic projection node inside an object's model");
}
static void geo_process_perspective(struct GraphNodePerspective *node) {
    (void) node;
    fail("a perspective node inside an object's model");
}
static void geo_process_master_list(struct GraphNodeMasterList *node) {
    (void) node;
    fail("a master list node inside an object's model");
}
static void geo_process_camera(struct GraphNodeCamera *node) {
    (void) node;
    fail("a camera node inside an object's model");
}
static void geo_process_background(struct GraphNodeBackground *node) {
    (void) node;
    fail("a background node inside an object's model");
}
static void geo_process_object_parent(struct GraphNodeObjectParent *node) {
    (void) node;
    fail("an object parent node inside an object's model");
}

/* The castle mirror's callbacks: they build display lists (and set drawing
 * layers) for the mirror room's Mario; outside it they change nothing the
 * simulation keeps. */
Gfx *geo_mirror_mario_set_alpha(s32 callContext, struct GraphNode *node, UNUSED Mat4 *c) {
    (void) callContext;
    (void) node;
    return NULL;
}
Gfx *geo_mirror_mario_backface_culling(s32 callContext, struct GraphNode *node, UNUSED Mat4 *c) {
    (void) callContext;
    (void) node;
    return NULL;
}

/* One node of MODEL_MARIO's graph, as the Rust side describes it (layout
 * matches `OracleGeoNode` in oracle/src/lib.rs). */
typedef struct {
    s32 kind;
    s32 parent;
    s32 layer;
    s32 hasDisplayList;
    s32 callback;
    s32 param;
    s32 param2;
    s32 param3;
    s16 a[3];
    s16 b[3];
    u32 scale;
    s32 flags;
} OracleGeoNode;

enum {
    GEO_START = 0, GEO_LOD = 1, GEO_SWITCH = 2, GEO_TRANSLATION_ROTATION = 3, GEO_TRANSLATION = 4,
    GEO_ROTATION = 5, GEO_SCALE = 6, GEO_ANIMATED_PART = 7, GEO_BILLBOARD = 8,
    GEO_DISPLAY_LIST = 9, GEO_SHADOW = 10, GEO_GENERATED = 11, GEO_HELD_OBJECT = 12,
    GEO_CULLING_RADIUS = 13
};

/* The callback roles, in `MarioCallback`'s order. */
static GraphNodeFunc callback_function(s32 role) {
    switch (role) {
        case -1: return NULL;
        case 0: return (GraphNodeFunc) geo_mirror_mario_backface_culling;
        case 1: return (GraphNodeFunc) geo_mirror_mario_set_alpha;
        case 2: return (GraphNodeFunc) geo_switch_mario_stand_run;
        case 3: return (GraphNodeFunc) geo_switch_mario_cap_effect;
        case 4: return (GraphNodeFunc) geo_switch_mario_cap_on_off;
        case 5: return (GraphNodeFunc) geo_switch_mario_eyes;
        case 6: return (GraphNodeFunc) geo_switch_mario_hand;
        case 7: return (GraphNodeFunc) geo_mario_head_rotation;
        case 8: return (GraphNodeFunc) geo_mario_tilt_torso;
        case 9: return (GraphNodeFunc) geo_mario_rotate_wing_cap_wings;
        case 10: return (GraphNodeFunc) geo_mario_hand_foot_scaler;
        case 11: return (GraphNodeFunc) geo_move_mario_part_from_parent;
        case 12: return (GraphNodeFunc) geo_switch_mario_hand_grab_pos;
    }
    fail("unknown Mario callback role");
    return NULL;
}

/* The display list a node draws: never interpreted, only tested for NULL. */
static u8 sDisplayListStandIn;

static OracleGeoNode *sMarioNodes;
static s32 sMarioNodeCount;
static s32 sMarioRootIndex = -1;
static void *sMarioGraph[2048];
static s32 sMarioGraphCount;

void oracle_render_set_mario_model(const OracleGeoNode *nodes, s32 count, s32 root) {
    free(sMarioNodes);
    sMarioNodes = NULL;
    sMarioNodeCount = 0;
    sMarioRootIndex = -1;
    if (count <= 0) {
        return;
    }
    if (count > (s32) (sizeof(sMarioGraph) / sizeof(sMarioGraph[0]))) {
        fail("Mario's model has too many nodes");
    }
    sMarioNodes = malloc(sizeof(OracleGeoNode) * (size_t) count);
    memcpy(sMarioNodes, nodes, sizeof(OracleGeoNode) * (size_t) count);
    sMarioNodeCount = count;
    sMarioRootIndex = root;
}

s32 oracle_render_has_mario(void) {
    return sMarioNodeCount > 0;
}

/* process_geo_layout's node creation for one described node. */
static struct GraphNode *build_node(const OracleGeoNode *in) {
    void *dl = in->hasDisplayList ? &sDisplayListStandIn : NULL;
    GraphNodeFunc func = callback_function(in->callback);
    Vec3s a, b;
    vec3s_set(a, in->a[0], in->a[1], in->a[2]);
    vec3s_set(b, in->b[0], in->b[1], in->b[2]);
    switch (in->kind) {
        case GEO_START:
            return &init_graph_node_start(NULL, calloc(1, sizeof(struct GraphNodeStart)))->node;
        case GEO_LOD:
            return &init_graph_node_render_range(NULL, calloc(1, sizeof(struct GraphNodeLevelOfDetail)),
                                                 (s16) in->param, (s16) in->param2)->node;
        case GEO_SWITCH:
            return &init_graph_node_switch_case(NULL, calloc(1, sizeof(struct GraphNodeSwitchCase)),
                                                (s16) in->param, 0, func, 0)->fnNode.node;
        case GEO_TRANSLATION_ROTATION:
            return &init_graph_node_translation_rotation(
                        NULL, calloc(1, sizeof(struct GraphNodeTranslationRotation)), in->layer, dl, a, b)->node;
        case GEO_TRANSLATION:
            return &init_graph_node_translation(NULL, calloc(1, sizeof(struct GraphNodeTranslation)),
                                                in->layer, dl, a)->node;
        case GEO_ROTATION:
            return &init_graph_node_rotation(NULL, calloc(1, sizeof(struct GraphNodeRotation)), in->layer,
                                             dl, b)->node;
        case GEO_SCALE:
            /* geo_layout_cmd_node_scale's cur_geo_cmd_u32(0x04) / 65536.0f. */
            return &init_graph_node_scale(NULL, calloc(1, sizeof(struct GraphNodeScale)), in->layer, dl,
                                          in->scale / 65536.0f)->node;
        case GEO_ANIMATED_PART:
            return &init_graph_node_animated_part(NULL, calloc(1, sizeof(struct GraphNodeAnimatedPart)),
                                                  in->layer, dl, a)->node;
        case GEO_BILLBOARD:
            return &init_graph_node_billboard(NULL, calloc(1, sizeof(struct GraphNodeBillboard)),
                                              in->layer, dl, a)->node;
        case GEO_DISPLAY_LIST:
            return &init_graph_node_display_list(NULL, calloc(1, sizeof(struct GraphNodeDisplayList)),
                                                 in->layer, dl)->node;
        case GEO_SHADOW:
            return &init_graph_node_shadow(NULL, calloc(1, sizeof(struct GraphNodeShadow)),
                                           (s16) in->param3, (u8) in->param2, (u8) in->param)->node;
        case GEO_GENERATED:
            return &init_graph_node_generated(NULL, calloc(1, sizeof(struct GraphNodeGenerated)), func,
                                              in->param)->fnNode.node;
        case GEO_HELD_OBJECT:
            return &init_graph_node_held_object(NULL, calloc(1, sizeof(struct GraphNodeHeldObject)), NULL,
                                                a, func, in->param)->fnNode.node;
        case GEO_CULLING_RADIUS:
            return &init_graph_node_culling_radius(NULL, calloc(1, sizeof(struct GraphNodeCullingRadius)),
                                                   (s16) in->param)->node;
    }
    fail("unknown node kind in Mario's model");
    return NULL;
}

/* A level load's LOAD_MODEL_FROM_GEO(MODEL_MARIO): fresh nodes from the
 * layout, linked in registration order, as gLoadedGraphNodes[MODEL_MARIO]. */
void oracle_render_load_mario(void) {
    struct GraphNode *built[2048];
    s32 i;
    for (i = 0; i < sMarioGraphCount; i++) {
        free(sMarioGraph[i]);
    }
    sMarioGraphCount = 0;
    if (sMarioNodeCount == 0) {
        return;
    }
    for (i = 0; i < sMarioNodeCount; i++) {
        const OracleGeoNode *in = &sMarioNodes[i];
        built[i] = build_node(in);
        sMarioGraph[sMarioGraphCount++] = built[i];
        if (built[i]->flags != (s16) in->flags) {
            fail("a Mario node's flags differ from the decoded layout's");
        }
        if (in->parent >= 0) {
            if (in->parent >= i) {
                fail("a Mario node precedes its parent");
            }
            geo_add_child(built[in->parent], built[i]);
        }
    }
    gLoadedGraphNodes[MODEL_MARIO] = built[sMarioRootIndex];
}

/* geo_mario_hand_foot_scaler keeps a function-local counter from boot. The
 * oracle runs many entries in one process, so each entry sets it to the
 * value a fresh boot gives at area update 0 (0) through the verbatim
 * function itself, and restores what that call touched. */
void oracle_render_prime_scaler(void) {
    struct GraphNodeGenerated scaler;
    struct GraphNodeScale scale;
    u8 punchState = gBodyStates[0].punchState;
    u16 counter = gAreaUpdateCounter;
    memset(&scaler, 0, sizeof(scaler));
    memset(&scale, 0, sizeof(scale));
    scaler.fnNode.node.next = &scale.node;
    scaler.parameter = 0;
    gAreaUpdateCounter = 0;
    gBodyStates[0].punchState = 1;
    geo_mario_hand_foot_scaler(GEO_CONTEXT_RENDER, &scaler.fnNode.node, NULL);
    gBodyStates[0].punchState = punchState;
    gAreaUpdateCounter = counter;
}

/* geo_process_node_and_siblings over gObjParentGraphNode's first child,
 * Mario's object node, under the camera node: geo_process_root's identity,
 * then geo_process_camera's transform. */
void oracle_render_mario(s8 rootAreaIndex) {
    struct GraphNodeRoot root;
    struct GraphNodeCamera *camera;
    struct GraphNodePerspective *perspective;
    struct Object *node = gMarioObject;
    Mat4 cameraTransform;
    Mtx *mtx;
    memset(&root, 0, sizeof(root));
    root.areaIndex = rootAreaIndex;
    gCurGraphNodeRoot = &root;
    oracle_camera_graph_nodes(&camera, &perspective);
    gCurGraphNodeCamFrustum = perspective;
    gMatStackIndex = 0;
    mtxf_identity(gMatStack[0]);
    mtxf_lookat(cameraTransform, camera->pos, camera->focus, camera->roll);
    mtxf_mul(gMatStack[1], cameraTransform, gMatStack[0]);
    gMatStackIndex = 1;
    mtx = alloc_display_list(sizeof(*mtx));
    mtxf_to_mtx(mtx, gMatStack[1]);
    gMatStackFixed[1] = mtx;
    gCurGraphNodeCamera = camera;
    camera->matrixPtr = &gMatStack[gMatStackIndex];
    if (node->header.gfx.node.flags & GRAPH_RENDER_ACTIVE) {
        if (node->header.gfx.node.flags & GRAPH_RENDER_CHILDREN_FIRST) {
            fail("Mario's object node processes its children first");
        }
        geo_process_object(node);
    } else {
        node->header.gfx.throwMatrix = NULL;
    }
    gCurGraphNodeCamera = NULL;
    gMatStackIndex = 0;
    gCurGraphNodeRoot = NULL;
}
