# writui

A writing app for the terminal. No nonsense. Markdown. Local. Encrypted.

- **Private.** All your posts live in one file, encrypted with your password.
  Without the password nobody can read them.
- **Plain markdown.** You write markdown. Each post starts with a `# Title`.
  It stays plain markdown on screen, formatted as you type: headings and
  bold in bold, italics in italics, code in green, with the markdown
  symbols faded.
- **Mouse and keyboard.** Click, scroll and press buttons with the mouse, or
  use shortcuts. The bar at the bottom of the screen shows the shortcuts
  for where you are, and you can click those too. While writing, Ctrl+K
  opens a menu of everything you can do, with each shortcut.
- **Fits your terminal.** It uses your terminal's own colours and font, and
  works in any modern terminal (Alacritty, Ghostty, iTerm2, and others).

> **Early days.** writui is being built in the open, a small piece at a time
> ([what's planned](docs/PLAN.md)). It's usable for writing now, but keep a
> copy of anything you can't afford to lose. And remember: **there is no way
> to recover a forgotten password.**

## Install

On macOS or Linux, paste this into a terminal:

```sh
curl -fsSL https://writui.com/install.sh | bash
```

It downloads the latest writui for your computer, checks it, and puts it in
`~/.local/bin`. If that folder isn't on your `PATH` yet, it tells you the line
to add. ([Read the script](install.sh) first if you like.) Check it worked
with `writui --version`.

<details>
<summary>Or download it yourself</summary>

The install script is also attached to every release:
`curl -fsSL https://github.com/aakashns/writui/releases/latest/download/install.sh | bash`.

Every [release](https://github.com/aakashns/writui/releases/latest) has a
ready-to-run file for each computer:

| Computer                       | File                  |
| ------------------------------ | --------------------- |
| Mac with Apple Silicon (M1+)   | `writui-macos-arm64`  |
| Mac with Intel                 | `writui-macos-x86_64` |
| Linux (most PCs)               | `writui-linux-x86_64` |
| Linux (ARM, e.g. Raspberry Pi) | `writui-linux-arm64`  |

Save it as `writui` in a folder on your `PATH` and make it executable
(`chmod +x writui`). The Mac files aren't signed: if you downloaded with a
browser and macOS won't open it, run `xattr -d com.apple.quarantine writui`
once. (The install script avoids this.)

</details>

### Upgrade

```sh
writui upgrade
```

This checks for a newer version, shows what's new, and asks before replacing
writui with it (`writui upgrade --yes` skips the question). If the new
version stores your writing differently, it updates your vault the next time
you unlock it: it backs the vault up first, checks the result, and only then
removes the backup. Running the install script again also upgrades.

### Windows

There's no ready-made Windows version, but you can build writui yourself:

1. Install [Rust](https://rustup.rs), the
   [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   (the "Desktop development with C++" workload), and
   [Strawberry Perl](https://strawberryperl.com) (needed to build the
   encryption library).
2. In a new PowerShell window (not Git Bash, whose Perl can't build it), run:

   ```sh
   cargo install --locked --git https://github.com/aakashns/writui
   ```

The build takes a few minutes. To upgrade, run the same command again.

writui isn't tested on Windows automatically, or tried out there by hand
regularly, so please
[report anything odd](https://github.com/aakashns/writui/issues). To run its
automated tests and checks yourself, in the same kind of PowerShell window:

```sh
git clone https://github.com/aakashns/writui
cd writui
cargo test
cargo clippy --all-targets -- -D warnings
```

If both finish without errors, writui's tests pass on your computer.

## Use

Open a terminal and run:

```sh
writui
```

The first time, writui asks you to choose a password (twice) and creates your
vault: the encrypted file that holds all your posts. After that, it asks for
the password each time it starts.

- **Posts** are listed with the most recently changed first. Open one with
  Enter or a click, start a new one with `n` (or Enter: "New" is
  highlighted when the list opens). The list's shortcuts are single keys,
  shown next to "New" and "Import" and at the bottom. Esc asks before quitting. writui opens
  where you left it: on the post you were writing, cursor and all.
- **Search:** press `/` (or click "Type to filter…") and type. Posts with
  every word you type, anywhere in their text or front matter, are listed,
  and the first is highlighted: Enter opens it, ↑/↓ pick another. Esc
  clears the search.
- **Sort** the list by when posts were updated or created, or by title,
  either way round: click "Sort by" on the right of the search, or press
  Tab, and pick one.
- **Lock** with Ctrl+O, wherever you are: writui asks for the password
  again, then takes you back to where you were.
- **Zen writing:** the editor shows only your words. Hold Ctrl to see the
  post's word count and when you started it. (This needs a terminal that can
  report Ctrl on its own, like Alacritty, Ghostty, kitty, WezTerm or a recent
  iTerm2; in others, and inside tmux, they're always shown.)
- **The menu** (Ctrl+K, or Cmd+K on a Mac) lists everything you can do
  while writing, with its shortcut. Type to filter it, pick with the arrow keys and Enter, or click.
- **Writing:** type away. Your post is saved automatically as you go: a
  second after you stop typing, every few seconds while you keep typing,
  and when you go back to the list (Esc) or quit (Ctrl+Q). There's no need
  to save. When you reopen a post, the cursor is where you left it. The
  terminal window's title shows the post's title.
- **Versions** are snapshots you make on purpose, to read later or go back
  to. Ctrl+S asks for a name (like a commit message) and keeps a copy of the
  whole post as it is now.
- **History** (Ctrl+R) lists a post's versions. Open one to read it, and
  press Enter to restore it into your post (Ctrl+Z undoes that).
- **Undo** with Ctrl+Z and **redo** with Ctrl+Y (or Ctrl+Shift+Z, in
  terminals that tell it apart from Ctrl+Z). Undo goes back a word at a
  time while typing.
- **Selecting:** drag with the mouse, double-click a word, triple-click a
  paragraph, or hold Shift while moving (Ctrl+A selects everything).
  Typing or Backspace replaces what's selected. Ctrl+C copies, Ctrl+X cuts
  and Ctrl+V pastes, using the system clipboard (your terminal's own paste,
  like Cmd+V, works too). Right-click for a menu with Cut, Copy and Paste.
  Esc drops the selection. Copying a whole post and pasting it into an empty
  one gives the same post, not a doubled `# `.
- **Formatting** follows standard markdown (CommonMark, plus GitHub's
  `~~strikethrough~~`, task lists and tables), so text is formatted only
  when it really will be: `**bold` stays plain until you close it. Wrapped
  list items and quotes line up under their text.
- **Links:** Ctrl+click a link to open it in your browser (or put the
  cursor on it and pick "Open link" from the menu). That works for `[text](https://…)` links and for web
  addresses written out in the text. Only web and email links open.
- **Moving by line, word and paragraph:** Ctrl+Left/Right (or Cmd+Left/Right
  on a Mac, where the terminal passes Cmd on, as Alacritty does) go to the
  start / end of the line, like Home and End. Option+Left/Right (Alt on Linux
  and Windows) jump by word, Option+Up/Down (Ctrl+Up/Down) by paragraph. Add
  Shift to select as you go.
- **Getting your writing out:** Ctrl+Shift+C copies the whole post as
  markdown, and Ctrl+Shift+S exports it to a `.md` file wherever you choose
  (both are in the menu too, for terminals where Ctrl+Shift doesn't come
  through). Exports start with front matter (title, date, description, …)
  for site generators like Zola, Hugo and Jekyll, which you can edit or turn
  off. Tab completes the path, and the next export of the post goes to the
  same file, asking before it replaces it.
  The first export of a post starts in your Documents folder (or wherever
  you last exported or imported).
- **Bringing writing in:** "Import" on the list (`i`) makes a post of a
  `.md` or `.txt` file, keeping its front matter and dated when the file
  was made. Export it again and it goes back to that file, so you can edit
  a published post here.
- **Deleting** (`d`) moves a post to the Trash, where you can restore it
  for 30 days.
- **Settings** (`s` on the list): your name, added as the
  `author` in exported front matter, and how wide the text is while
  writing (68 characters unless you change it).

### Cmd keys on a Mac

In the editor, Cmd works like Ctrl (Cmd+Z undoes, Cmd+X cuts, Cmd+A
selects all, Cmd+K opens the menu) wherever your terminal passes Cmd on to
programs. Cmd+V is your terminal's paste, which writui understands.

Cmd+C is different: terminals keep it to copy *their own* selection, and
writui draws its own selection, so there's nothing for the terminal to
copy. Ctrl+C always works. To make Cmd+C copy in writui too, tell your
terminal to pass it on:

- **Alacritty**, in `~/.config/alacritty/alacritty.toml`:

  ```toml
  [[keyboard.bindings]]
  key = "C"
  mods = "Command"
  action = "ReceiveChar"
  ```

- **Ghostty**, in its config (it still copies the terminal's own selection
  when there is one):

  ```
  keybind = performable:super+c=copy_to_clipboard
  ```

- **iTerm2** (not tried yet): Settings → Keys → Key Bindings, add Cmd+C
  with the action "Send Escape Sequence" and `[99;9u`.

Cmd+K needs nothing in Alacritty (it sends Ctrl+L, which also opens the
menu). In Ghostty and iTerm2 it clears the screen unless you pass it on
the same way (`keybind = super+k=unbind` in Ghostty).

### Where your writing is kept

| Computer | Vault                                              |
| -------- | -------------------------------------------------- |
| Mac      | `~/Library/Application Support/writui/writui.db`   |
| Linux    | `~/.local/share/writui/writui.db`                  |
| Windows  | `%APPDATA%\writui\writui.db`                       |

It's a single file, so backing up means copying it somewhere safe. The copy
is just as encrypted as the original.

To keep the vault somewhere else (say, a synced folder), run
`writui --db ~/path/to/writui.db`, or make it the default by putting this in
`~/.config/writui/config.toml`:

```toml
vault = "~/path/to/writui.db"
```

## Contributing

Ideas and bug reports are welcome in
[issues](https://github.com/aakashns/writui/issues). To build writui yourself
or work on it, see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

writui is free and open source, under the [MIT license](LICENSE.txt).
