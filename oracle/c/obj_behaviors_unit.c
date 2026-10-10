/* Rustario64 oracle (authored, MIT). Compiles the verbatim obj_behaviors.c
 * excerpt (object_step and the helpers the ported behaviors call) and the
 * behavior files obj_behaviors.c includes, in its order, in one translation
 * unit, so they share its file-scope state (sObjFloor) as the original
 * does. Development comparison tool only. */
#include "excerpts/obj_behaviors.c"
#include "excerpts/moving_coin.c"
#include "excerpts/bobomb.c"
#include "excerpts/explosion.c"
#include "excerpts/corkbox.c"
