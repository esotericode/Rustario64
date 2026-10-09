/* Authored shim (MIT, Rustario64). struct Surface matches pinned include/types.h;
 * the object/Mario structs contain only the fields the vendored collision code
 * touches, so layouts are not the original's. */
#ifndef ORACLE_TYPES_H
#define ORACLE_TYPES_H
#include <PR/ultratypes.h>
typedef s16 Vec3s[3];
typedef f32 Vec3f[3];
typedef f32 Mat4[4][4];
typedef f32 Vec4f[4];
typedef s16 Vec4s[4];
typedef struct {
    s32 m[4][4];
} Mtx;
typedef s16 TerrainData;
typedef s8 RoomData;
typedef TerrainData Vec3Terrain[3];
typedef uintptr_t BehaviorScript;

/* As in the pinned include/types.h for non-IDO builds. */
#ifdef AVOID_UB
#define BAD_RETURN(cmd) void
#else
#define BAD_RETURN(cmd) cmd
#endif

struct Surface {
    TerrainData type;
    TerrainData force;
    s8 flags;
    RoomData room;
    TerrainData lowerY;
    TerrainData upperY;
    Vec3Terrain vertex1;
    Vec3Terrain vertex2;
    Vec3Terrain vertex3;
    struct {
        f32 x;
        f32 y;
        f32 z;
    } normal;
    f32 originOffset;
    struct Object *object;
};

struct GraphNode {
    s16 type;
    s16 flags;
};

struct GraphNodeObject {
    struct GraphNode node;
    s8 areaIndex;
    s8 activeAreaIndex;
    Vec3s angle;
    Vec3f pos;
    Mat4 *throwMatrix;
    Vec3f cameraToObject;
};

struct ObjectNode {
    struct GraphNodeObject gfx;
};

struct Object {
    struct ObjectNode header;
    u32 collidedObjInteractTypes;
    u32 oInteractStatus;
    s16 activeFlags;
    const BehaviorScript *behavior;
    void *collisionData;
    Mat4 transform;
    f32 oDistanceToMario;
    f32 oCollisionDistance;
    f32 oDrawingDistance;
    f32 oPosX;
    f32 oPosY;
    f32 oPosZ;
};

struct MarioBodyState {
    s8 wingFlutter;
};

struct Controller {
    s16 rawStickX, rawStickY;
    f32 stickX, stickY, stickMag;
    u16 buttonDown, buttonPressed;
};
struct Camera { s16 yaw; };
struct Area {
    struct Camera *camera;
    u16 terrainType;
};

/* Only the fields read or written by the vendored step code and helpers. */
struct MarioState {
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
    struct Controller *controller;
    Vec3s faceAngle;
    Vec3s angleVel;
    Vec3f pos;
    Vec3f vel;
    f32 forwardVel;
    f32 slideVelX;
    f32 slideVelZ;
    struct Surface *wall;
    struct Surface *ceil;
    struct Surface *floor;
    f32 ceilHeight;
    f32 floorHeight;
    s16 floorAngle;
    s16 waterLevel;
    struct Object *marioObj;
    struct Area *area;
    struct MarioBodyState *marioBodyState;
    f32 peakHeight;
    f32 quicksandDepth;
    f32 gettingBlownGravity;
};
#endif
