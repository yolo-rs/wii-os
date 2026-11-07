# Wii ROM-font HLE assets

These font files are copied unchanged from Dolphin's `Data/Sys/GC` fallback
font set. Dolphin documents them as Droid Sans based, generated with its
`gc-font-tool`, and licensed under Apache-2.0. The complete upstream notice and
license are retained in `font-licenses.txt`.

Gecko places the western font at EXI ROM offset `0x1FCF00` and the Japanese font
at `0x1AFF00`, matching the RVL SDK `OS_FONT_ROM_OFFSET_*` constants. This
font-only HLE is a redistributable fallback; it is not a Nintendo IPL dump and
does not reproduce the original console font exactly. A user-supplied IPL
override remains available for software that requires original firmware data.
