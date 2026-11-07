import subprocess

remote_script = """
import re

p = '/srv/wii-os/server.mjs'
with open(p, 'r', encoding='utf-8') as f:
    c = f.read()

# Add favicon and invite routes if not present
if 'path === "/favicon.ico"' not in c:
    target = 'const path = decodeURIComponent(url.pathname);'
    addition = '''const path = decodeURIComponent(url.pathname);

    // Official Nintendo Wii Favicons
    if (path === "/favicon.ico") {
      return serveFileWithRange(req, join(WII_ROOT, "assets/favicon.ico"), { "Content-Type": "image/x-icon", "Cache-Control": "public, max-age=86400" });
    }
    if (path === "/favicon.png") {
      return serveFileWithRange(req, join(WII_ROOT, "assets/favicon.png"), { "Content-Type": "image/png", "Cache-Control": "public, max-age=86400" });
    }
    // NetPlay Room Invite & Join Endpoints
    if (path.startsWith("/v1/netplay/invite/")) {
      const roomId = path.slice("/v1/netplay/invite/".length);
      const inviteCode = "WII-9791";
      const host = req.headers.get("host") || "3dland.tech";
      return jsonResponse({
        roomId,
        inviteCode,
        inviteUrl: `https://${host}/?join=${inviteCode}`,
        shortCode: inviteCode,
        qrData: `https://${host}/?join=${inviteCode}`,
        channelId: "budokai-tenkaichi-3-eu",
        maxPlayers: 4
      });
    }
    if (path.startsWith("/join/")) {
      const code = path.slice("/join/".length);
      return Response.redirect(`/?join=${code}`, 302);
    }'''
    c = c.replace(target, addition, 1)

with open(p, 'w', encoding='utf-8') as f:
    f.write(c)

print('Updated server.mjs')
"""

cmd = ['ssh.exe', '-i', r'C:\Users\aphro\.ssh\id_ed25519_aphrody', '-o', 'StrictHostKeyChecking=no', 'root@72.60.186.43', 'python3']
p = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
stdout, stderr = p.communicate(input=remote_script.encode())
print("stdout:", stdout.decode())
print("stderr:", stderr.decode())
