import os

p = r'C:\src\wii\wii-os\frontend\assets\js\main.js'
with open(p, 'r', encoding='utf-8') as f:
    c = f.read()

target = '\t// back to main menu'
replacement = """\t// start channel / game
\t$("body").on("click", ".splash-bar .btn:not(.menu-btn)", function(){
\t\tselect();
\t\tdelay(function(){
\t\t\twindow.location.href = "/games/budokai-tenkaichi-3";
\t\t}, 350);
\t});

\t// back to main menu"""

if target in c:
    c = c.replace(target, replacement, 1)
    with open(p, 'w', encoding='utf-8') as f:
        f.write(c)
    print('Updated main.js successfully')
else:
    print('Target not found')
