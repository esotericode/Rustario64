/* Authored shim (MIT, Rustario64). Host stand-ins for the libultra types the
 * vendored decomp headers name. The decomp's include/PR SDK-derived headers
 * are deliberately not vendored; nothing here is used by the compared code
 * beyond declarations. */
#ifndef ORACLE_ULTRA64_H
#define ORACLE_ULTRA64_H
#include <math.h>
#include <PR/ultratypes.h>
#include <PR/os_message.h>
#include <PR/os_cont.h>
#include <PR/gbi.h>
typedef struct {
    u32 type;
} OSTask;
typedef struct {
    u32 unused;
} OSThread;
typedef struct {
    u32 unused;
} OSIoMesg;
typedef struct {
    u32 unused;
} OSTime;
/* Called by math_util.c's mtxf_to_mtx; only display lists use the result. */
void guMtxF2L(float mf[4][4], Mtx *m);
#endif
