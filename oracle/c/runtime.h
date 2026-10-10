/* Authored oracle runtime declarations (MIT, Rustario64). */
#ifndef ORACLE_RUNTIME_H
#define ORACLE_RUNTIME_H
#include "sm64.h"

/* Boundary events: calls into systems that are not part of the compared
 * simulation. Each is recorded in call order; none changes compared state. */
enum OracleEventKind {
    ORACLE_EVENT_SOUND = 1,             /* a = sound bits */
    ORACLE_EVENT_STOP_SOUND = 2,        /* a = sound bits */
    ORACLE_EVENT_MOVING_SPEED = 3,      /* a = bank, b = speed */
    ORACLE_EVENT_RAISE_NOISE = 4,       /* a = argument */
    ORACLE_EVENT_LOWER_NOISE = 5,       /* a = argument */
    ORACLE_EVENT_STOP_CAP_MUSIC = 6,
    ORACLE_EVENT_FADEOUT_CAP_MUSIC = 7,
    ORACLE_EVENT_CAMERA_MODE = 8,       /* a = mode, b = frames */
    ORACLE_EVENT_CAMERA_SHAKE = 9,      /* a = shake */
    ORACLE_EVENT_WARP = 10,             /* a = warp operation */
    ORACLE_EVENT_LEVEL_INIT_TEXT = 11,  /* a = argument */
    ORACLE_EVENT_WIND_PARTICLES = 12,   /* a = type, b = angle */
    ORACLE_EVENT_UNSUPPORTED = 13,      /* a = reason, b = detail */
};

/* ORACLE_EVENT_UNSUPPORTED reasons. */
enum OracleUnsupported {
    ORACLE_UNSUPPORTED_CUTSCENE_GROUP = 1, /* b = action */
    ORACLE_UNSUPPORTED_SUBMERGED_GROUP = 2,
    ORACLE_UNSUPPORTED_INFINITE_STAIRS = 3,
    ORACLE_UNSUPPORTED_HUNDRED_COIN_STAR = 4,
};

typedef struct {
    s32 kind;
    s32 a;
    s32 b;
} OracleEvent;

void oracle_event(s32 kind, s32 a, s32 b);
void oracle_clear_events(void);

/* The original camera.c set_camera_mode and set_camera_shake_from_hit,
 * compiled under these names by camera_unit.c. */
struct Camera;
void oracle_camera_native_set_mode(struct Camera *c, s16 mode, s16 frames);
void oracle_camera_native_hit(s16 shake);
/* Nonzero while the complete camera runs with the tick harness. */
extern s32 gOracleCameraLinked;
extern s16 gOracleHudCameraStatus;
#endif
