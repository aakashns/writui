# writui — product spec

This is the living product spec. The quoted parts are in Aakash's own words
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

- Terminal agnostic. Aakash mostly uses Alacritty, sometimes Ghostty and
  iTerm2. Nothing may depend on one terminal's special features.
- Everything works with the mouse; everything also has a keyboard shortcut.
  A hint bar shows the shortcuts that matter on the current screen, so they
  can be picked up gradually.
- Shortcuts use Ctrl (not Cmd — the terminal swallows Cmd).

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

- Everything lives in the encrypted vault: posts, drafts, saves, chat
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
- A "New post" action, always visible.
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
  - Up/Down move by row on screen and keep the cursor's column.
  - Home/End go to the start/end of the row on screen; Ctrl+Home/Ctrl+End
    to the start/end of the post. PageUp/PageDown move a screenful.
  - Tab types two spaces (to be configurable in settings).
- Opening a post puts the cursor back where it was when the post was last
  closed, as near the middle of the screen as the text allows. A post that
  has never been opened starts with the cursor at the end.
- The cursor is a blinking bar in the editor.
- If the draft can't be stored when leaving the editor, the editor stays
  open and says so (a second Ctrl+Q quits anyway).
- Copy / cut / paste with the system clipboard.

## Drafts and saves

> I think I want conscious versioning i.e. when I hit save, I want a version
> to be recorded. Also, as I am editing a draft is saved too, so if I quit the
> app and come back, it shows me the current draft. So the manual saves are
> more like checkpoints, but let's just call it "save". And when I save, of
> course we record the time etc. but I should also be able to give it a name,
> kind of like a git commit message.

**Details**

- The draft saves itself continuously while editing (and on quit). Coming
  back to a post always shows the current draft.
- **Save** (Ctrl+S) asks for a name and records a full snapshot of the post
  with the time.
- The editor shows when the draft has changed since the last save.
- History: a list of a post's saves (name + time). Pick one to view its full
  text, and restore it into the draft if wanted.

  > Full text first, that's what I really need.

  Diffs between versions can come later.

## Live markdown formatting

> It would be nice if we can do some realtime formatting of the markdown. I
> still want it to look like markdown, the actual source, but it would be
> nice if the "##" headings show up in a bigger font, maybe a different color,
> same thing with bold, italic etc. too, so I know I'm getting the intended
> effect.

**Details**

- The markdown source is always fully visible — no hiding of `#`, `**`, etc.
- Terminals can't change font size, so headings are distinguished with bold,
  colour and underline per level instead.
- Bold, italic, inline code, code blocks, links, quotes and lists get styled
  too, using only terminal-standard colours.

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
  every draft autosave.
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

**Details**

- **Copy as markdown**: the whole post's markdown source to the clipboard.
- **Export as markdown**: write the post to a `.md` file of my choosing.
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

  The LLM never creates saves. Accepted edits change the draft like any other
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

**Details**

- The site lives in `site/` in this repo.
- Posts live in the encrypted vault, but the blog lives in the public repo —
  so blog posts get into `site/content/` via "export as markdown".
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

**Details**

- License: MIT OR Apache-2.0 (dual, the Rust convention).
- Public on GitHub from M0 onwards; the project site still comes with M3.
- Every ship is a GitHub release: prebuilt binaries for macOS (Apple Silicon,
  Intel) and Linux (x86_64, arm64), with notes listing the merged PRs. Aakash
  installs the same release build everyone else downloads.

## Command line

> I want the binary to also be usable as a CLI, like to list posts, show a
> post, export a post etc. That can come much later.

**Details**

- Plain `writui` opens the app; subcommands (e.g. `writui list`,
  `writui show`, `writui export`) do one thing and exit.
- The CLI still needs the vault unlocked, so it asks for the password. How
  that works for scripting is to be decided when we get there.

## Not now

- Full vim mode.
- Inbuilt themes.
- Sync across machines.
- Tags / folders.
- Diffs between versions.
