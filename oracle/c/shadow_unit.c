/* Development-only authored transport (MIT). Original routines compute all
 * vertex positions/alphas. The wrapper supplies a Mario animation and runs
 * the BOB player branch without allocating or emitting an N64 display list.
 * Other levels' lava and flying-carpet adjustments are outside this boundary. */
#include "excerpts/geo_misc.c"
#include "excerpts/shadow.c"
#include <stdlib.h>
#include <string.h>
#include "shadow_geo_boundary.h"

struct Object *gLuigiObject;
struct GraphNodeCamera *gCurGraphNodeCamera;
struct GraphNodeObject *gCurGraphNodeObject;
struct GraphNodeHeldObject *gCurGraphNodeHeldObject;
s16 gMatStackIndex;
Mat4 gMatStack[32];
Mtx *gMatStackFixed[32];
s8 gMarioOnIceOrCarpet;
static Vec3f sShadowOrigin;

/* Capture the actual arguments produced by the original geo routine. Its
 * display-list branch is intentionally skipped; none is built by the oracle. */
Gfx *create_shadow_below_xyz(f32 x, f32 y, f32 z, s16 scale, u8 alpha, s8 type) {
    sShadowOrigin[0] = x; sShadowOrigin[1] = y; sShadowOrigin[2] = z;
    return NULL;
}
void geo_append_display_list(void *list, s16 layer) { abort(); }
void *alloc_display_list(u32 size) { abort(); }
void geo_process_node_and_siblings(struct GraphNode *node) { }
#include "excerpts/shadow_geo.c"

void oracle_shadow_origin(const f32 *position, s16 yaw, s16 flags, s16 frame,
                          s16 yTrans, s16 divisor, const u16 *index,
                          const s16 *values, f32 childScale, f32 *out) {
    struct GraphNodeObject object;
    struct GraphNodeCamera camera;
    struct GraphNodeShadow shadow;
    struct GraphNodeScale scale;
    struct Animation animation;
    memset(&object, 0, sizeof(object));
    memset(&camera, 0, sizeof(camera));
    memset(&shadow, 0, sizeof(shadow));
    memset(&scale, 0, sizeof(scale));
    memset(&animation, 0, sizeof(animation));
    vec3f_copy(object.pos, (f32 *)position);
    object.scale[0] = 1.0f;
    object.angle[1] = yaw;
    animation.flags = flags; animation.animYTransDivisor = divisor;
    animation.index = index; animation.values = values;
    object.animInfo.curAnim = &animation;
    object.animInfo.animFrame = frame;
    object.animInfo.animYTrans = yTrans;
    gCurGraphNodeObject = &object; gCurGraphNodeCamera = &camera;
    gCurGraphNodeHeldObject = NULL;
    scale.node.type = GRAPH_NODE_TYPE_SCALE; scale.scale = childScale;
    shadow.node.children = &scale.node;
    geo_set_animation_globals(&object.animInfo, FALSE);
    geo_process_shadow(&shadow);
    vec3f_copy(out, sShadowOrigin);
    gCurGraphNodeObject = NULL; gCurGraphNodeCamera = NULL;
}

typedef struct {
    s16 position[3];
    s16 uv[2];
    u8 alpha;
} OracleShadowVertex;

int oracle_shadow(const f32 *position, s16 scale, u8 solidity, s16 animation,
                  s16 frame, OracleShadowVertex *out, u8 *layer) {
    struct Shadow shadow;
    struct Surface *surface;
    Vtx vertices[9];
    int i, ret;
    gShadowAboveWaterOrLava = FALSE;
    sMarioOnFlyingCarpet = FALSE;
    find_floor(position[0], position[1], position[2], &surface);
    *layer = surface != NULL && surface->type == SURFACE_ICE ? 5 : 6;
    gMarioObject->header.gfx.animInfo.animID = animation;
    gMarioObject->header.gfx.animInfo.animFrame = frame;
    ret = correct_shadow_solidity_for_animations(0, solidity, &shadow);
    if (ret == SHADOW_SOLIDITY_NO_SHADOW) return 0;
    ret = init_shadow(&shadow, position[0], position[1], position[2], scale,
                     ret == SHADOW_SOILDITY_ALREADY_SET ? 0 : solidity);
    if (ret != 0) return 0;
    if (gShadowAboveWaterOrLava) *layer = 4;
    for (i = 0; i < 9; i++) {
        make_shadow_vertex(vertices, i, shadow, SHADOW_WITH_9_VERTS);
        out[i].position[0] = vertices[i].v.ob[0];
        out[i].position[1] = vertices[i].v.ob[1];
        out[i].position[2] = vertices[i].v.ob[2];
        out[i].uv[0] = vertices[i].v.tc[0];
        out[i].uv[1] = vertices[i].v.tc[1];
        out[i].alpha = vertices[i].v.cn[3];
    }
    return 1;
}
