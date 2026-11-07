import subprocess

remote_script = """
import re

p = '/srv/wii-os/frontend/index.html'
with open(p, 'r', encoding='utf-8') as f:
    c = f.read()

c = c.replace('<title>Wii.JS</title>', '<title>Menu Wii</title>\\n    <link rel="icon" type="image/x-icon" href="/favicon.ico">\\n    <link rel="icon" type="image/png" sizes="32x32" href="/favicon.png">')

with open(p, 'w', encoding='utf-8') as f:
    f.write(c)

print('Updated frontend/index.html')
"""

cmd = ['ssh.exe', '-i', r'C:\Users\aphro\.ssh\id_ed25519_aphrody', '-o', 'StrictHostKeyChecking=no', 'root@72.60.186.43', 'python3']
p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
stdout, stderr = p.communicate(input=remote_script.encode())
print("stdout:", stdout.decode())
print("stderr:", stderr.decode())
