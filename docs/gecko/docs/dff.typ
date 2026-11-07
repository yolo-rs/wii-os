#set document(title: "DFF File Format", author: "Layle")
#set page(
  paper: "a4",
  margin: (x: 2.2cm, y: 2.4cm),
  footer: context [
    #set text(8pt, fill: luma(120))
    DFF v6 #h(1fr) © 2026 Layle #h(1fr) #counter(page).display("1 / 1", both: true)
  ],
)
#set text(font: "New Computer Modern", size: 10pt)
#set heading(numbering: "1.1")
#show heading: set text(weight: "bold")
#show raw.where(block: true): it => block(
  fill: luma(248),
  stroke: luma(220) + 0.5pt,
  inset: 8pt,
  radius: 3pt,
  width: 100%,
  it,
)
#show raw.where(block: false): set text(fill: rgb("#1f4e79"))

// ---- byte-layout diagram -------------------------------------------------

#let c-off = rgb("#dbe9f6") // u64 file offsets
#let c-pad = luma(244) // padding / reserved

#let hex(n) = {
  let s = upper(str(n, base: 16))
  while s.len() < 2 { s = "0" + s }
  raw("0x" + s)
}

// fields: array of (name, size) or (name, size, fill)
#let bytestruct(bpr: 8, fields) = {
  let cells = ()

  // column header
  cells.push(grid.cell(stroke: none)[])
  for i in range(bpr) {
    cells.push(grid.cell(stroke: none, text(6.5pt, fill: luma(130), raw("+" + str(i)))))
  }

  let pos = 0
  for f in fields {
    let (name, size) = (f.at(0), f.at(1))
    let fill = if f.len() > 2 { f.at(2) } else { white }
    let body = text(8pt, raw(name))

    // full rows from column 0: one cell, possibly spanning several rows
    if calc.rem(pos, bpr) == 0 and calc.rem(size, bpr) == 0 {
      let rows = calc.div-euclid(size, bpr)
      cells.push(grid.cell(rowspan: rows, stroke: none, align: right + horizon, text(7pt, hex(pos))))
      cells.push(grid.cell(colspan: bpr, rowspan: rows, fill: fill, body))
      pos += size
      continue
    }

    let remaining = size
    let first = true
    while remaining > 0 {
      if calc.rem(pos, bpr) == 0 {
        cells.push(grid.cell(stroke: none, align: right + horizon, text(7pt, hex(pos))))
      }
      let take = calc.min(remaining, bpr - calc.rem(pos, bpr))
      cells.push(grid.cell(colspan: take, fill: fill, if first { body } else { sym.dots.h }))
      pos += take
      remaining -= take
      first = false
    }
  }

  block(
    breakable: false,
    grid(
      columns: (3em,) + (1fr,) * bpr,
      align: center + horizon,
      inset: 4.5pt,
      stroke: 0.6pt + luma(60),
      ..cells,
    ),
  )
}

#let field-table(..rows) = table(
  columns: (auto, auto, auto, 1fr),
  align: (right, right, left, left),
  stroke: 0.5pt + luma(200),
  fill: (_, y) => if y == 0 { luma(240) },
  table.header[*Offset*][*Size*][*Field*][*Description*],
  ..rows,
)

// ---- document ------------------------------------------------------------

#align(center)[
  #text(20pt, weight: "bold")[DFF File Format]
  #v(2pt)
  #text(11pt)[Dolphin FIFO log, version 6]
  #v(2pt)
  #text(9pt, fill: luma(100))[Reference implementation: `crates/dff/src/lib.rs`]
]
#v(8pt)

= Overview

A `.dff` file is a recording of the GameCube/Wii GPU command stream. It
starts with a snapshot of the GPU state at the moment recording began (BP,
CP and XF registers plus TMEM), followed by the raw FIFO bytes of each
frame and the guest RAM the FIFO reads. That is enough to replay the
frames without running any CPU emulation.

All integers are little-endian. The structs are packed, so the `u64`
offsets sit at 4-byte boundaries instead of natural alignment. Offsets are
absolute file offsets. Always take section positions from the header, not
from the layout below.

#figure(
  table(
    columns: (auto, 1fr),
    align: (left, left),
    stroke: 0.5pt + luma(200),
    fill: (_, y) => if y == 0 { luma(240) },
    table.header[*Section*][*Contents*],
    [Header (128 B)], [magic, version, section offsets, flags, game ID],
    [Frame table], [`frameCount` × 64 B frame descriptors],
    [`bpMem`], [256 × `u32`, the BP raster-state registers (TEV, PE, texture units)],
    [`cpMem`], [256 × `u32`, the CP (command processor) registers],
    [`xfMem`], [4096 × `u32`, XF (transform unit) memory],
    [`xfRegs`], [88 × `u32`, the XF registers (xfmem `0x1000` to `0x1057`)],
    [`texMem`], [1 MiB TMEM snapshot],
    [Frame payloads], [per frame: FIFO bytes, memory-update table, update payloads],
  ),
  caption: [File layout as emitted by the gecko writer. Readers must follow header offsets.],
)

= Header

128 bytes at offset 0.

#bytestruct(bpr: 12, (
  ("fileId", 4), ("fileVersion", 4), ("minLoaderVersion", 4),
  ("bpMemOffset", 8, c-off), ("bpMemSize", 4),
  ("cpMemOffset", 8, c-off), ("cpMemSize", 4),
  ("xfMemOffset", 8, c-off), ("xfMemSize", 4),
  ("xfRegsOffset", 8, c-off), ("xfRegsSize", 4),
  ("frameListOffset", 8, c-off), ("frameCount", 4),
  ("flags", 4), ("texMemOffset", 8, c-off),
  ("texMemSize", 4), ("mem1Size", 4), ("mem2Size", 4),
  ("gameId", 8), ("padding", 24, c-pad),
))

#field-table(
  [`0x00`], [4], [`fileId`], [Magic `0x0D01F1F0`],
  [`0x04`], [4], [`fileVersion`], [Currently 6],
  [`0x08`], [4], [`minLoaderVersion`], [Reject the file if this exceeds the loader's supported version],
  [`0x0C`], [8], [`bpMemOffset`], [Absolute offset of BP register block],
  [`0x14`], [4], [`bpMemSize`], [Element count in `u32` units (256)],
  [`0x18`], [8], [`cpMemOffset`], [CP register block],
  [`0x20`], [4], [`cpMemSize`], [256],
  [`0x24`], [8], [`xfMemOffset`], [XF memory block],
  [`0x2C`], [4], [`xfMemSize`], [4096],
  [`0x30`], [8], [`xfRegsOffset`], [XF register block],
  [`0x38`], [4], [`xfRegsSize`], [88],
  [`0x3C`], [8], [`frameListOffset`], [Frame descriptor table],
  [`0x44`], [4], [`frameCount`], [Number of frame descriptors],
  [`0x48`], [4], [`flags`], [Bit 0: `IS_WII`],
  [`0x4C`], [8], [`texMemOffset`], [TMEM snapshot _(v4+)_],
  [`0x54`], [4], [`texMemSize`], [Byte count, `0x100000` _(v4+)_],
  [`0x58`], [4], [`mem1Size`], [Guest MEM1 size, retail `0x01800000` _(v5+)_],
  [`0x5C`], [4], [`mem2Size`], [Guest MEM2 size, retail `0x04000000` _(v5+)_],
  [`0x60`], [8], [`gameId`], [ASCII, NUL-padded; `"00000000"` if unknown _(v6+)_],
)

Register block sizes count 32-bit words, `texMemSize` counts bytes. When
reading an older file, zero-fill whatever it does not store and fall back
to the retail memory sizes (before v5) and the default game ID (before
v6).

= Frame descriptor

One 64-byte entry per frame at `frameListOffset`.

#bytestruct(bpr: 16, (
  ("fifoDataOffset", 8, c-off), ("fifoDataSize", 4), ("fifoStart", 4),
  ("fifoEnd", 4), ("memoryUpdatesOffset", 8, c-off), ("numMemoryUpdates", 4),
  ("reserved", 32, c-pad),
))

#field-table(
  [`0x00`], [8], [`fifoDataOffset`], [Absolute offset of this frame's raw FIFO bytes],
  [`0x08`], [4], [`fifoDataSize`], [FIFO byte count],
  [`0x0C`], [4], [`fifoStart`], [Guest physical address of the CP FIFO ring base at record time],
  [`0x10`], [4], [`fifoEnd`], [Guest physical address of the CP FIFO ring end, has to be above `fifoStart`],
  [`0x14`], [8], [`memoryUpdatesOffset`], [Absolute offset of the memory-update table],
  [`0x1C`], [4], [`numMemoryUpdates`], [Entries in that table],
)

`fifoData` is the GX command stream for one frame, with one twist: display
lists are flattened. `CALL_DL` (`0x40`) never shows up, the contents of
the list are recorded inline at the call site. A frame ends right at the
end of the EFB copy command (BP `0x52`) that presents it, with no trailing
bytes. Dolphin's frame analyzer asserts this. Feeding the whole buffer
produces exactly one presented frame. `fifoStart` and `fifoEnd` are not
just informational. Dolphin programs the emulated CP ring from them and
derives the watermarks from their distance, so they need plausible values.

= Memory update

The FIFO references guest RAM by physical address (textures, vertex
arrays, display lists, XF loads). Each frame comes with a table of RAM
writes that have to land before the FIFO consumes them. One entry is 24
bytes:

#bytestruct(bpr: 8, (
  ("fifoPosition", 4), ("address", 4),
  ("dataOffset", 8, c-off),
  ("dataSize", 4), ("type", 1), ("padding", 3, c-pad),
))

#field-table(
  [`0x00`], [4], [`fifoPosition`], [Byte offset into this frame's `fifoData`. Apply the write before feeding bytes at or past this offset],
  [`0x04`], [4], [`address`], [Guest physical destination address],
  [`0x08`], [8], [`dataOffset`], [Absolute offset of the payload in the file],
  [`0x10`], [4], [`dataSize`], [Payload byte count],
  [`0x14`], [1], [`type`], [See below],
)

#grid(
  columns: (1fr, 1.4fr),
  gutter: 12pt,
  table(
    columns: (auto, 1fr),
    stroke: 0.5pt + luma(200),
    fill: (_, y) => if y == 0 { luma(240) },
    table.header[*Value*][*Type*],
    [`0x01`], [`TEXTURE_MAP`],
    [`0x02`], [`XF_DATA`],
    [`0x04`], [`VERTEX_STREAM`],
    [`0x08`], [`TMEM`],
  ),
  [
    The type says what the data feeds, nothing more. Every kind is applied
    the same way: copy `data` to `address` in guest RAM. A replayer can
    use `TEXTURE_MAP` to invalidate texture caches that overlap the write.
    gecko decodes unknown values as `TEXTURE_MAP`.
  ],
)

Entries have to be sorted by `fifoPosition`. Dolphin walks the table in
order and never sorts it. The gecko reader sorts on load anyway, just in
case. Writers deduplicate by diffing every referenced range against a
shadow copy of guest RAM and only emit an update when the bytes actually
changed. A static texture shows up once in the whole file, not once per
frame.

= Replay

+ Validate `fileId` and `minLoaderVersion`.
+ Load `bpMem`, `cpMem`, `xfMem` and `xfRegs` into the GPU state and
  `texMem` into TMEM. This stands in for every register write the game did
  before the recording started. When replaying `bpMem` as register writes,
  skip the registers with side effects: PE finish (`0x45`), the PE tokens
  (`0x47`, `0x48`), the EFB copy trigger (`0x52`), the TLUT DMA trigger
  (`0x65`) and the write mask (`0xFE`). Dolphin and gecko both do this.
+ For each frame, interleave updates and FIFO data:

```
pos = 0
for u in sort_by(updates, fifo_position):
    p = min(u.fifo_position, len(fifo_data))
    feed(fifo_data[pos..p])          # GX command stream
    pos = p
    ram[u.address .. u.address+len(u.data)] = u.data
feed(fifo_data[pos..])
present()                            # frame ends at its XFB copy
```

State carries across frames. A frame assumes the register and RAM state
left behind by the frames before it, so frames cannot be skipped, only
played from the start up to a target.

= Versions

#table(
  columns: (auto, 1fr),
  stroke: 0.5pt + luma(200),
  fill: (_, y) => if y == 0 { luma(240) },
  table.header[*Version*][*Change*],
  [1], [Base format: header, register blocks, frames, memory updates],
  [2], [EFB copy recording fixed. Dolphin treats v1 files as having broken EFB copies],
  [4], [`texMem` snapshot added],
  [5], [`mem1Size` / `mem2Size` added],
  [6], [`gameId` added],
)

`minLoaderVersion` decides forward compatibility, not `fileVersion`. A
writer that only appends optional data keeps `minLoaderVersion` low so old
loaders still accept the file.
