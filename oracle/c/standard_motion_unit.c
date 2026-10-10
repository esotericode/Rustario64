/* Authored MIT transport, under the Oracle lock. No runtime links C.
 * Convert only host pointers and host halfword layout at the boundary. */
#include <stdlib.h>
#include <string.h>
#include "standard_motion_boundary.h"
#include "excerpts/standard_motion.c"

int oracle_surface_index(struct Surface *surface);
struct Surface *oracle_surface_ref(s32 index);

void oracle_standard_motion(const OracleStandardMotionInput *in, OracleStandardMotionOutput *out) {
    struct Object object;
    struct Object mario;
    struct Object *saved = gCurrentObject;
    struct Object *savedMario = gMarioObject;
    memset(&object, 0, sizeof(object));
    memset(out, 0, sizeof(*out));
    memcpy(object.rawData.asU32, in->raw, sizeof(in->raw));
    object.rawData.asU32[0x4E] = 0;
    object.oFloor = oracle_surface_ref((s32) in->raw[0x4E] - 1);
    object.oFloorType = in->raw[0x4C] >> 16;
    object.oFloorRoom = in->raw[0x4C] & 0xFFFF;
    object.activeFlags = in->activeFlags;
    memset(&mario, 0, sizeof(mario));
    mario.oPosX = in->marioPos[0];
    mario.oPosY = in->marioPos[1];
    mario.oPosZ = in->marioPos[2];
    gMarioObject = &mario;
    gCurrentObject = &object;
    gCheckingSurfaceCollisionsForCamera = in->forCamera;
    gFindFloorIncludeSurfaceIntangible = in->includeIntangible;
    switch (in->operation) {
        case 0: cur_obj_update_floor_and_walls(); break;
        case 1: cur_obj_move_standard(in->angle); break;
        case 2: cur_obj_move_y(in->gravity, in->bounciness, in->buoyancy); break;
        case 3: cur_obj_move_using_fvel_and_gravity(); break;
        case 4: out->result = cur_obj_resolve_wall_collisions(); break;
        case 5: cur_obj_apply_drag_xz(in->drag); break;
        case 6: out->result = oracle_surface_index(cur_obj_update_floor_height_and_get_floor()); break;
        case 7: out->result = abs_angle_diff(in->angle, in->otherAngle); break;
        case 8: cur_obj_move_after_thrown_or_dropped(in->gravity, in->bounciness); break;
        default: abort();
    }
    memcpy(out->raw, object.rawData.asU32, sizeof(out->raw));
    out->raw[0x4E] = oracle_surface_index(object.oFloor) + 1;
    out->raw[0x4C] = ((u32)(u16) object.oFloorType << 16) | (u16) object.oFloorRoom;
    out->includeAfter = gFindFloorIncludeSurfaceIntangible;
    gCurrentObject = saved;
    gMarioObject = savedMario;
}
