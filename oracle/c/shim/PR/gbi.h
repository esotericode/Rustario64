/* Authored shim (MIT, Rustario64): display-list/matrix stand-ins named by the
 * vendored headers, the Vtx fields used by the shadow comparison and the
 * _SHIFTL field-packing macro. No display list is built or interpreted. */
#ifndef ORACLE_GBI_H
#define ORACLE_GBI_H
#include <PR/ultratypes.h>
/* The SDK's field-packing macro, which data/behavior_data.c's command
 * macros are written with. */
#define _SHIFTL(v, s, w) ((unsigned int) (((unsigned int) (v) & ((0x01 << (w)) - 1)) << (s)))
typedef struct {
    u32 w0;
    u32 w1;
} Gfx;
typedef struct {
    s32 m[4][4];
} Mtx;
typedef struct {
    s16 vscale[4];
    s16 vtrans[4];
} Vp;
typedef struct {
    struct {
        s16 ob[3];
        u16 flag;
        s16 tc[2];
        u8 cn[4];
    } v;
} Vtx;
#endif
