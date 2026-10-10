/* Authored (MIT, Rustario64): sMacroObjectPresets is a file-scope table of
 * macro_special_objects.c (include/macro_presets.inc.c). The tick harness
 * fills this stand-in from the ROM's table (via the Rust importer) for the
 * presets of the macro objects it spawns; the struct is the verbatim
 * excerpt. */
#ifndef ORACLE_MACRO_PRESET_BOUNDARY_H
#define ORACLE_MACRO_PRESET_BOUNDARY_H
#define ORACLE_MACRO_PRESET_COUNT 366
extern struct MacroPreset sMacroObjectPresets[ORACLE_MACRO_PRESET_COUNT];
#endif
