# writui — product spec

This is the living product spec. The quoted parts are in the creator's own words
and should stay that way. The **Details** under each section capture what
we've agreed since, and get updated as things change. Technical choices live
in [DECISIONS.md](DECISIONS.md); the build order lives in [PLAN.md](PLAN.md).

## What it is

> A terminal-based writing app called writui. I'm building it mainly for
> myself, but I want to keep it open-sourceable, so let's build it as if it's
> open source. I want to build it with Rust, so that it's light and fast.

> I will write using markdown — I'm a dev and I understand markdown.

> While it's a TUI, I do want mouse operation. I'm not trying to go full vim or
> anything, I like the terminal aspect simply because it's no nonsense. Of
> course, it would still be nice to have keyboard-based navigation that I can
> pick up on slowly as I become a power user.

**Details**

  > change the "Markdown, encrypted, no nonsense." to "No nonsense.
  > Markdown. Local. Encrypted." everywhere

- The tagline is "No nonsense. Markdown. Local. Encrypted." (README, the
  site and its social card, `writui --help`, the GitHub repo description).
- Terminal agnostic. The creator mostly uses Alacritty, sometimes Ghostty and
  iTerm2. Nothing may depend on one terminal's special features.
- Everything works with the mouse; everything also has a keyboard shortcut.
  A hint bar shows the shortcuts that matter on the current screen, so they
  can be picked up gradually.

  > show the shortcuts and the saved status only when i hold down control.
  > otherwise i want a total zen experience

  In the editor, the hint bar only shows while Ctrl is held. Passing
  messages ("Copied.", errors) and dialogs still show. Terminals that can't report
  Ctrl on its own (no kitty keyboard protocol, e.g. Terminal.app, tmux)
  always show them, so the shortcuts are never out of reach. The list and
  other screens keep their hint bar.

  > let's remove ctrl+q from the article page (i mean, let the shortcut be
  > there, just don't show it)

  > okay, let's think about the hint bar a little bit, it's getting out of
  > hand. i've zoomed in my terminal and it's occuping the whole bottom row
  > alraedy. i think i want to go in a different direction with it. so
  > here's what i'm thinking: let's go with a two step approach here. upon
  > pressing control, let's show the hint Ctrl+Space to open menu, and upon
  > clicking control space show the actual menu in a overlay in the center
  > of the page, similar to what omarchy has. the top of the overlay is
  > actually an input box using which i can fiter out commands, and below
  > each command is listed with its shortcut (the direct shortcuts still
  > work, i can reach them without control space). does that make sense?
  > also, let's limit the width of the hint bar to match the post width,
  > and on the right end of the hint bar let's show number of words and
  > date & time of creation

  > let's keep the shortcuts muted. also, instead of bar, let's go with
  > highlight, similar to the list screen. by default, nothing is highlited

  > okay, let's do ctrl+k

  > let's not chagne the list page at all for now, keep the hint bar there

  The editor's hint bar is as wide as the post. It shows just "Ctrl+K menu"
  on the left, and on the right the post's word count and when it was
  started ("412 words · Oct 8, 2026, 9:14 AM"; the date is dropped first, then the
  count, when the bar is too narrow). Ctrl+K (or clicking the hint) opens
  the command menu in the middle of the screen: a filter field on top, then
  every editor command with its shortcut, faded. Nothing is highlighted at
  first; ↑/↓ move the highlight, typing filters (matching anywhere in a
  command's name) and highlights the first match, Enter runs the
  highlighted command, clicking runs one, Esc / Ctrl+K / a click outside
  close it. Every command keeps its own shortcut. Quit (Ctrl+Q) is in the
  menu, not on the bar. Dialogs keep their short Enter / Esc hints. The
  list and other screens are unchanged.

  > the menu on post page should not shift up and down while filtering

  The menu keeps the height of the full list while filtering, so the box
  stays put; matches are listed from the top.

  > can we change the terminal title to the article title? and on the post
  > list page, can we set the terminal title to writui?

- The terminal window's title is the post's title while editing (updating
  as it's typed; "Untitled" if empty), and "writui" everywhere else. The
  terminal's own title comes back on quitting, where the terminal keeps a
  title stack (xterm's push / pop title; others ignore it).
- Shortcuts use Ctrl (not Cmd — most terminals swallow Cmd). The one
  exception: Cmd+arrows also move on macOS, where the terminal passes Cmd
  on (Alacritty does).

  > Cmd/Ctrl+up/down should jump to the start/end of the post

  > similarly, option/alt+up/down should jump logical units

  > also with Cmd/Ctrl+left, if i'm at the beginning of a line, i should go
  > the beginning of the sentence or the beginning of the previous line,
  > whicheer is closer. similarly for Cmd/Ctrl+right

  Ctrl/Cmd+Left/Right go to the start / end of the line (like Home / End;
  a line here is a row on screen). From there, Ctrl/Cmd+Left goes to the
  start of the sentence or of the row before, whichever is nearer, and
  Ctrl/Cmd+Right to the end of the sentence or of the row after. A
  sentence ends at `.`, `!`, `?` or `…` (and any closing quotes, brackets,
  `*` or `_`) before a space, or at the end of its paragraph; a blank line
  counts as a row. Ctrl/Cmd+Up/Down to the start / end of the post (like Ctrl+Home /
  Ctrl+End). Option/Alt+Left/Right jump by word, Option/Alt+Up/Down by
  paragraph. Shift with any of them selects.

  > support cmd+c and cmd+v for cut/copy/paste (mac defaults)

  > for mac, cmd+k should also work apart from ctrl+k for menu

  > but why won't Cmd+C/V work normally when it works in nano? Let's skip
  > the locking for now. Let both Ctrl+L (Cmd+K) and Ctrl+K open the menu.

  On macOS, Cmd+letter in the editor is the same shortcut as Ctrl+letter
  (Cmd+X cuts, Cmd+Z undoes, Cmd+A selects all, …) wherever the terminal
  passes Cmd on, and never types the letter. Terminals keep some Cmd keys
  for themselves: Cmd+V is the terminal's paste, which writui takes as a
  paste, so it just works. Cmd+C is the terminal's copy of *its own*
  selection; in nano that's what you see, because nano leaves the mouse
  to the terminal, but writui draws its own selection (to know what's
  selected), so the terminal has nothing to copy. The README shows the
  one line of terminal config that passes Cmd+C on to writui. Alacritty
  sends Ctrl+L for Cmd+K, so Ctrl+L opens (and closes) the menu too, and
  Cmd+K works there without any config.

## Look and feel

> I want it to inherit the terminal's theme for now, I don't want any inbuilt
> themes, just use the terminal standard colors for this stuff.

**Details**

- Only the terminal's 16 standard colours plus bold / italic / underline /
  dim. No hard-coded RGB in the TUI.

## Encryption and lock

> I want to use this for private writing, and I want to keep whatever I write
> completely encrypted. So basically the entire app will be locked with a
> password/encryption key that I will have to enter when I open the app (just
> like Day One).

**Details**

- Everything lives in the encrypted vault: posts, versions, chat
  history, preview presets, settings, API keys.
- First run creates the vault and asks for the password twice (at least 8
  characters), with a clear warning: there is no recovery. Forgotten
  password = lost writing.
- One machine for now, but the vault file location is configurable. Default:
  `~/Library/Application Support/writui/writui.db` (platform equivalent
  elsewhere).
- Auto-lock after a period of inactivity — planned, not urgent.

  > on the password page, can we put writui in a fancy ascii font?

  > no i want something that feels closer to the normal monospace font
  > that we have for the logo

- Both password screens open with the logo drawn large in block characters:
  the `>` mark, the green caret bar, and "writui" in bold lowercase pixel
  letters, all sitting on one baseline and fitting the 68-column text
  column. Windows narrower than that, or too short to fit it, get the plain
  "writui" title instead.

  > can you try making "writui" less wide? also, i think some space is
  > needed between the logo and the password box. think of the aesthetics

  The letters are narrow (the name is 42 columns, not 54), and a blank line
  under the logo leaves two rows before what follows.

  > put a box around the password

  > and esc to clear password, and if clear esc to quit

- Password fields are boxes with the label on the top edge ("Create your
  vault" puts its two side by side, so the banner still fits 30 rows). Esc
  clears what's typed; with nothing typed, Esc quits.

  > and to the right of the password box add a clickable "Unlock" button

- The unlock screen has an "Unlock" button right of the password box, as
  tall as it, dimmed until something's typed. A window too narrow for it
  leaves it out (Enter still unlocks).

  > and finally, on the password screen, let's add an "Overview" describing
  > how the app works in no more than 5-6 points, each no longer than one
  > line. this is for new users and returning ones, run the copy by me once

- Both password screens (creating the vault, and unlocking) end with an
  "Overview" (creator-approved copy):
  - All your writing lives in one file, encrypted with this password.
  - Forget the password and the writing is gone: there's no recovery.
  - Write in markdown. Every post starts with a # title.
  - Posts save as you type; Ctrl+S also keeps a version to go back to.
  - While writing, Ctrl+K shows every command and its shortcut.
  - Ctrl+O locks writui; unlocking takes you back where you were.

  > you know, with the overview in palce, we dont need the initla para

  So "Create your vault" no longer has its own paragraph about the
  password being unrecoverable: the Overview's first two points say it.
- Change password — planned.

## The screens

> At the moment I'm thinking a two screen flow, where first screen shows list
> of existing posts, which I can pick to edit, or the option to create a new
> post, and the second screen is the post editing screen.

### The list screen

**Details**

- Flat list of posts, most recently updated first unless sorted otherwise.
  Each row shows the title and when it was last updated.
- A "New" action, always visible, "Import" right below it (with a
  different icon: "+ New (n)", "» Import (i)", the shortcuts dim), a blank
  line, then a search field ("🔎 Type to filter (/)", always underlined, and
  all of it dim until typing goes into it) with "Sort: Updated ↓" on its
  right, on the same row. "New" is highlighted
  when the list opens (unless it's coming back to a post, which stays
  highlighted).

  > Change Type to filter.. to Type to filter (/)

  > change "New Post" to just "New"

  > change "Import markdown" to just "Import" (also support import .txt
  > file, apart from md)

  > use a different icon for the "Import" option

  > i dont' like the down icon for import, it can be confusing. what other
  > optoins would you suggest?

  (Creator's pick: "»", over "←", "↳" and "+ Import…".)

  > on the list page, i don't want to use ctrl for the shortcuts, direct
  > keypress is fine

  > we don't need ctrl+q on the main screen to quit, Esc should work (with
  > confirmation)

  > Let's add a type to filter after new & import, which does a full text
  > search of the posts (including front matter), enter opens the first
  > post

  The list's shortcuts are single keys: `n` new, `i` import, `/` filter,
  `d` (or Delete) delete, `s` settings, Tab sort (and in the Trash, `d`
  deletes forever). `n` and `i` are shown on their rows, not in the hint
  bar. `j` / `k` move like ↓ / ↑ (unshown), passing over the search field
  so the next one isn't typed into it.

  > can you add j/k support for down and up on the list page (while i'm
  > not typing in the filter box)?

  > no need to mention them in the bottom menu Ctrl+N and Ctrl+D still work but aren't shown. Esc quits; Ctrl+Q
  and Ctrl+C also quit, unshown.

  > also remove the quit confirmation on th elist screen on pressing esc, i
  > think it's not necessary

  (Esc used to ask "Quit writui?" first. Everything is saved as you type,
  so there's nothing to lose.)

  > add a line gap above type to filter. and below type to filter, i want
  > "Sort by": "Updated", "Created", "Title". for filter use the maginifier
  > icon: 🔎

  > remove settings from the list, it's odd. change "open the first" to just
  > "open" and when i am typing, keep the first result selected by default,
  > basically focus shifts to first result as soon as i type the first
  > chracer, but i am still typing in the sarch box, you understand?

  > btw, can you make it such that if there a nerd font, then we use the
  > non-colored magnifying glass character, and if not we use the emoji,
  > only if it is possible

  (It isn't: terminals don't say which font they use, and a missing glyph
  still takes its space, so writui can't tell. The emoji it is; a setting
  could choose later.)

  > add a nerd fonts on/off setting in settings, and use a nerd font char
  > instead of search magnifying glass

  (So it's a setting: with Nerd Font on, the search field's icon is the
  Nerd Font magnifier, nf-fa-search, in the terminal's own colour.)

  Searching (creator's call: a field you move into, so letters can stay
  shortcuts): `/`, ↓ from Import, or a click puts typing into the field.
  Every word typed has to appear somewhere in the post's text or its front
  matter, ignoring case; the matching posts are listed in the list's order,
  and the Trash row hides while searching. As soon as something's typed,
  the first match is highlighted while typing still goes into the field:
  Enter opens the highlighted post, ↑/↓ move the highlight among the
  matches, Delete deletes it. Esc empties the field, then leaves it (to
  the first post). The empty field is just a row ↑/↓ pass through; a
  click on a post leaves the field too. On the posts, Esc empties the
  search before it quits.

  > let's put "Sort by: Updated " on the right of the "type to filter"
  > (within the main viewport), and make it a dropdown that i can click and
  > select from (have two rows for each e.g. Updated up arrow & updated
  > down arrow for desc and asc)

  > also, when i open the app, "New" should be selected by default. and
  > just show "New (n)" next to new instead of in th emenu. same with (i)
  > for import. and change "/" in the bottom menu to say filter instead of
  > search. and always show an underline for type to filter

  > change "Sort by" to just Sort", keep the entire type to filter line in
  > muted text till it ise selcted (so the underline does not have 2
  > colors).

  > and the hints are not needed in the short menu

  Sorting: by when posts were updated or created, or by title (ignoring
  case), either way round. ↓ is newest first (or Z to A), ↑ oldest first
  (or A to Z). Clicking "Sort" drops
  down a menu under it, right-aligned, with six rows: Updated ↓ / ↑,
  Created ↓ / ↑, Title ↑ / ↓. The
  current one is highlighted; ↑/↓ and Enter, or a click, picks one; Esc
  or a click outside closes it.

  > i want tab to cycle through the sort options, not open the dropdown
  > (i'll use the mouse for that)

  Tab (in the field too) sorts the next way in the menu's order, Shift+Tab
  the one before, round and round. The choice is remembered (in the vault).
  The Trash is always most recently deleted first.

  Settings isn't a row on the list; it's `s`, shown in the hint bar.

  > let's implment the lock using ctrl+o

- **Lock** (Ctrl+O, or Cmd+O; anywhere once unlocked; shown on the list's
  and Settings' hint bars and in the editor's menu): stores the post being
  written, remembers the screen, and closes the vault, so its key is gone
  from memory. The unlock screen says "Locked. Unlock to pick up where you
  were."; the password opens the same screen again (a post with the cursor
  where it was; its undo history doesn't survive). Anything half-done in a
  dialog is dropped. If the post can't be stored, it doesn't lock and the
  editor says why.

  > remember the screen i was previously on at exit, and open back to same
  > screen

- writui opens on the screen it was left on (and so does unlocking after a
  lock): the post that was open (with
  the cursor where it was), the list, the Trash or Settings. A post that's
  been deleted since means the list. The screen is remembered in the vault
  every time it changes, so a closed terminal counts too.
- Search on the main screen, matching against the full post body (not just
  titles).
- Delete a post (with confirmation). Deleted posts go to the Trash.

  > Let's keep a "Recently Deleted".

  > Actually, keep the confirmation still, for deletion.

  > Okay, change of mind, rename "Recently Deleted" to "Trash".

### Trash

**Details**

- Reached from a "Trash (n)" row at the end of the post list
  (only shown when something's in there).
- Each row shows how long until the post is gone for good. Posts are deleted
  forever 30 days after being deleted.
- Restore a post, or delete it forever (with a "can't be undone"
  confirmation).

### Dialogs

**Details**

- ←/→/Tab move between buttons, Enter presses the selected one, each button
  also has a key (e.g. `y`), Esc cancels, and buttons can be clicked.
- The selected button starts on the sensible choice: Delete for moving to
  the Trash (it can be undone), Cancel for anything permanent.

### Posts

**Details**

- Every post starts with `# `. The first line is always an H1 and is the
  post's title. New posts open with `# ` and the cursor ready to type the
  title; the editor won't let the leading `# ` (hash *and* space) be
  removed.
- Created and updated times are tracked automatically for every post.
- Times read naturally: "just now", "25 min ago", "9:05 AM", "Yesterday",
  "Mar 14", "Mar 14, 2025" (12-hour clock).

## Editing

> I want the editing to feel zen, and respect ideal line length etc. (like
> 60-something characters? idk exactly).

**Details**

- The text sits in a centred column, 68 characters wide by default,
  configurable in Settings.
- Soft wrap: lines wrap on screen at the column width, but the stored
  markdown keeps paragraphs as single lines (so it pastes cleanly anywhere).
- Narrower terminals wrap at the terminal's width instead.
- Mouse: click to place the cursor, drag to select, scroll wheel to scroll.
  The scroll wheel moves the view, not the cursor.
- Keyboard: the usual arrows, word / line / paragraph movement, Home/End,
  undo/redo, select with Shift.
  - Undo is Ctrl+Z, redo Ctrl+Y (Ctrl+Shift+Z too, where the terminal tells
    it apart from Ctrl+Z). Typing undoes a word at a time; a run of
    Backspaces is one step. Undo history lasts while the post is open.
  - Up/Down move by row on screen and keep the cursor's column.
  - Home/End go to the start/end of the row on screen, and so do
    Ctrl+Left/Right (and Cmd+Left/Right on macOS); Ctrl+Home/Ctrl+End
    to the start/end of the post. PageUp/PageDown move a screenful.
  - Tab types two spaces (to be configurable in settings).
- Opening a post puts the cursor back where it was when the post was last
  closed, as near the middle of the screen as the text allows. A post that
  has never been opened starts with the cursor at the end.
- The cursor is a blinking bar in the editor.
- If the post can't be stored when leaving the editor, the editor stays
  open and says so (a second Ctrl+Q quits anyway).
- Selection: Shift+movement, drag, double-click a word, triple-click a
  paragraph, Ctrl+A for everything. A selection can include the title's
  `# ` (so copying a whole post gives its markdown), but that `# ` is never
  deleted or replaced. Pasting into an empty post (or over a selection of
  everything) leaves off the pasted text's own leading `#` or `# `, so a
  copied post pastes back as itself. The selection is drawn reversed, and
  typing, Backspace or Delete replaces it (as one undo step). Left/Right
  drop it at its near end; Esc drops it before it leaves the post.
- Word movement is Alt+Left/Right (and Alt+B/F, since terminals differ); paragraph movement is Alt/Ctrl+Up/Down, to the start or end of
  the paragraph (a paragraph is a line of the stored text, not a screen row).
- Copy / cut / paste with the system clipboard: Ctrl+C / Ctrl+X / Ctrl+V.
  If there's no system clipboard (e.g. no display), they still work inside
  writui. Pasting from the terminal (Cmd+V) also works.

  > add a context menu for when i right click on a selection to cut, copy,
  > paste (disabled if clipboard is empty)

  > in the context menu in a post, add  "Select All"

- Right-clicking the text opens a small menu there: Cut, Copy, Paste,
  Select All, with their shortcuts. Cut and Copy are faded (and do
  nothing) without a selection, Paste when the clipboard is empty, Select
  All when the post is empty. Right-clicking inside the
  selection keeps it; elsewhere, the cursor moves there first. ↑/↓ and the
  mouse highlight, Enter or a click runs one, Esc or a click outside
  closes it.

## Autosave and versions

> I think I want conscious versioning i.e. when I hit save, I want a version
> to be recorded. Also, as I am editing a draft is saved too, so if I quit the
> app and come back, it shows me the current draft. So the manual saves are
> more like checkpoints, but let's just call it "save". And when I save, of
> course we record the time etc. but I should also be able to give it a name,
> kind of like a git commit message.

> i want to revisit the terminology around draft and save. beacuse wright
> now, "never saved" seems to indicate that the content is not saved, which
> is not the same. so, let's first remove the terminology draft. we are
> auto-saving the file in real time. and then, what we were previously
> calling "saves", let's call versions. so, by pressing control+s we are
> saving a version. but even in the ctrl+s dialog, mention that the post is
> auto-saved continuously, and here you can record a version that you can
> view later and revert to. you catch my drift? and so we also no longer need
> the save indicator at the bottom of the page.

**Details**

- There are no "drafts": the post itself is saved automatically and
  continuously while editing: a second after typing pauses, at least every
  5 seconds during long stretches of typing, and on leaving the editor or
  quitting. Coming back to a post always shows it as last typed. If storing
  fails, the editor says so and keeps trying.
- **Save version** (Ctrl+S) records a full snapshot of the post with the
  time, under a name. Its dialog says the post is already saved
  automatically, and that a version is something to read later and go back
  to. The name can be left empty. If nothing changed since the last
  version, Ctrl+S just says so.
- There's no saved / unsaved indicator under the text: the post is always
  saved.
- History (Ctrl+R): a list of a post's versions (name + time), newest
  first. Pick one to view its full text, and restore it into the post if
  wanted.
  Restoring asks first, and is an ordinary edit: Ctrl+Z undoes it. The
  history opens over the post, so its undo history survives the visit.
- Versions are kept as long as the post; deleting a post forever deletes
  its versions too.

  > Full text first, that's what I really need.

  Diffs between versions can come later.

## Live markdown formatting

> It would be nice if we can do some realtime formatting of the markdown. I
> still want it to look like markdown, the actual source, but it would be
> nice if the "##" headings show up in a bigger font, maybe a different color,
> same thing with bold, italic etc. too, so I know I'm getting the intended
> effect.

> hmm, i'm not so sure about colored headings actually, i think they disturb
> the flow. let's just make them bold. also, can we make the urls clickable?
> let's not color the bullet/number either. and let's not color the quoted
> text either. i like idea of faded markdown symbols. btw, what are my color
> optoins really, i want a non-distracting non-flow-breaking writing
> experience

> okay, your plan sounds good, but just use a different color for code.
> green sounds fine, actually. later we can look into syntax highlighting
> language-wise

> btw, with bullets are we doing proper indentation? of the text in the
> bullet

**Details**

- The markdown source is always fully visible — no hiding of `#`, `**`, etc.
  The markdown symbols (`#`, `**`, backticks, `>`, bullets and numbers, a
  link's `](url)`) are faded.
- No colours, only text styles, except code: headings (every level) and bold
  are bold, italic is italic, `~~struck~~` is struck through, link text is
  underlined, code (inline and blocks) is green. Quote and list text is
  plain.
- Formatting follows CommonMark (plus GitHub's strikethrough, task lists and
  tables), so half-typed markdown stays plain until it's complete.
- Links open in the browser: Ctrl+click, or "Open link" in the menu with
  the cursor on one (it was Ctrl+O until `0.12.0`, when Ctrl+O became
  lock).
  Markdown links and web addresses written out in the text both count. Only
  `http(s)` and `mailto` links open.
- List items and quotes wrap with a hanging indent: later rows line up under
  the text, not the bullet. On screen only; no spaces are added to the text.
- Later: syntax highlighting in code blocks, per language.

## Web preview

> A lot of the writing I'm doing is actually for work etc. so I want to be
> able to do a web preview of the current article, with live reload, so that
> I can write in the terminal on one half of the screen, and view the web
> preview live on the other half, and since the draft is continuously saving,
> it's nice and live.

> The web preview will have multiple presets. The default will be simple
> white text in Inter font, standard settings on dark background, but there
> will be other presets like Substack, Medium, Gmail, etc. so that I can view
> exactly what the text looks like on the platform I'm targeting. I should
> also be able to create my own preset by providing a CSS file that can
> customize every aspect of the preview, including using Google Fonts etc. so
> that I can make it look like my company blog too.

**Details**

- A key in the editor opens the preview in the browser. It reloads live on
  every autosave.
- The preview server only listens on this machine and needs a secret token
  in the URL, since it serves decrypted writing.
- Built-in presets: Default (white Inter on dark), Substack, Medium, Gmail.
- Custom presets are CSS, stored in the vault alongside everything else.
  Create one by importing a CSS file, or by duplicating a built-in and
  editing it inside writui. Google Fonts work via `@import`.
- Switch preset from the editor.
- Nice to have: the preview follows the cursor's position.

## Getting writing out

> Yes, let's have a copy as formatted text, rich text copying is useful.

> Yes, we do also want export as markdown and copy as markdown.

> since frontmatter is such a common thing, i think it might be worth adding
> into the export flow, yeah? like when i export, i want to be able to edit
> the front matter (we can pre-populate it with title, date, description,
> (say first N characters after title), and slug (same logic as filename),
> updated (current date), and the user can add more tags. let it be
> remembered between exports. and let us also remember the export path
> between exports. and i want path completion with tab in the export dialog
> where i enter the export path, yeah? and let's also ask for confirmation
> when overwriting an existing file during export, i expect that to be a
> common thing if i want to go back and edit a post.

> along with this, let's also add an Import option on the main page, right
> below New Post. and when i import i also want path completion there. and
> if the imported post has frontmatter, parse it and save it, and also save
> the export path for the file as the place it was imported from. so the
> idea is that i can import a post, edit it here and and then export it
> back.

**Details**

- **Copy as markdown** (Ctrl+Shift+C, or the menu): the whole post's
  markdown source to the clipboard.
- **Export as markdown** (Ctrl+Shift+S, like "Save as", or the menu): write
  the post to a `.md` file of my choosing, with front matter on top. One
  dialog has both:
  - **The file.** The first time, it suggests the post's title as a file
    name (`morning-pages.md`) in the folder of the last export (or import),
    or else Documents. After that, it's where the
    post was last exported to (or imported from). Typing a folder puts that
    file name in it, and `~` is the home folder. Tab completes paths as in a
    shell: as far as the matches agree, listing them under the field; Tab
    again goes through them (Shift+Tab back), Enter keeps one, and they can
    be clicked. Ctrl+W deletes back a folder.
  - **The front matter**, YAML between `---` lines, which takes the place of
    the `# Title` line (site generators show the title themselves), unless
    "Keep the # title line" is checked (it isn't at first): then the line
    comes after the front matter. It can
    be edited freely (add `tags`, `author`, anything) and turned off. writui
    suggests `title`, `date` and `updated` (today), `description` (the
    first 160 characters of the text, without markdown, cut at a word) and
    `slug` (the file name's logic).
  - **Remembered per post**, in the vault: the file, the front matter, and
    both checkboxes. Exporting again keeps the front matter as it was left,
    except that `title` is always the post's title, `updated` is today, and
    `description` and `slug` follow the post unless they were changed by
    hand. `date` stays. Fields taken out stay out. A post exported for the
    first time gets the extra fields (e.g. `author`, `tags`) and the
    checkboxes of the latest export (creator's call).
  - It asks before replacing a file (Cancel is the default, and goes back
    to the dialog). The file ends with a newline.
  > export files to the right place by default (Documents?)

  > keep track of where a file was imported from, so we can export back to
  > that locaation by default

  (That second one was already so since `0.11.0`: the file a post was
  imported from is its own, and the first export goes back to it.)
- **Import** (on the list, below "New", or `i`): makes a post of a `.md`
  file, or any text file (`.txt` too). The path has the same Tab completion, starting in
  the folder of the last export or import.
  - The title is the front matter's `title`, or else the file's first line
    if it's `# Title`, or else the file name ("morning-pages.md" → "Morning
    pages"). A `# Title` line that repeats the front matter's title isn't
    kept twice.
  - The front matter is kept, and the file is remembered as the post's own,
    so the next export goes back to it with that front matter (adding the
    suggested fields the file didn't have). If the file had front matter
    and a `# Title` line too, "Keep the # title line" starts checked. TOML front matter (`+++`, as
    Zola also takes) is turned into YAML (creator's call); what can't be
    turned into YAML is kept as comments.
  - Importing a file that's already a post's (imported from or exported to
    that file, and not in the Trash) asks: update that post (its text
    before is saved as a version, "Before importing <file>") or make a new
    post (creator's call).
  - Files that aren't text, or bigger than 10 MB, are refused.

  > while importing a file, use the file's creation date (instead of
  > current time)

  - A new post made by importing was started when the file was made (or
    else last changed, where the system doesn't know), not when it was
    imported. Updating a post from its file keeps the post's own date.

    > btw, for an imported file, use the created at date fromm the file
    > metadata

  - A file without a `date` in its front matter gets that same day as its
    `date` (after `title`), not the day it's first exported. A file with
    no front matter at all keeps none, but turning it on in the export
    dialog starts with that date.
- **Copy as formatted text**: the whole post (or the selection) as rich
  text, so pasting into Substack, Gmail, Medium, Google Docs etc. keeps
  headings, bold, links, lists.

## LLM chat sidebar

> I like brainstorming with an LLM while writing. So, I should be able to open
> up an LLM chat sidebar while editing a post. Maybe I'll put in my API key
> (let's say OpenAI & Anthropic for now) somewhere in a settings screen, or
> when I open the LLM chat for the first time (we'll see about that). And
> then, I should be able to chat with the LLM and ask it to edit the current
> post, just like I might do in, say, Cursor with a markdown file open. Other
> than that, I think when I select some text, I want the LLM to know what is
> selected (and maybe also where my cursor is in general), so I don't need to
> provide that context while I'm in the flow of editing.

**Details**

- Providers: OpenAI and Anthropic. API keys are stored in the vault.
- The LLM automatically gets the whole post, the current selection and the
  cursor position.
- Edits the LLM proposes are shown as a diff to accept or reject.

  > Checkpoints are mine. We might revisit this later.

  The LLM never creates versions. Accepted edits change the post like any other
  edit (and can be undone).
- Chat history is kept per post. A post can have several conversations; pick
  one up again like Claude Code's `/resume` does per folder.
- Note: whatever is sent to the LLM leaves the machine (and the encryption).

## Settings

> let's add a third "Settings" screen now, keep some minimal settings there
> (like defualt Author name)

> and on the settings page Esc should just say "back"

> add a nerd fonts on/off setting in settings, and use a nerd font char
> instead of search magnifying glass

**Details**

- A Settings screen, reached with `s` on the list (it's on the hint bar). Each row is a setting and its value; Enter or a
  click changes it in a dialog; Esc goes back to the list (its hint
  just says "back"). Changes are
  stored right away.
- For now:
  - **Author**: suggested as `author` in the front matter of exports
    (after `title`), unless it's empty. Like the description and slug, it
    follows Settings on later exports unless it was changed by hand in the
    front matter.
  - **Line width**: how wide the text is in the editor (and the history),
    from 40 to 120 characters; 68 by default.
  - **Nerd Font**: on or off (off by default), for terminals whose font is
    a Nerd Font: writui uses its icons (so far, the search field's
    magnifier). Enter or a click switches it, no dialog; the hint says
    "Enter switch".
- Later: what Tab types, default preview preset, API keys, default
  model, auto-lock timeout — all stored in the vault.
- The vault file location has to be known *before* unlocking, so it lives
  outside the vault in a small plain config file (and can also be passed on
  the command line).

## Project site: landing page, blog, changelog

> I will also need a landing page & blog/changelog for writui, and I want to
> keep it in the same repo. As you can imagine, I will write the changelog /
> blog posts for writui using writui itself.

> writui.com

> remove the automatic changelog, rather let "Changelog" point to the
> Github releasees page.

**Details**

- The site lives in `site/` in this repo, and is published at
  **writui.com** (Cloudflare Workers).
- It looks like the app: light Inter text on a dark background, one narrow
  column. Pages: a landing page (what writui is, install, a short
  recording of it, then what it does) and the blog.

  > and on the site, let's move installation above the demo tape and add a
  > copy button

  The install command has a "Copy" button in its corner (shown only where
  the browser allows copying; it says "Copied" for a moment). The landing
  page is kept up to date with each new feature (see AGENTS.md). "Changelog" in the menu links to the GitHub
  releases page.
- Posts live in the encrypted vault, but the blog lives in the public repo —
  so blog posts get into `site/content/blog/` via "export as markdown",
  with front matter (Zola reads YAML front matter), and are edited by
  importing them and exporting them back.
- A "writui site" preview preset matching the site's CSS, so blog posts can be
  previewed exactly as they'll look.

### Try online

> any chance in hell we can have a "try online" version of writui via wasm
> without that we can embed on the site at writui.com/try ? like is that even
> possible

> i definitely want it to work ona phone, what's the blocker?

> okay, make it the next item of work

**Details**

- **writui.com/try** runs writui in the visitor's browser, with no server:
  the same screens, built for the web (Ratzilla, ratatui's web backend), as
  static files on the site.
- It's a demo: no encryption or password, posts kept in that browser only
  (or forgotten with the tab), no files, no `writui upgrade`. Import / export
  become upload / download, or are hidden.
- Shortcuts the browser keeps for itself (Ctrl+W, Ctrl+T, Ctrl+N, …) get
  other keys in the demo.
- It must work on a phone: tapping opens the on-screen keyboard (a hidden
  text field that typing goes through, so autocorrect and swipe typing
  work), a row of extra keys above it (Esc, Ctrl, Tab, arrows), taps act as
  clicks, touch scrolls, and every screen works at phone width (about 40–50
  columns).
- The real app's behaviour doesn't change for this; CI checks that the web
  build still builds.

## Open source

> I think we'll be open sourcing sooner rather than later, so that I can use
> GitHub Actions and releases and all too, but I want to have something
> concrete first.

> Let's plan to do the open source and site right after M2, I want to get it
> out as soon as it's doing something.

> i want to take the repo public right now use the github releases going
> forward

> and can we publish for all platforms?

> let's remove the logic to download to my local bin in scripts/ship.sh, i
> want to upgrade manually like everyeone else

> let's drop the windows builds entriely, yeah, and just include a "Windows"
> section in the readme asking people to build from source. and let's also
> set up the automatic release from main when version is bumped. and for
> macos and linux, i want a single command to install, like bun.com has

> lets do the shorter install command too

**Details**

- License: MIT, in `LICENSE.txt`.
- Public on GitHub from M0 onwards; the project site still comes with M3.
- Merging a PR that bumps the version publishes a GitHub release
  automatically: prebuilt binaries for macOS and Linux (each x86_64 and
  arm64), with notes listing the merged PRs. The creator upgrades the same
  way every user does.
- Install on macOS / Linux with one command:
  `curl -fsSL https://writui.com/install.sh | bash`
  (writui.com serves the repo's `install.sh`; every release has it
  attached too, and the README gives that longer URL as well).
  Installs to `~/.local/bin`, never edits shell startup files (it prints the
  line to add instead).
- Windows: no prebuilt binaries; the README explains building from source.
  CI still builds and tests on Windows.
- macOS binaries aren't signed (no paid Apple Developer account). Installing
  with the script or `curl` isn't affected; browser downloads need a one-off
  `xattr` command, explained in the README.

## Command line

> I want the binary to also be usable as a CLI, like to list posts, show a
> post, export a post etc. That can come much later.

> i want to add a upgrade cli command which checks the current version, and
> upgrades to the latest version (with confirmation, or --yes), and performs
> the migrations (backup before, delte backup after success), and also add a
> --version cli command

> also, i want you to remove the migrate step within upgrade, since migration
> is auto applied on next open, so no point asking for the password twice

> add a writui info command that prints current version, database file
> location, config file location

**Details**

- Plain `writui` opens the app; subcommands (e.g. `writui list`,
  `writui show`, `writui export`) do one thing and exit.
- The CLI still needs the vault unlocked, so it asks for the password. How
  that works for scripting is to be decided when we get there.
- `writui --version` prints the version.
- `writui info` prints the version, the vault's location and the config
  file's (respecting `--db`), saying "(not there yet)" for one that
  doesn't exist. It doesn't need the password.
- `writui upgrade` checks GitHub for a newer release, shows "0.1.0 → 0.2.0"
  and a link to what's new, asks before upgrading (`--yes` skips that), then
  downloads it, checks its checksum and replaces itself. It doesn't touch
  the vault or ask for the password: the new version migrates the vault the
  next time it's unlocked, if it needs to. (From 0.5.0 to 0.9.0, upgrade
  asked for the password and migrated right away; dropped in 0.10.0, since
  it meant typing the password twice.) Migrations back the vault up
  first, check the migrated vault's integrity, and only then delete the
  backup; if anything fails, the backup stays and the error says where it
  is.

## Not now

- Full vim mode.
- Inbuilt themes.
- Sync across machines.
- Tags / folders.
- Diffs between versions.
