/* Rustario64 component oracle glue (authored, MIT). Development comparison
 * tool only; it is never linked into the game runtime. See oracle/README.md
 * for its boundary and replacement plan.
 *
 * Entry points for the collision, math, Mario step and input checks. They run
 * the vendored, unmodified CC0 decomp code (oracle/c/decomp) on the original
 * struct layouts from the vendored headers. */
#include <stdlib.h>
#include <string.h>
#include "sm64.h"
#include "engine/math_util.h"
#include "engine/surface_collision.h"
#include "engine/surface_load.h"
#include "game/area.h"
#include "game/camera.h"
#include "game/game_init.h"
#include "game/interaction.h"
#include "game/level_update.h"
#include "game/mario.h"
#include "game/mario_misc.h"
#include "game/mario_step.h"
#include "game/object_list_processor.h"
#include "runtime.h"

void oracle_free_pools(void);

static struct Object sMario;
static struct Object sOtherObject;
static struct Area sArea;
static struct Camera sCamera;
static TerrainData *sData;

typedef struct {
    s16 type;
    s16 force;
    s8 flags;
    s8 room;
    s16 lowerY;
    s16 upperY;
    s16 vertices[9];
    f32 normal[3];
    f32 originOffset;
} OracleSurface;

int oracle_load(const s16 *data, int words) {
    oracle_free_pools();
    free(sData);
    sData = malloc(sizeof(TerrainData) * (size_t) words);
    memcpy(sData, data, sizeof(TerrainData) * (size_t) words);
    memset(&sMario, 0, sizeof(sMario));
    memset(&sOtherObject, 0, sizeof(sOtherObject));
    memset(&gMarioStates[0], 0, sizeof(gMarioStates[0]));
    gMarioObject = &sMario;
    gMarioState = &gMarioStates[0];
    gCurrentObject = &sOtherObject;
    gTimeStopState = 0;
    gCheckingSurfaceCollisionsForCamera = 0;
    gFindFloorIncludeSurfaceIntangible = 0;
    alloc_surface_pools();
    clear_dynamic_surfaces();
    load_area_terrain(0, sData, NULL, NULL);
    return gSurfacesAllocated;
}

int oracle_node_count(void) {
    return gSurfaceNodesAllocated;
}

void oracle_get_surface(int index, OracleSurface *out) {
    struct Surface *s = &sSurfacePool[index];
    out->type = s->type;
    out->force = s->force;
    out->flags = s->flags;
    out->room = s->room;
    out->lowerY = s->lowerY;
    out->upperY = s->upperY;
    memcpy(&out->vertices[0], s->vertex1, sizeof(Vec3Terrain));
    memcpy(&out->vertices[3], s->vertex2, sizeof(Vec3Terrain));
    memcpy(&out->vertices[6], s->vertex3, sizeof(Vec3Terrain));
    out->normal[0] = s->normal.x;
    out->normal[1] = s->normal.y;
    out->normal[2] = s->normal.z;
    out->originOffset = s->originOffset;
}

int oracle_surface_index(struct Surface *s) {
    return s == NULL ? -1 : s == &gWaterSurfacePseudoFloor ? -2 : (int) (s - sSurfacePool);
}

struct Surface *oracle_surface_ref(s32 i) {
    return i == -1 ? NULL : i == -2 ? &gWaterSurfacePseudoFloor : &sSurfacePool[i];
}

int oracle_cell_list(int dynamic, int cellZ, int cellX, int kind, s32 *out, int max) {
    struct SurfaceNode *node = dynamic ? gDynamicSurfacePartition[cellZ][cellX][kind].next
                                       : gStaticSurfacePartition[cellZ][cellX][kind].next;
    int n = 0;
    while (node != NULL) {
        if (n < max) {
            out[n] = oracle_surface_index(node->surface);
        }
        n++;
        node = node->next;
    }
    return n;
}

static void set_context(s16 forCamera, int passVanishWalls) {
    gCheckingSurfaceCollisionsForCamera = forCamera;
    sOtherObject.activeFlags = passVanishWalls ? ACTIVE_FLAG_MOVE_THROUGH_GRATE : 0;
    gCurrentObject = &sOtherObject;
}

f32 oracle_find_floor(f32 x, f32 y, f32 z, s16 includeIntangible, s16 forCamera, s32 *index,
                      s16 *includeAfter) {
    struct Surface *floor;
    f32 height;
    set_context(forCamera, 0);
    gFindFloorIncludeSurfaceIntangible = includeIntangible;
    height = find_floor(x, y, z, &floor);
    *index = oracle_surface_index(floor);
    *includeAfter = gFindFloorIncludeSurfaceIntangible;
    return height;
}

f32 oracle_find_ceil(f32 x, f32 y, f32 z, s16 forCamera, s32 *index) {
    struct Surface *ceil;
    f32 height;
    set_context(forCamera, 0);
    height = find_ceil(x, y, z, &ceil);
    *index = oracle_surface_index(ceil);
    return height;
}

s32 oracle_find_walls(f32 *position, f32 offsetY, f32 radius, s16 forCamera, s32 passVanishWalls,
                      s16 *numWalls, s32 *walls) {
    struct WallCollisionData data;
    s32 n, i;
    set_context(forCamera, passVanishWalls);
    memset(&data, 0, sizeof(data));
    data.x = position[0];
    data.y = position[1];
    data.z = position[2];
    data.offsetY = offsetY;
    data.radius = radius;
    n = find_wall_collisions(&data);
    position[0] = data.x;
    position[1] = data.y;
    position[2] = data.z;
    *numWalls = data.numWalls;
    for (i = 0; i < 4; i++) {
        walls[i] = i < data.numWalls ? oracle_surface_index(data.walls[i]) : -1;
    }
    return n;
}

f32 oracle_find_water_level(f32 x, f32 z) {
    return find_water_level(x, z);
}

f32 oracle_find_poison_gas_level(f32 x, f32 z) {
    return find_poison_gas_level(x, z);
}

/* ---- math_util.c oracle ---- */
extern f32 gSineTable[];
extern s16 gArctanTable[0x401];

void oracle_set_trig(const f32 *sine, const s16 *arctan) {
    memcpy(gSineTable, sine, sizeof(f32) * 0x1400);
    memcpy(gArctanTable, arctan, sizeof(s16) * 0x401);
}

f32 oracle_sins(s32 x) {
    return sins(x);
}

f32 oracle_coss(s32 x) {
    return coss(x);
}

s16 oracle_atan2s(f32 y, f32 x) {
    return atan2s(y, x);
}

f32 oracle_atan2f(f32 y, f32 x) {
    return atan2f(y, x);
}

s32 oracle_approach_s32(s32 current, s32 target, s32 inc, s32 dec) {
    return approach_s32(current, target, inc, dec);
}

f32 oracle_approach_f32(f32 current, f32 target, f32 inc, f32 dec) {
    return approach_f32(current, target, inc, dec);
}

/* ---- Mario step and input oracles ---- */
/* Defined in mario_step.c but not declared in mario_step.h. */
void apply_gravity(struct MarioState *m);
void apply_vertical_wind(struct MarioState *m);
void set_vel_from_pitch_and_yaw(struct MarioState *m);
void set_vel_from_yaw(struct MarioState *m);

typedef struct {
    u16 input;
    u32 flags;
    u32 action;
    u32 terrainSoundAddend;
    u32 particleFlags;
    u32 collidedObjInteractTypes;
    f32 intendedMag;
    s16 intendedYaw;
    u8 framesSinceA;
    u8 framesSinceB;
    u8 squishTimer;
    u8 wallKickTimer;
    u8 doubleJumpTimer;

    s16 faceAngle[3];
    s16 angleVel[3];
    f32 pos[3];
    f32 vel[3];
    f32 forwardVel;
    f32 slideVelX;
    f32 slideVelZ;
    s32 wall, ceil, floor; /* surface index, -1 NULL, -2 water pseudo-floor */
    f32 ceilHeight;
    f32 floorHeight;
    s16 floorAngle;
    s16 waterLevel;
    f32 peakHeight;
    f32 quicksandDepth;
    f32 gettingBlownGravity;
    s8 wingFlutter;
    f32 gfxPos[3];
    s16 gfxAngle[3];
    /* World inputs and outputs. */
    u32 globalTimer;
    u16 areaTerrainType;
    s16 levelNum;
    f32 waterPseudoOriginOffset;
    s16 includeIntangible;
} OracleMario;

static struct MarioState *mario_in(const OracleMario *o) {
    struct MarioState *m = &gMarioStates[0];
    memset(m, 0, sizeof(*m));
    m->input = o->input;
    m->flags = o->flags;
    m->action = o->action;
    m->terrainSoundAddend = o->terrainSoundAddend;
    m->particleFlags = o->particleFlags;
    m->collidedObjInteractTypes = o->collidedObjInteractTypes;
    m->intendedMag = o->intendedMag;
    m->intendedYaw = o->intendedYaw;
    m->framesSinceA = o->framesSinceA;
    m->framesSinceB = o->framesSinceB;
    m->squishTimer = o->squishTimer;
    m->wallKickTimer = o->wallKickTimer;
    m->doubleJumpTimer = o->doubleJumpTimer;
    memcpy(m->faceAngle, o->faceAngle, sizeof(Vec3s));
    memcpy(m->angleVel, o->angleVel, sizeof(Vec3s));
    memcpy(m->pos, o->pos, sizeof(Vec3f));
    memcpy(m->vel, o->vel, sizeof(Vec3f));
    m->forwardVel = o->forwardVel;
    m->slideVelX = o->slideVelX;
    m->slideVelZ = o->slideVelZ;
    m->wall = oracle_surface_ref(o->wall);
    m->ceil = oracle_surface_ref(o->ceil);
    m->floor = oracle_surface_ref(o->floor);
    m->ceilHeight = o->ceilHeight;
    m->floorHeight = o->floorHeight;
    m->floorAngle = o->floorAngle;
    m->waterLevel = o->waterLevel;
    m->peakHeight = o->peakHeight;
    m->quicksandDepth = o->quicksandDepth;
    m->gettingBlownGravity = o->gettingBlownGravity;
    memset(&gBodyStates[0], 0, sizeof(gBodyStates[0]));
    gBodyStates[0].wingFlutter = o->wingFlutter;
    m->marioBodyState = &gBodyStates[0];
    memset(&sMario, 0, sizeof(sMario));
    memcpy(sMario.header.gfx.pos, o->gfxPos, sizeof(Vec3f));
    memcpy(sMario.header.gfx.angle, o->gfxAngle, sizeof(Vec3s));
    m->marioObj = &sMario;
    memset(&sArea, 0, sizeof(sArea));
    sArea.terrainType = o->areaTerrainType;
    m->area = &sArea;
    gGlobalTimer = o->globalTimer;
    gCurrLevelNum = o->levelNum;
    gWaterSurfacePseudoFloor.originOffset = o->waterPseudoOriginOffset;
    gFindFloorIncludeSurfaceIntangible = o->includeIntangible;
    gCheckingSurfaceCollisionsForCamera = FALSE;
    gMarioObject = &sMario;
    gCurrentObject = &sMario;
    gMarioState = m;
    return m;
}

static void mario_out(const struct MarioState *m, OracleMario *o) {
    o->input = m->input;
    o->flags = m->flags;
    o->action = m->action;
    o->terrainSoundAddend = m->terrainSoundAddend;
    o->particleFlags = m->particleFlags;
    o->collidedObjInteractTypes = m->collidedObjInteractTypes;
    o->intendedMag = m->intendedMag;
    o->intendedYaw = m->intendedYaw;
    o->framesSinceA = m->framesSinceA;
    o->framesSinceB = m->framesSinceB;
    o->squishTimer = m->squishTimer;
    o->wallKickTimer = m->wallKickTimer;
    o->doubleJumpTimer = m->doubleJumpTimer;
    memcpy(o->faceAngle, m->faceAngle, sizeof(Vec3s));
    memcpy(o->angleVel, m->angleVel, sizeof(Vec3s));
    memcpy(o->pos, m->pos, sizeof(Vec3f));
    memcpy(o->vel, m->vel, sizeof(Vec3f));
    o->forwardVel = m->forwardVel;
    o->slideVelX = m->slideVelX;
    o->slideVelZ = m->slideVelZ;
    o->wall = oracle_surface_index(m->wall);
    o->ceil = oracle_surface_index(m->ceil);
    o->floor = oracle_surface_index(m->floor);
    o->ceilHeight = m->ceilHeight;
    o->floorHeight = m->floorHeight;
    o->floorAngle = m->floorAngle;
    o->waterLevel = m->waterLevel;
    o->peakHeight = m->peakHeight;
    o->quicksandDepth = m->quicksandDepth;
    o->gettingBlownGravity = m->gettingBlownGravity;
    o->wingFlutter = gBodyStates[0].wingFlutter;
    memcpy(o->gfxPos, sMario.header.gfx.pos, sizeof(Vec3f));
    memcpy(o->gfxAngle, sMario.header.gfx.angle, sizeof(Vec3s));
    o->waterPseudoOriginOffset = gWaterSurfacePseudoFloor.originOffset;
    o->includeIntangible = gFindFloorIncludeSurfaceIntangible;
}

/* Run one ported function: 0 ground step, 1 air step, 2 stationary ground step,
 * 3 stop and set height, 4 bonk reflection, 5 gravity, 6 vertical wind,
 * 7 vel from pitch and yaw, 8 vel from yaw, 9 set forward vel (arg as f32 bits),
 * 10 moving sand, 11 windy ground. Returns the function's result (0 if void). */
s32 oracle_mario_call(OracleMario *o, s32 which, u32 arg) {
    struct MarioState *m = mario_in(o);
    s32 r = 0;
    f32 farg;
    memcpy(&farg, &arg, sizeof(farg));
    switch (which) {
        case 0: r = perform_ground_step(m); break;
        case 1: r = perform_air_step(m, arg); break;
        case 2: r = stationary_ground_step(m); break;
        case 3: stop_and_set_height_to_floor(m); break;
        case 4: mario_bonk_reflection(m, arg); break;
        case 5: apply_gravity(m); break;
        case 6: apply_vertical_wind(m); break;
        case 7: set_vel_from_pitch_and_yaw(m); break;
        case 8: set_vel_from_yaw(m); break;
        case 9: mario_set_forward_vel(m, farg); break;
        case 10: r = mario_update_moving_sand(m); break;
        case 11: r = mario_update_windy_ground(m); break;
        default: abort();
    }
    mario_out(m, o);
    return r;
}

/* Pre-action input oracle. Warp requests are counted from the event log. */
void adjust_analog_stick(struct Controller *controller);
void update_mario_button_inputs(struct MarioState *m);
void update_mario_joystick_inputs(struct MarioState *m);
s32 oracle_events(OracleEvent *out, s32 max);

typedef struct {
    s16 rawStick[2];
    f32 stickX, stickY, stickMag;
    u16 buttonDown, buttonPressed, sampleButtons;
    s16 cameraYaw;
    u16 cameraMovementFlags;
    u32 objectInteractStatus, objectCollidedInteractTypes;
    s32 deathWarpRequests;
} OracleInput;

/* 0 controller; 1 buttons/joystick; 2 full input stage. No actions run. */
void oracle_input_tick(OracleMario *o, OracleInput *i, s32 which) {
    struct MarioState *m = mario_in(o);
    struct Controller controller;
    OracleEvent events[16];
    s32 n, k;
    memset(&controller, 0, sizeof(controller));
    memset(&sCamera, 0, sizeof(sCamera));
    controller.rawStickX = i->rawStick[0];
    controller.rawStickY = i->rawStick[1];
    /* Same assignment expression as read_controller_inputs. */
    controller.buttonPressed = i->sampleButtons & (i->sampleButtons ^ i->buttonDown);
    controller.buttonDown = i->sampleButtons;
    adjust_analog_stick(&controller);
    m->controller = &controller;
    sCamera.yaw = i->cameraYaw;
    sArea.camera = &sCamera;
    gCameraMovementFlags = (s16) i->cameraMovementFlags;
    sMario.oInteractStatus = (s32) i->objectInteractStatus;
    sMario.collidedObjInteractTypes = i->objectCollidedInteractTypes;
    oracle_clear_events();
    switch (which) {
        case 0: break;
        case 1: update_mario_button_inputs(m); update_mario_joystick_inputs(m); break;
        case 2: update_mario_inputs(m); break;
        default: abort();
    }
    mario_out(m, o);
    i->stickX = controller.stickX;
    i->stickY = controller.stickY;
    i->stickMag = controller.stickMag;
    i->buttonDown = controller.buttonDown;
    i->buttonPressed = controller.buttonPressed;
    i->cameraMovementFlags = (u16) gCameraMovementFlags;
    n = oracle_events(events, 16);
    if (n < 0 || n > 16) {
        abort();
    }
    i->deathWarpRequests = 0;
    for (k = 0; k < n; k++) {
        if (events[k].kind != ORACLE_EVENT_WARP || events[k].a != WARP_OP_DEATH) {
            abort();
        }
        i->deathWarpRequests++;
    }
    m->controller = NULL;
    sArea.camera = NULL;
}
