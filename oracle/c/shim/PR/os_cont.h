/* Authored shim (MIT, Rustario64): controller records named by types.h. The
 * oracle sets struct Controller fields directly and never reads these. */
#ifndef ORACLE_OS_CONT_H
#define ORACLE_OS_CONT_H
#include <PR/ultratypes.h>
typedef struct {
    u16 type;
    u8 status;
    u8 errnum;
} OSContStatus;
typedef struct {
    u16 button;
    s8 stick_x;
    s8 stick_y;
    u8 errnum;
} OSContPad;
/* N64 controller button bits, as the vendored game code names them. */
#define A_BUTTON 0x8000
#define B_BUTTON 0x4000
#define Z_TRIG 0x2000
#define START_BUTTON 0x1000
#define U_JPAD 0x0800
#define D_JPAD 0x0400
#define L_JPAD 0x0200
#define R_JPAD 0x0100
#define L_TRIG 0x0020
#define R_TRIG 0x0010
#define U_CBUTTONS 0x0008
#define D_CBUTTONS 0x0004
#define L_CBUTTONS 0x0002
#define R_CBUTTONS 0x0001
#endif
