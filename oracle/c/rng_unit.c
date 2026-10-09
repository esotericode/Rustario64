/* Authored accessors for the verbatim behavior_script.c RNG excerpt; MIT,
 * development only. gRandomSeed16 is file-scope in the original. */
#include "excerpts/behavior_script.c"

u16 oracle_rng_seed(void) {
    return gRandomSeed16;
}

void oracle_set_rng_seed(u16 seed) {
    gRandomSeed16 = seed;
}

u16 oracle_random_u16(void) {
    return random_u16();
}

f32 oracle_random_float(void) {
    return random_float();
}
