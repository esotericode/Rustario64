/* Authored MIT fixture transport. Original C functions implement every
 * action/interaction; fixtures inject initial conditions only, then both
 * worlds evolve independently. These actors' behavior scripts do not run. */
#include <stdlib.h>
#include <string.h>
#include "grab_boundary.h"
#include "behavior_data.h"
#include "game/interaction.h"
#include "game/mario.h"
#include "game/mario_actions_automatic.h"
#include "game/mario_actions_moving.h"
#include "game/mario_actions_object.h"
#include "game/mario_actions_stationary.h"
#include "game/spawn_object.h"
#include "engine/graph_node.h"
#include "game/area.h"
#include "excerpts/grab_helpers.c"
#include "excerpts/mario_anchor.c"

static struct Object *sActor, *sAnchor, *sCarried;

void oracle_grab_fixture(const OracleGrabFixture *f) {
    struct MarioState *m = gMarioState;
    sActor = spawn_object(gMarioObject, MODEL_NONE, bhvBobomb);
    sAnchor = spawn_object(sActor, MODEL_NONE, bhvBobomb);
    sCarried = spawn_object(gMarioObject, MODEL_NONE, bhvBobomb);
    sActor->oFlags = OBJ_FLAG_HOLDABLE;
    sActor->oInteractType = INTERACT_GRABBABLE;
    sActor->oInteractionSubtype = f->subtype;
    sActor->oInteractStatus = f->objectStatus;
    sActor->oMoveAngleYaw = f->objectYaw;
    sActor->oKingBobombUnk88 = f->anchorState;
    sActor->activeFlags = f->parentActive;
    sActor->hitboxRadius = 200.0f;
    sActor->hitboxHeight = 300.0f;
    memcpy(&sActor->oPosX, f->objectPos, sizeof(Vec3f));
    memcpy(&sAnchor->oPosX, f->anchorPos, sizeof(Vec3f));
    sAnchor->oGraphYOffset = 35.0f;
    sAnchor->oMoveAnglePitch = 0x12345;
    sAnchor->oMoveAngleYaw = -0x12345;
    sAnchor->oMoveAngleRoll = 0x34567;
    sCarried->oFlags = OBJ_FLAG_HOLDABLE;
    sCarried->oHeldState = HELD_HELD;
    m->action = f->action;
    m->actionArg = f->actionArg;
    m->invincTimer = f->invincTimer;
    m->faceAngle[1] = f->marioYaw;
    m->forwardVel = f->forwardVel;
    m->vel[1] = f->velY;
    memcpy(gMarioObject->header.gfx.pos, f->gfxPos, sizeof(Vec3f));
    gMarioObject->oInteractStatus = f->marioStatus;
    m->interactObj = sActor;
    m->usedObj = sActor;
    m->heldObj = f->holding == 1 ? sCarried : f->holding == 2 ? sActor : NULL;
    if (f->holding == 2) sActor->oHeldState = HELD_HELD;
    gCurrentObject = gMarioObject;
}

s32 oracle_grab_call(const OracleGrabCall *c) {
    struct MarioState *m = gMarioState;
    s32 result = 0;
    switch (c->operation) {
        case 0:
            m->collidedObjInteractTypes = INTERACT_GRABBABLE;
            gMarioObject->numCollidedObjs = 1;
            gMarioObject->collidedObjs[0] = sActor;
            mario_process_interactions(m);
            break;
        case 1: result = mario_execute_automatic_action(m); break;
        case 2:
            gCurrentObject = sActor;
            result = cur_obj_check_grabbed_mario();
            break;
        case 3:
            gCurrentObject = sAnchor;
            common_anchor_mario_behavior(c->a, c->b, c->status);
            break;
        case 4:
            gPlayer1Controller->stickMag = c->a;
            gPlayer1Controller->buttonPressed = c->status;
            result = player_performed_grab_escape_action();
            break;
        case 5:
            gCurrentObject = sActor;
            cur_obj_get_thrown_or_placed(c->a, c->b, c->argument);
            break;
        case 6: result = mario_execute_object_action(m); break;
        case 7: result = mario_execute_stationary_action(m); break;
        case 8: result = mario_execute_moving_action(m); break;
        case 9:
            gAreaUpdateCounter++;
            if (gMarioObject->header.gfx.animInfo.curAnim != NULL) {
                struct AnimInfo *a = &gMarioObject->header.gfx.animInfo;
                a->animFrame = geo_update_animation_frame(a, &a->animFrameAccelAssist);
                a->animTimer = gAreaUpdateCounter;
            }
            break;
        case 10:
            sActor->oKingBobombUnk88 = c->argument;
            break;
        case 11:
            m->input = c->argument;
            m->intendedMag = c->a;
            m->intendedYaw = (s16) c->status;
            break;
        case 12: result = set_mario_animation(m, c->argument); break;
        default: abort();
    }
    gCurrentObject = gMarioObject;
    return result;
}
