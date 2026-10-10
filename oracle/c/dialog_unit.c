/* Authored MIT wrapper for Mario's side of NPC dialog. It compiles the
 * verbatim mario_actions_cutscene.c excerpt (excerpts/mario_dialog.c) and
 * replaces the cutscene group's former recording stub with a partial
 * dispatcher: the verbatim check_for_instant_quicksand runs first and the two
 * ported actions are called as the original switch does, followed by its
 * water-wave tail. Every other cutscene action is still recorded as
 * unsupported and returns FALSE, matching the Rust port. */
#include "sm64.h"
#include "runtime.h"
#include "excerpts/mario_dialog.c"

s32 mario_execute_cutscene_action(struct MarioState *m) {
    s32 cancel;

    if (check_for_instant_quicksand(m)) {
        return TRUE;
    }

    switch (m->action) {
        case ACT_READING_NPC_DIALOG: cancel = act_reading_npc_dialog(m); break;
        case ACT_WAITING_FOR_DIALOG: cancel = act_waiting_for_dialog(m); break;
        default:
            oracle_event(ORACLE_EVENT_UNSUPPORTED, ORACLE_UNSUPPORTED_CUTSCENE_GROUP, (s32) m->action);
            return FALSE;
    }

    if (!cancel && (m->input & INPUT_IN_WATER)) {
        m->particleFlags |= PARTICLE_IDLE_WATER_WAVE;
    }

    return cancel;
}
