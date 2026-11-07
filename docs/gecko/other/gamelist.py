from pathlib import Path

ROMS_PATH = Path("\\\\VIBRATOR\\Roms\\Nintendo - GameCube")
BLACKLIST = [
    "Alpha", "Beta", "Proto", "Rev ", "Action Replay", "(v1.", "(Unl)", "Disc 2", "Disc 3", "Demo"
]

screenshots = []
for file in ROMS_PATH.glob("*.zip"):
    if "(USA)" in file.name and not any(x.lower() in file.name.lower() for x in BLACKLIST):
        screenshots.append(file)
for sc in screenshots:
    print(sc.name)