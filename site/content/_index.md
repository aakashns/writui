+++
+++

- **Private.** All your posts live in one file, encrypted with your
  password. Without the password nobody can read them. Ctrl+O locks it
  when you step away, and unlocking takes you back where you were.
- **Plain markdown.** Each post starts with a `# Title`, and it stays plain
  markdown on screen, formatted as you type: headings and bold in bold,
  italics in italics, code in green, with the markdown symbols faded.
- **Versions.** Save a named version whenever you like, look back through
  them, and bring an old one back.
- **Mouse and keyboard.** Click, scroll, select and press buttons with the
  mouse, or use shortcuts. While writing, Ctrl+K opens a menu of
  everything you can do.
- **Fits your terminal.** It uses your terminal's own colours and font, and
  works in any modern terminal: Alacritty, Ghostty, iTerm2, and others.
- **Finds anything.** Type to search the full text of every post, front
  matter and all, and sort the list by when posts were updated or created,
  or by title, either way round.
- **Gets out of the way.** A single centred column of text. The word count
  and shortcuts show up while you hold Ctrl, and it opens where you left
  off.
- **Takes your writing anywhere.** Copy a post as markdown, or export it as
  a file with front matter for your blog. Import a markdown or text file to
  edit it here and export it back.

writui is early, and built in the open, a small piece at a time. It's
usable for writing now, but keep a copy of anything you can't afford to
lose. And there is no way to recover a forgotten password.

## First session

After installing on macOS or Linux, try writing in a temporary vault:

```sh
writui --db "$(mktemp -d)/writui.db"
```

Use `writui-dev` as the password for this throwaway vault. Write only sample
text here: your computer may clean up temporary files later.

1. Choose **New**, add a title, and write a few sentences.
2. Try a heading, a list, and some **bold** text using markdown.
3. Open the menu with Ctrl+K to explore versions and exporting.
4. Close the menu with Esc, then press Esc again to return to the list.
   Open your post again: changes are saved automatically.

[Tell us how it went](https://github.com/aakashns/writui/issues/new?template=writing-feedback.yml).
What did you want to write? What got in your way? Would you use writui again?
If you've been using it for a while, we'd like to hear that too. Feedback on
GitHub is public; leave out private writing and passwords.

Ready to keep writing? Run `writui` normally and choose your own password
for your regular vault.
