# The demo on writui.com's landing page. From the repo root, after
# `cargo build`:
#
#   vhs site/demo.tape
#   ffmpeg -y -sseof -0.2 -i site/static/demo.mp4 -frames:v 1 site/static/demo.png
#
# It drives the debug build against a throwaway vault in /tmp.

Output "site/static/demo.mp4"

Set Shell "bash"
Set FontSize 22
Set Width 1200
Set Height 680
Set Padding 20
Set Framerate 30
Set TypingSpeed 45ms
Set Theme { "name": "writui", "background": "#111113", "foreground": "#e4e4e7", "cursor": "#e4e4e7", "selection": "#3a3a40", "black": "#18181b", "red": "#f87171", "green": "#86d39b", "yellow": "#facc15", "blue": "#7aa2f7", "magenta": "#c084fc", "cyan": "#67d4e8", "white": "#e4e4e7", "brightBlack": "#6b6b73", "brightRed": "#fca5a5", "brightGreen": "#a7e3b6", "brightYellow": "#fde68a", "brightBlue": "#a5c0fb", "brightMagenta": "#d8b4fe", "brightCyan": "#a5e8f3", "brightWhite": "#ffffff" }

Hide
Type "rm -rf /tmp/writui-demo && mkdir /tmp/writui-demo && clear && target/debug/writui --db /tmp/writui-demo/writui.db"
Enter
Sleep 1.5s
Type@5ms "writui-dev"
Tab
Type@5ms "writui-dev"
Enter
Sleep 1s
Ctrl+N
Sleep 300ms
Type@5ms "Notes on the new garden"
Enter
Enter
Type@5ms "The tomatoes came in early this year."
Ctrl+Q
Sleep 1s
Type "clear && target/debug/writui --db /tmp/writui-demo/writui.db"
Enter
Sleep 1.5s
Type@5ms "writui-dev"
Enter
Sleep 1s
Show

Sleep 1.2s
Ctrl+N
Sleep 600ms
Type "Writing in the terminal"
Enter
Enter
Type "I write every morning, and lately I do it in a **terminal**. "
Type "It's *quiet*: one column of text, nothing else."
Enter
Enter
Type "- Plain `markdown`, formatted as I type"
Enter
Type "- Encrypted with my password"
Enter
Type "- Versions whenever I want them"
Enter
Enter
Type "> The best tool is the one that gets out of the way."
Sleep 1.5s
Ctrl+K
Sleep 1s
Type@120ms "exp"
Sleep 1.2s
Escape
Sleep 2.5s
