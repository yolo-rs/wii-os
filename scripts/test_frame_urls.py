import urllib.request

frames = ['frame_0060.png', 'frame_0300.png', 'frame_0750.png', 'frame_1110.png', 'frame_2070.png', 'frame_2610.png', 'frame_2850.png', 'frame_3150.png', 'frame_3600.png']
for f in frames:
    u = f'https://3dland.tech/cdn/shenron/frames/{f}'
    try:
        with urllib.request.urlopen(u) as r:
            print(f'[OK] {f} -> {r.status} ({r.headers.get("content-length")} bytes)')
    except Exception as e:
        print(f'[ERR] {f} -> {e}')
