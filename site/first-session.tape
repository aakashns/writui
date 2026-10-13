# Screenshots for the first writing session guide. From the repo root:
#   bash site/record-first-session.sh
# Raw screenshots and a review GIF go to /tmp/writui-first-session-captures.
# This uses the debug app, synthetic writing, and a new throwaway vault.
# The public guide itself uses the normal installed app and a personal password.

Output "/tmp/writui-first-session-captures/flow.gif"

Set Shell "bash"
Set FontSize 22
Set Width 1200
Set Height 760
Set Padding 20
Set Framerate 10
Set TypingSpeed 5ms
Set Theme { "name": "writui", "background": "#111113", "foreground": "#e4e4e7", "cursor": "#e4e4e7", "selection": "#3a3a40", "black": "#18181b", "red": "#f87171", "green": "#86d39b", "yellow": "#facc15", "blue": "#7aa2f7", "magenta": "#c084fc", "cyan": "#67d4e8", "white": "#e4e4e7", "brightBlack": "#6b6b73", "brightRed": "#fca5a5", "brightGreen": "#a7e3b6", "brightYellow": "#fde68a", "brightBlue": "#a5c0fb", "brightMagenta": "#d8b4fe", "brightCyan": "#a5e8f3", "brightWhite": "#ffffff" }

Hide
Type "writui_guide_dir=$(mktemp -d /tmp/writui-guide.XXXXXX)"
Enter
Type 'target/debug/writui --db "$writui_guide_dir/writui.db"'
Enter
Sleep 1.5s
Show
Screenshot "/tmp/writui-first-session-captures/create-vault.png"
Sleep 1s
Type "writui-dev"
Tab
Type "writui-dev"
Enter
Sleep 1s
Screenshot "/tmp/writui-first-session-captures/new-post.png"
Sleep 1s
Type "n"
Sleep 300ms
Type "A little room to think"
Enter
Enter
Type "Today I want to make a little room for **writing**."
Enter
Enter
Type "## Things worth noticing"
Enter
Enter
Type "- A good conversation"
Enter
Type "- An idea for tomorrow"
Enter
Type "- A sentence I want to keep"
Enter
Enter
Type "One quiet paragraph is a good start."
Sleep 800ms
Screenshot "/tmp/writui-first-session-captures/writing.png"
Sleep 1s
Ctrl+K
Sleep 500ms
Screenshot "/tmp/writui-first-session-captures/menu.png"
Sleep 1s
Type "save"
Enter
Sleep 300ms
Type "First thoughts"
Sleep 300ms
Screenshot "/tmp/writui-first-session-captures/save-version.png"
Sleep 1s
Enter
Sleep 500ms
Ctrl+R
Sleep 500ms
Screenshot "/tmp/writui-first-session-captures/history.png"
Sleep 1s
Escape
Sleep 300ms
Escape
Sleep 500ms
Screenshot "/tmp/writui-first-session-captures/posts.png"
Sleep 1s
Enter
Sleep 500ms
Ctrl+O
Sleep 500ms
Screenshot "/tmp/writui-first-session-captures/unlock.png"
Sleep 1s
Type "writui-dev"
Enter
Sleep 1s
Screenshot "/tmp/writui-first-session-captures/reopened.png"
Sleep 1s
Hide
Ctrl+Q
Sleep 500ms
