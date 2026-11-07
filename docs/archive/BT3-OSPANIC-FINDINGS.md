# BT3 (RDSPAF) : cause du blocage `PC=0x80207058` (Ghidra, 2026-10-30)

- `0x80207050` est la boucle d'arrêt terminale du SDK (`sync; nop; li r3,0; nop; b self`), appelée par OSPanic
  `0x8020A400` (printf « in %s on line %d » + pile « Address Back Chain LR Save »), par `exit()` `0x802136B4`,
  par le gestionnaire de machine check `0x80209868`, etc.
- Le gestionnaire disque `wii_diskerror.c` (`0x8020649C`) appelle OSPanic à la ligne 0x21D (541) si l'allocation
  `0x80010520(0x120F00|0x20120|0x141020, 0x20, 0, 1)` renvoie 0 (« Ins. memory to load ROM font »), et à la
  ligne 0x221 (545) si `0x8020D980` (OSInitFont) renvoie 0 (« ROM font is available in boot ROM only »).
- À faire : capturer r3/r4/r5 à l'appel d'OSPanic (`0x8020A400`) dans Gecko : r3 = nom de fichier, r4 = ligne.
  Cela dit lequel des deux contrôles échoue (tas/arena contre police ROM de l'IPL).
- Exports : `C:\tmp\bt3\ghidra-out\{bt3_functions.jsonl,bt3_strings.jsonl,halt_callers.txt}` (local, hors dépôt).
