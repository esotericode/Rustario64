/* Rustario64 oracle (authored, MIT). Compiles the verbatim ingame_menu.c
 * excerpt (the dialog box's state) with c/ingame_menu_boundary.h, defines
 * the drawing helpers it calls as stand-ins that draw nothing, holds the
 * ROM's dialog table as the Rust importer decoded it, and reads/resets the
 * dialog globals for the tick harness. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "excerpts/ingame_menu.c"

static void unmodelled(const char *what) {
    fprintf(stderr, "dialog oracle: %s is not modelled\n", what);
    abort();
}

void render_dialog_box_type(struct DialogEntry *dialog, s8 linesPerBox) {
    (void) dialog;
    (void) linesPerBox;
}
void change_and_flash_dialog_text_color_lines(s8 colorMode, s8 lineNum) {
    (void) colorMode;
    (void) lineNum;
}
void render_generic_char(u8 c) {
    (void) c;
}
void render_multi_text_string_lines(s8 multiTextId, s8 lineNum, s16 *linePos, s8 linesPerBox,
                                    s8 xMatrix, s8 lowerBound) {
    (void) multiTextId;
    (void) lineNum;
    (void) linePos;
    (void) linesPerBox;
    (void) xMatrix;
    (void) lowerBound;
}
void render_star_count_dialog_text(s8 *xMatrix, s16 *linePos) {
    (void) xMatrix;
    (void) linePos;
}
void render_dialog_triangle_next(s8 linesPerBox) {
    (void) linesPerBox;
}
void create_dl_translation_matrix(s8 pushOp, f32 x, f32 y, f32 z) {
    (void) pushOp;
    (void) x;
    (void) y;
    (void) z;
}
void create_dl_ortho_matrix(void) {
}
void print_peach_letter_message(void) {
    unmodelled("print_peach_letter_message");
}
s16 render_pause_screen(void) {
    unmodelled("render_pause_screen");
    return 0;
}
s16 render_course_complete_screen(void) {
    unmodelled("render_course_complete_screen");
    return 0;
}

/* The master display list's head (game_init.c). select_gfx_pool points it at
 * a fresh list each frame; the boundary macros only advance it. */
static Gfx sDialogDisplayList[4096];
Gfx *gDisplayListHead = sDialogDisplayList;

void oracle_dialog_begin_frame(void) {
    gDisplayListHead = sDialogDisplayList;
}

/* ---- The ROM's dialog table ---- */

typedef struct {
    u32 unused;
    s32 linesPerBox;
    s32 leftOffset;
    s32 width;
    const u8 *text;
    s32 textLength;
} OracleDialog;

#define ORACLE_MAX_DIALOGS 256
void *seg2_dialog_table[ORACLE_MAX_DIALOGS + 1];
static struct DialogEntry sDialogs[ORACLE_MAX_DIALOGS];
static u8 *sDialogText[ORACLE_MAX_DIALOGS];

/* Copies `count` entries; the table ends with NULL, as the original's does. */
void oracle_dialog_set_table(const OracleDialog *dialogs, s32 count) {
    s32 i;
    if (count < 0 || count > ORACLE_MAX_DIALOGS) {
        unmodelled("a dialog table of that size");
    }
    for (i = 0; i < ORACLE_MAX_DIALOGS; i++) {
        free(sDialogText[i]);
        sDialogText[i] = NULL;
        seg2_dialog_table[i] = NULL;
    }
    seg2_dialog_table[ORACLE_MAX_DIALOGS] = NULL;
    for (i = 0; i < count; i++) {
        sDialogText[i] = malloc(dialogs[i].textLength);
        memcpy(sDialogText[i], dialogs[i].text, dialogs[i].textLength);
        sDialogs[i].unused = dialogs[i].unused;
        sDialogs[i].linesPerBox = (s8) dialogs[i].linesPerBox;
        sDialogs[i].leftOffset = (s16) dialogs[i].leftOffset;
        sDialogs[i].width = (s16) dialogs[i].width;
        sDialogs[i].str = sDialogText[i];
        seg2_dialog_table[i] = &sDialogs[i];
    }
}

/* The values a fresh boot starts with (initializers and BSS). */
void oracle_dialog_reset(void) {
    gMenuTextColorTransTimer = 0;
    gLastDialogLineNum = 0;
    gDialogVariable = 0;
    gMenuState = MENU_STATE_DEFAULT;
    gDialogBoxAngle = DIALOG_BOX_ANGLE_DEFAULT;
    gDialogBoxScale = DIALOG_BOX_SCALE_DEFAULT;
    gDialogScrollOffsetY = 0;
    gDialogBoxType = DIALOG_TYPE_ROTATE;
    gDialogID = DIALOG_NONE;
    gNextDialogPageStartStrIndex = 0;
    gDialogPageStartStrIndex = 0;
    gMenuLineNum = 1;
    gDialogWithChoice = FALSE;
    gMenuHoldKeyIndex = 0;
    gMenuHoldKeyTimer = 0;
    gDialogResponse = DIALOG_RESPONSE_NONE;
    gMenuMode = MENU_MODE_NONE;
}

/* Every dialog global as a named word; src/object_trace.rs produces the same
 * names from the Rust dialog box. */
void oracle_dialog_snapshot(void (*put)(const char *name, u32 value)) {
    u32 bits;
    put("dialog.menuState", (u32) (s32) gMenuState);
    memcpy(&bits, &gDialogBoxAngle, sizeof(bits));
    put("dialog.angle", bits);
    memcpy(&bits, &gDialogBoxScale, sizeof(bits));
    put("dialog.scale", bits);
    put("dialog.scrollOffsetY", (u32) (s32) gDialogScrollOffsetY);
    put("dialog.boxType", (u32) (s32) gDialogBoxType);
    put("dialog.id", (u32) (s32) gDialogID);
    put("dialog.nextPageStart", (u32) (s32) gNextDialogPageStartStrIndex);
    put("dialog.pageStart", (u32) (s32) gDialogPageStartStrIndex);
    put("dialog.menuLineNum", (u32) (s32) gMenuLineNum);
    put("dialog.withChoice", (u32) (s32) gDialogWithChoice);
    put("dialog.holdKeyIndex", gMenuHoldKeyIndex);
    put("dialog.holdKeyTimer", gMenuHoldKeyTimer);
    put("dialog.response", (u32) gDialogResponse);
    put("dialog.lastLineNum", (u32) (s32) gLastDialogLineNum);
    put("dialog.variable", (u32) gDialogVariable);
    put("dialog.textColorTimer", gMenuTextColorTransTimer);
    put("dialog.menuMode", (u32) (s32) gMenuMode);
}
