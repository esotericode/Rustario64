/* Rustario64 oracle (authored, MIT). Compiles the verbatim interaction.c
 * excerpt in this translation unit so the tick oracle can read and reset its
 * file-scope state, which the original keeps in static variables. */
#include "excerpts/interaction.c"

/* sDelayInvincTimer, sInvulnerable, sDisplayingDoorText, sJustTeleported,
 * sPSSSlideStarted, in that order. */
void oracle_interaction_state(s32 *out) {
    out[0] = sDelayInvincTimer;
    out[1] = sInvulnerable;
    out[2] = sDisplayingDoorText;
    out[3] = sJustTeleported;
    out[4] = sPSSSlideStarted;
}

/* The values a fresh boot starts with (static initializers and BSS). */
void oracle_reset_interaction_state(void) {
    sDelayInvincTimer = 0;
    sInvulnerable = 0;
    sDisplayingDoorText = FALSE;
    sJustTeleported = FALSE;
    sPSSSlideStarted = FALSE;
}
