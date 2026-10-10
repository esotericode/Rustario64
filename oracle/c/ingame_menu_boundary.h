/* Authored drawing boundary for the ingame_menu.c excerpt; MIT, development
 * only. The excerpt's dialog functions advance the dialog box's state while
 * they build its display list. The oracle keeps the state and discards the
 * drawing: the display-list macros only advance their packet pointer (as the
 * originals do), and the drawing helpers c/ingame_menu_unit.c defines draw
 * nothing. None of them changes a value the state functions read. The
 * dialog table is the ROM's, supplied by the Rust importer. */
#ifndef ORACLE_INGAME_MENU_BOUNDARY_H
#define ORACLE_INGAME_MENU_BOUNDARY_H
#include "game/memory.h"

#define gSPDisplayList(pkt, dl) ((void) (pkt))
#define gSPPopMatrix(pkt, n) ((void) (pkt))
#define gDPSetScissor(pkt, mode, ulx, uly, lrx, lry) ((void) (pkt))
#define gDPSetEnvColor(pkt, r, g, b, a) ((void) (pkt))

/* seg2_dialog_table (segment2.h is not vendored). */
extern void *seg2_dialog_table[];

/* Drawing helpers defined in ingame_menu.c (US signatures); authored
 * stand-ins that draw nothing. */
void render_dialog_box_type(struct DialogEntry *dialog, s8 linesPerBox);
void change_and_flash_dialog_text_color_lines(s8 colorMode, s8 lineNum);
void render_generic_char(u8 c);
void render_multi_text_string_lines(s8 multiTextId, s8 lineNum, s16 *linePos, s8 linesPerBox,
                                    s8 xMatrix, s8 lowerBound);
void render_star_count_dialog_text(s8 *xMatrix, s16 *linePos);
void render_dialog_triangle_next(s8 linesPerBox);
/* Not modelled: the Peach letter and the pause and course-complete
 * screens abort. */
void print_peach_letter_message(void);
s16 render_pause_screen(void);
s16 render_course_complete_screen(void);
#endif
