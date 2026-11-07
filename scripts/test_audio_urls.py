import urllib.request

tracks = ['BGM_M01.wav', 'BGM_M02.wav', 'BGM_M03.wav', 'VOICE_V01.wav', 'VOICE_V02.wav', 'VOICE_V04.wav']
for t in tracks:
    u = f'https://3dland.tech/cdn/shenron/audio/{t}'
    try:
        with urllib.request.urlopen(u) as r:
            print(f'[OK] {t} -> {r.status} ({r.headers.get("content-length")} bytes)')
    except Exception as e:
        print(f'[ERR] {t} -> {e}')
