/* Authored MIT harness: use the frame arena for the original callback. */
#define alloc_display_list oracle_frame_alloc_display_list
/* Drawing commands are discarded; evaluate the pointer to retain its advance. */
#define G_AC_DITHER 3
#define gDPSetAlphaCompare(pkt, mode) ((void) (pkt), (void) (mode))
#define gDPSetEnvColor(pkt, r, g, b, a) ((void) (pkt), (void) (r), (void) (g), (void) (b), (void) (a))
#define gSPEndDisplayList(pkt) ((void) (pkt))
#include "excerpts/object_transparency.c"
