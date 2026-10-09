/* Authored shim (MIT, Rustario64): opaque libultra message types. */
#ifndef ORACLE_OS_MESSAGE_H
#define ORACLE_OS_MESSAGE_H
#include <PR/ultratypes.h>
typedef void *OSMesg;
typedef struct OSMesgQueue_s {
    s32 validCount;
} OSMesgQueue;
#endif
