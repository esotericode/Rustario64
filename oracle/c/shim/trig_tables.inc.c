/* Authored shim (MIT, Rustario64). The original tables are game data, so the
 * oracle declares zeroed storage that tests fill from the owner's ROM or from
 * authored values. With AVOID_UB, gCosineTable is gSineTable + 0x400. */
f32 gSineTable[0x1400];
s16 gArctanTable[0x401];
