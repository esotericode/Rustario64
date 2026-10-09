/* Authored shim (MIT, Rustario64): display-list/matrix stand-ins named by the
 * vendored headers and the Vtx fields used by the shadow comparison. No
 * display list is built or interpreted. */
#ifndef ORACLE_GBI_H
#define ORACLE_GBI_H
#include <PR/ultratypes.h>
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
