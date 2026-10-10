/* Rustario64 oracle (authored, MIT). The render pass's writes to object
 * state for objects other than Mario, around verbatim functions: the camera
 * node's transform (mtxf_lookat with the node's roll, applied to the root's
 * identity, as geo_process_camera does), geo_process_object's matrix
 * (the throw matrix, mtxf_billboard or mtxf_rotate_zxy_and_translate,
 * mtxf_mul, mtxf_scale_vec3f), the verbatim geo_set_animation_globals for
 * animated objects, the verbatim obj_is_in_view (excerpts/rendering_view.c)
 * and, for objects in view, the model's switch callbacks (the verbatim
 * geo_switch_anim_state) with geo_process_switch's child selection. The
 * models' node trees come from the Rust importer's ROM decode
 * (tools/check_object_model_reference.py checks the layouts independently).
 * Display lists and drawing are not modelled. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "sm64.h"
#include "engine/graph_node.h"
#include "engine/math_util.h"
#include "game/object_list_processor.h"
#include "game/rendering_graph_node.h"

struct GraphNodePerspective *gCurGraphNodeCamFrustum;
#include "excerpts/rendering_view.c"

Gfx *geo_switch_anim_state(s32 callContext, struct GraphNode *node, void *context);
void geo_set_animation_globals(struct AnimInfo *node, s32 hasAnimation);
void oracle_camera_graph_nodes(struct GraphNodeCamera **camera, struct GraphNodePerspective **perspective);
extern struct GraphNode **gLoadedGraphNodes;

typedef struct {
    s32 model;
    s32 root;
    s32 nodeCount;
    const s32 *kinds;
    const s32 *params;
    const s32 *childStart;
    const s32 *childCount;
    const s32 *children;
} OracleModel;

enum { NODE_PLAIN = 0, NODE_ANIM_STATE_SWITCH = 1, NODE_CULLING_RADIUS = 2, NODE_UNAUDITED = 3 };

typedef struct {
    s32 loaded;
    s32 root;
    s32 nodeCount;
    s32 *kinds;
    s32 *params;
    s32 *childStart;
    s32 *childCount;
    s32 *children;
} RenderModel;

static RenderModel sModels[0x100];
/* gLoadedGraphNodes' entries: each loaded model's root node, typed as the
 * root's kind so obj_is_in_view reads a culling radius only from one. */
static struct GraphNodeCullingRadius sModelRoots[0x100];
static Mat4 sCameraMatrix;

static void fail(const char *what) {
    fprintf(stderr, "object render oracle: %s\n", what);
    abort();
}

static s32 *copy(const s32 *src, s32 n) {
    s32 *dst = malloc(sizeof(s32) * (size_t) (n > 0 ? n : 1));
    if (n > 0) {
        memcpy(dst, src, sizeof(s32) * (size_t) n);
    }
    return dst;
}

void oracle_render_set_models(const OracleModel *models, s32 count) {
    s32 i;
    for (i = 0; i < 0x100; i++) {
        RenderModel *m = &sModels[i];
        free(m->kinds);
        free(m->params);
        free(m->childStart);
        free(m->childCount);
        free(m->children);
        memset(m, 0, sizeof(*m));
    }
    for (i = 0; i < count; i++) {
        const OracleModel *in = &models[i];
        RenderModel *m;
        s32 j, childTotal = 0;
        if (in->model <= 0 || in->model >= 0x100) {
            fail("model ID outside gLoadedGraphNodes");
        }
        m = &sModels[in->model];
        m->loaded = TRUE;
        m->root = in->root;
        m->nodeCount = in->nodeCount;
        for (j = 0; j < in->nodeCount; j++) {
            if (in->childStart[j] + in->childCount[j] > childTotal) {
                childTotal = in->childStart[j] + in->childCount[j];
            }
        }
        m->kinds = copy(in->kinds, in->nodeCount);
        m->params = copy(in->params, in->nodeCount);
        m->childStart = copy(in->childStart, in->nodeCount);
        m->childCount = copy(in->childCount, in->nodeCount);
        m->children = copy(in->children, childTotal);
    }
}

/* level_main_scripts_entry and the level script's registrations. */
void oracle_render_load_models(void) {
    s32 i;
    memset(sModelRoots, 0, sizeof(sModelRoots));
    for (i = 0; i < 0x100; i++) {
        gLoadedGraphNodes[i] = NULL;
        if (sModels[i].loaded) {
            struct GraphNodeCullingRadius *root = &sModelRoots[i];
            s32 cull = sModels[i].nodeCount > 0 && sModels[i].kinds[sModels[i].root] == NODE_CULLING_RADIUS;
            root->node.type = cull ? GRAPH_NODE_TYPE_CULLING_RADIUS : GRAPH_NODE_TYPE_START;
            root->node.flags = GRAPH_RENDER_ACTIVE;
            root->cullingRadius = cull ? (s16) sModels[i].params[sModels[i].root] : 0;
            gLoadedGraphNodes[i] = &root->node;
        }
    }
}

/* The model ID of a gLoadedGraphNodes entry. */
s32 oracle_render_model_of(struct GraphNode *node) {
    s32 i;
    for (i = 0; i < 0x100; i++) {
        if (gLoadedGraphNodes[i] == node && node != NULL) {
            return i;
        }
    }
    fail("a graph node outside gLoadedGraphNodes");
    return -1;
}

/* The camera node's transform above GEO_RENDER_OBJ, after the camera nodes
 * ran this frame's callbacks. */
void oracle_render_begin(void) {
    struct GraphNodeCamera *camera;
    struct GraphNodePerspective *perspective;
    Mat4 cameraTransform, root;
    oracle_camera_graph_nodes(&camera, &perspective);
    gCurGraphNodeCamFrustum = perspective;
    mtxf_identity(root);
    mtxf_lookat(cameraTransform, camera->pos, camera->focus, camera->roll);
    mtxf_mul(sCameraMatrix, cameraTransform, root);
}

static void process_node(struct Object *obj, RenderModel *m, s32 node) {
    s32 i;
    switch (m->kinds[node]) {
        case NODE_PLAIN:
        case NODE_CULLING_RADIUS:
            for (i = 0; i < m->childCount[node]; i++) {
                process_node(obj, m, m->children[m->childStart[node] + i]);
            }
            break;
        case NODE_ANIM_STATE_SWITCH: {
            struct GraphNodeSwitchCase switchCase;
            s32 selected;
            memset(&switchCase, 0, sizeof(switchCase));
            switchCase.numCases = (s16) m->params[node];
            geo_switch_anim_state(GEO_CONTEXT_RENDER, &switchCase.fnNode.node, NULL);
            /* geo_process_switch: walk the circular sibling list. */
            if (m->childCount[node] > 0) {
                selected = 0;
                for (i = 0; switchCase.selectedCase > i; i++) {
                    selected = (selected + 1) % m->childCount[node];
                }
                process_node(obj, m, m->children[m->childStart[node] + selected]);
            }
            break;
        }
        default:
            fail("the render pass reaches an unaudited node");
    }
}

/* geo_process_object's state changes for one object other than Mario. */
void oracle_render_object(struct Object *obj, s8 rootAreaIndex) {
    struct GraphNodeObject *node = &obj->header.gfx;
    s32 hasAnimation = (node->node.flags & GRAPH_RENDER_HAS_ANIMATION) != 0;
    Mat4 placed, matrix;
    if (!(node->node.flags & GRAPH_RENDER_ACTIVE)) {
        node->throwMatrix = NULL;
        return;
    }
    if (node->areaIndex != rootAreaIndex) {
        return;
    }
    if (node->throwMatrix != NULL) {
        mtxf_mul(placed, *node->throwMatrix, sCameraMatrix);
    } else if (node->node.flags & GRAPH_RENDER_BILLBOARD) {
        struct GraphNodeCamera *camera;
        struct GraphNodePerspective *perspective;
        oracle_camera_graph_nodes(&camera, &perspective);
        mtxf_billboard(placed, sCameraMatrix, node->pos, camera->roll);
    } else {
        Mat4 local;
        mtxf_rotate_zxy_and_translate(local, node->pos, node->angle);
        mtxf_mul(placed, local, sCameraMatrix);
    }
    mtxf_scale_vec3f(matrix, placed, node->scale);
    if (node->animInfo.curAnim != NULL) {
        geo_set_animation_globals(&node->animInfo, hasAnimation);
    }
    if (obj_is_in_view(node, matrix) && node->sharedChild != NULL) {
        s32 model = oracle_render_model_of(node->sharedChild);
        if (sModels[model].nodeCount == 0) {
            fail("a drawn model without a traversal");
        }
        gCurGraphNodeObject = node;
        gCurGraphNodeHeldObject = NULL;
        process_node(obj, &sModels[model], sModels[model].root);
        gCurGraphNodeObject = NULL;
    }
    node->throwMatrix = NULL;
}
