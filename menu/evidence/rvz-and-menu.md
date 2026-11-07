# REA and menu evidence note

## Supplied RVZ

REA format inspection of
`C:/src/wii/wii-os/userland/data/wii/roms/Dragon-Ball-Z-Budokai-Tenkaichi-3-Europe-En-Fr-De-Es-It.rvz`
reported:

- Format/platform: RVZ, Wii/GameCube; parsed by the repository RVZ reader.
- Header: Wii, game ID `RDSPAF`, title `Dragon Ball Z Budokai Tenkaichi 3`,
  PAL Europe, two partitions, Zstandard compression.
- Logical disc size: 4,699,979,776 bytes; file size: 2,276,009,944 bytes.
- SHA-256: `d3910a98ba3c068d721e8671ceb25418d55321af5f27f5b614a1837a5ac2714f`.
- REA streamed 34,730 digest chunks. The extraction record reports matching
  Redump verification and that the original image was left unchanged.

Extraction counts, hashes, Ghidra version, function count and limitations are
in
`C:/src/wii/wii-os/userland/build/extractions/budokai-tenkaichi-3-eur-20261030T005541Z/README.md`.
The decompiled `main.dol` is game pseudocode, not original C source. The
generic REA pipeline remained partial for DOL loading; dedicated Ghidra
analysis completed with function-level warnings.

## Wii Menu recon boundary

- The checked-in frontend is a Wii-inspired browser reproduction with a
  DOM/CSS shell and WebGPU channel artwork. Source inspection is not runtime
  or physical-console evidence.
- `sources/wii-ipl` is a separate incomplete System Menu decompilation that
  requires an operator-provided WAD. It is not a browser UI implementation.
- The supplied game RVZ is not a Wii Menu capture, WAD, firmware state or
  evidence for menu behavior.
- No physical-console comparison capture is available; pixel parity is
  unmeasured.
