import subprocess

cmd = ['ssh.exe', '-i', r'C:\Users\aphro\.ssh\id_ed25519_aphrody', '-o', 'StrictHostKeyChecking=no', 'root@72.60.186.43', 'grep -rn "dragonballfr" /etc /srv 2>/dev/null || true']
p = subprocess.run(cmd, capture_output=True, text=True, encoding='utf-8', errors='replace')
print("STDOUT:", p.stdout)
print("STDERR:", p.stderr)
