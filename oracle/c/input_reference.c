/* CC0 excerpts from n64decomp/sm64 9921382a68bb0c865e5e45eb594d9c64db59b1af.
 * Verbatim function text; hashes and shim boundaries in oracle/README.md. */
#include "input_reference.h"
#include "engine/math_util.h"
#include "engine/surface_collision.h"
#include "mario.h"
#include "game/mario_step.h"

/* src/game/game_init.c: adjust_analog_stick */
void adjust_analog_stick(struct Controller *controller) {
    UNUSED u8 filler[8];

    // Reset the controller's x and y floats.
    controller->stickX = 0;
    controller->stickY = 0;

    // Modulate the rawStickX and rawStickY to be the new f32 values by adding/subtracting 6.
    if (controller->rawStickX <= -8) {
        controller->stickX = controller->rawStickX + 6;
    }

    if (controller->rawStickX >= 8) {
        controller->stickX = controller->rawStickX - 6;
    }

    if (controller->rawStickY <= -8) {
        controller->stickY = controller->rawStickY + 6;
    }

    if (controller->rawStickY >= 8) {
        controller->stickY = controller->rawStickY - 6;
    }

    // Calculate f32 magnitude from the center by vector length.
    controller->stickMag =
        sqrtf(controller->stickX * controller->stickX + controller->stickY * controller->stickY);

    // Magnitude cannot exceed 64.0f: if it does, modify the values
    // appropriately to flatten the values down to the allowed maximum value.
    if (controller->stickMag > 64) {
        controller->stickX *= 64 / controller->stickMag;
        controller->stickY *= 64 / controller->stickMag;
        controller->stickMag = 64;
    }
}

/* src/game/mario.c: mario_get_floor_class */
s32 mario_get_floor_class(struct MarioState *m) {
    s32 floorClass;

    // The slide terrain type defaults to slide slipperiness.
    // This doesn't matter too much since normally the slide terrain
    // is checked for anyways.
    if ((m->area->terrainType & TERRAIN_MASK) == TERRAIN_SLIDE) {
        floorClass = SURFACE_CLASS_VERY_SLIPPERY;
    } else {
        floorClass = SURFACE_CLASS_DEFAULT;
    }

    if (m->floor != NULL) {
        switch (m->floor->type) {
            case SURFACE_NOT_SLIPPERY:
            case SURFACE_HARD_NOT_SLIPPERY:
            case SURFACE_SWITCH:
                floorClass = SURFACE_CLASS_NOT_SLIPPERY;
                break;

            case SURFACE_SLIPPERY:
            case SURFACE_NOISE_SLIPPERY:
            case SURFACE_HARD_SLIPPERY:
            case SURFACE_NO_CAM_COL_SLIPPERY:
                floorClass = SURFACE_CLASS_SLIPPERY;
                break;

            case SURFACE_VERY_SLIPPERY:
            case SURFACE_ICE:
            case SURFACE_HARD_VERY_SLIPPERY:
            case SURFACE_NOISE_VERY_SLIPPERY_73:
            case SURFACE_NOISE_VERY_SLIPPERY_74:
            case SURFACE_NOISE_VERY_SLIPPERY:
            case SURFACE_NO_CAM_COL_VERY_SLIPPERY:
                floorClass = SURFACE_CLASS_VERY_SLIPPERY;
                break;
        }
    }

    // Crawling allows Mario to not slide on certain steeper surfaces.
    if (m->action == ACT_CRAWLING && m->floor->normal.y > 0.5f && floorClass == SURFACE_CLASS_DEFAULT) {
        floorClass = SURFACE_CLASS_NOT_SLIPPERY;
    }

    return floorClass;
}

/* src/game/mario.c: mario_floor_is_slippery */
u32 mario_floor_is_slippery(struct MarioState *m) {
    f32 normY;

    if ((m->area->terrainType & TERRAIN_MASK) == TERRAIN_SLIDE
        && m->floor->normal.y < 0.9998477f //~cos(1 deg)
    ) {
        return TRUE;
    }

    switch (mario_get_floor_class(m)) {
        case SURFACE_VERY_SLIPPERY:
            normY = 0.9848077f; //~cos(10 deg)
            break;

        case SURFACE_SLIPPERY:
            normY = 0.9396926f; //~cos(20 deg)
            break;

        default:
            normY = 0.7880108f; //~cos(38 deg)
            break;

        case SURFACE_NOT_SLIPPERY:
            normY = 0.0f;
            break;
    }

    return m->floor->normal.y <= normY;
}

/* src/game/mario.c: update_mario_button_inputs */
void update_mario_button_inputs(struct MarioState *m) {
    if (m->controller->buttonPressed & A_BUTTON) {
        m->input |= INPUT_A_PRESSED;
    }

    if (m->controller->buttonDown & A_BUTTON) {
        m->input |= INPUT_A_DOWN;
    }

    // Don't update for these buttons if squished.
    if (m->squishTimer == 0) {
        if (m->controller->buttonPressed & B_BUTTON) {
            m->input |= INPUT_B_PRESSED;
        }

        if (m->controller->buttonDown & Z_TRIG) {
            m->input |= INPUT_Z_DOWN;
        }

        if (m->controller->buttonPressed & Z_TRIG) {
            m->input |= INPUT_Z_PRESSED;
        }
    }

    if (m->input & INPUT_A_PRESSED) {
        m->framesSinceA = 0;
    } else if (m->framesSinceA < 0xFF) {
        m->framesSinceA++;
    }

    if (m->input & INPUT_B_PRESSED) {
        m->framesSinceB = 0;
    } else if (m->framesSinceB < 0xFF) {
        m->framesSinceB++;
    }
}

/* src/game/mario.c: update_mario_joystick_inputs */
void update_mario_joystick_inputs(struct MarioState *m) {
    struct Controller *controller = m->controller;
    f32 mag = ((controller->stickMag / 64.0f) * (controller->stickMag / 64.0f)) * 64.0f;

    if (m->squishTimer == 0) {
        m->intendedMag = mag / 2.0f;
    } else {
        m->intendedMag = mag / 8.0f;
    }

    if (m->intendedMag > 0.0f) {
        m->intendedYaw = atan2s(-controller->stickY, controller->stickX) + m->area->camera->yaw;
        m->input |= INPUT_NONZERO_ANALOG;
    } else {
        m->intendedYaw = m->faceAngle[1];
    }
}

/* src/game/mario.c: update_mario_geometry_inputs */
void update_mario_geometry_inputs(struct MarioState *m) {
    f32 gasLevel;
    f32 ceilToFloorDist;

    f32_find_wall_collision(&m->pos[0], &m->pos[1], &m->pos[2], 60.0f, 50.0f);
    f32_find_wall_collision(&m->pos[0], &m->pos[1], &m->pos[2], 30.0f, 24.0f);

    m->floorHeight = find_floor(m->pos[0], m->pos[1], m->pos[2], &m->floor);

    // If Mario is OOB, move his position to his graphical position (which was not updated)
    // and check for the floor there.
    // This can cause errant behavior when combined with astral projection,
    // since the graphical position was not Mario's previous location.
    if (m->floor == NULL) {
        vec3f_copy(m->pos, m->marioObj->header.gfx.pos);
        m->floorHeight = find_floor(m->pos[0], m->pos[1], m->pos[2], &m->floor);
    }

    m->ceilHeight = vec3f_find_ceil(m->pos, m->floorHeight, &m->ceil);
    gasLevel = find_poison_gas_level(m->pos[0], m->pos[2]);
    m->waterLevel = find_water_level(m->pos[0], m->pos[2]);

    if (m->floor != NULL) {
        m->floorAngle = atan2s(m->floor->normal.z, m->floor->normal.x);
        m->terrainSoundAddend = mario_get_terrain_sound_addend(m);

        if ((m->pos[1] > m->waterLevel - 40) && mario_floor_is_slippery(m)) {
            m->input |= INPUT_ABOVE_SLIDE;
        }

        if ((m->floor->flags & SURFACE_FLAG_DYNAMIC)
            || (m->ceil && m->ceil->flags & SURFACE_FLAG_DYNAMIC)) {
            ceilToFloorDist = m->ceilHeight - m->floorHeight;

            if ((0.0f <= ceilToFloorDist) && (ceilToFloorDist <= 150.0f)) {
                m->input |= INPUT_SQUISHED;
            }
        }

        if (m->pos[1] > m->floorHeight + 100.0f) {
            m->input |= INPUT_OFF_FLOOR;
        }

        if (m->pos[1] < (m->waterLevel - 10)) {
            m->input |= INPUT_IN_WATER;
        }

        if (m->pos[1] < (gasLevel - 100.0f)) {
            m->input |= INPUT_IN_POISON_GAS;
        }

    } else {
        level_trigger_warp(m, WARP_OP_DEATH);
    }
}

/* src/game/mario.c: update_mario_inputs */
void update_mario_inputs(struct MarioState *m) {
    m->particleFlags = 0;
    m->input = 0;
    m->collidedObjInteractTypes = m->marioObj->collidedObjInteractTypes;
    m->flags &= 0xFFFFFF;

    update_mario_button_inputs(m);
    update_mario_joystick_inputs(m);
    update_mario_geometry_inputs(m);

    debug_print_speed_action_normal(m);

    if (gCameraMovementFlags & CAM_MOVE_C_UP_MODE) {
        if (m->action & ACT_FLAG_ALLOW_FIRST_PERSON) {
            m->input |= INPUT_FIRST_PERSON;
        } else {
            gCameraMovementFlags &= ~CAM_MOVE_C_UP_MODE;
        }
    }

    if (!(m->input & (INPUT_NONZERO_ANALOG | INPUT_A_PRESSED))) {
        m->input |= INPUT_UNKNOWN_5;
    }

    // These 3 flags are defined by Bowser stomping attacks
    if (m->marioObj->oInteractStatus
        & (INT_STATUS_MARIO_STUNNED | INT_STATUS_MARIO_KNOCKBACK_DMG | INT_STATUS_MARIO_SHOCKWAVE)) {
        m->input |= INPUT_STOMPED;
    }

    // This function is located near other unused trampoline functions,
    // perhaps logically grouped here with the timers.
    stub_mario_step_1(m);

    if (m->wallKickTimer > 0) {
        m->wallKickTimer--;
    }

    if (m->doubleJumpTimer > 0) {
        m->doubleJumpTimer--;
    }
}

