/* Authored shim (MIT, Rustario64). struct Surface matches pinned include/types.h;
 * the object/Mario structs contain only the fields the vendored collision code
 * touches, so layouts are not the original's. */
#ifndef ORACLE_TYPES_H
#define ORACLE_TYPES_H
#include <PR/ultratypes.h>
typedef s16 Vec3s[3];
typedef f32 Vec3f[3];
typedef f32 Mat4[4][4];
typedef s16 TerrainData;
typedef s8 RoomData;
typedef TerrainData Vec3Terrain[3];
typedef uintptr_t BehaviorScript;

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
    Mat4 *throwMatrix;
};

struct ObjectNode {
    struct GraphNodeObject gfx;
};

struct Object {
    struct ObjectNode header;
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

struct MarioState {
    u32 flags;
};
#endif
