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

  > can we change the terminal title to the article title? and on the post
  > list page, can we set the terminal title to writui?

- The terminal window's title is the post's title while editing (updating
  as it's typed; "Untitled" if empty), and "writui" everywhere else. The
  terminal's own title comes back on quitting, where the terminal keeps a
  title stack (xterm's push / pop title; others ignore it).
- Shortcuts use Ctrl (not Cmd — most terminals swallow Cmd). The one
  exception: Cmd+Left/Right also go to the start/end of the line on macOS,
  where the terminal passes Cmd on (Alacritty does).

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
- Change password — planned.

## Two screens

> At the moment I'm thinking a two screen flow, where first screen shows list
> of existing posts, which I can pick to edit, or the option to create a new
> post, and the second screen is the post editing screen.

### The list screen

**Details**

- Flat list of posts, most recently updated first. Each row shows the title
  and when it was last updated.
- A "New post" action, always visible, and "Import markdown" right below
  it (Ctrl+O).
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
  configurable in settings.
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
- Links open in the browser: Ctrl+O with the cursor on one, or Ctrl+click.
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
    or else the folder writui was started from. After that, it's where the
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
- **Import markdown** (on the list, below "New post", or Ctrl+O): makes a
  post of a `.md` file. The path has the same Tab completion, starting in
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

**Details**

- Line width, what Tab types, default preview preset, API keys, default
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
  column. Pages: a landing page (what writui is, a short recording of it,
  install) and the blog. "Changelog" in the menu links to the GitHub
  releases page.
- Posts live in the encrypted vault, but the blog lives in the public repo —
  so blog posts get into `site/content/blog/` via "export as markdown",
  with front matter (Zola reads YAML front matter), and are edited by
  importing them and exporting them back.
- A "writui site" preview preset matching the site's CSS, so blog posts can be
  previewed exactly as they'll look.

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

**Details**

- Plain `writui` opens the app; subcommands (e.g. `writui list`,
  `writui show`, `writui export`) do one thing and exit.
- The CLI still needs the vault unlocked, so it asks for the password. How
  that works for scripting is to be decided when we get there.
- `writui --version` prints the version.
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
