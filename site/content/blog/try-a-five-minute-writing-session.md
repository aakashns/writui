+++
title = "Try a five-minute writing session"
description = "Create your vault, write your first post, and pick up where you left off. A step-by-step guide to writui."
date = 2026-10-13
+++

Let's write your first post in writui. You'll choose a password, put a few
thoughts on the page, and save a version to come back to. Here are the steps
we'll follow:

1. [Install and open writui](#1-install-and-open-writui)
2. [Choose your password](#2-choose-your-password)
3. [Start a post](#3-start-a-post)
4. [Write a few thoughts](#4-write-a-few-thoughts)
5. [Keep a version](#5-keep-a-version)
6. [Come back to it](#6-come-back-to-it)

Before installing: your drafts live in a password-encrypted vault, not loose
`.md` files. A forgotten password cannot be recovered. Exporting a post creates
an ordinary, **unencrypted** Markdown file that other editors can open.

## 1. Install and open writui

On macOS or Linux, paste this into your terminal and press Enter:

```sh
curl -fsSL https://writui.com/install.sh | bash
```

Follow any instructions the installer prints, then open the app:

```sh
writui
```

On Windows, follow the [build instructions](https://github.com/aakashns/writui#windows)
first, then run `writui`.

## 2. Choose your password

The first time you open writui, it asks you to create your vault. This is
where your posts and their versions are kept, encrypted on your computer.

Type a password of at least eight characters, press Tab, and type it again
in **Confirm password**. Press Enter to create your vault. You can also
click either password box to move between them.

<a target="_blank" rel="noopener" href="/blog/first-session/create-vault.webp"><img class="terminal-screenshot" src="/blog/first-session/create-vault.webp" alt="The Create your vault screen with Password and Confirm password fields" width="1000" height="360" loading="eager" decoding="async"></a>

Choose a password you can remember or keep in your password manager.
**There is no password recovery:** forgetting it means losing access to
your writing. If you've already set up writui, enter your existing password
to unlock it instead.

## 3. Start a post

You'll see your list of posts. Click **New** or press `n` to start writing.

<a target="_blank" rel="noopener" href="/blog/first-session/new-post.webp"><img class="terminal-screenshot" src="/blog/first-session/new-post.webp" alt="The empty post list, with New highlighted above Import and the search field" width="1000" height="300" loading="eager" decoding="async"></a>

The first line already starts with `# `. Type a title, such as
**A little room to think**, then press Enter twice to leave a blank line
before the body.

## 4. Write a few thoughts

Write about something on your mind. Or try this short example to see how
Markdown looks:

<div class="command writing-example">
<button type="button" class="copy" data-copy="writing-example" aria-live="polite" hidden>Copy</button>

<pre><code id="writing-example">Today I want to make a little room for **writing**.

## Things worth noticing

- A good conversation
- An idea for tomorrow
- A sentence I want to keep

One quiet paragraph is a good start.</code></pre>

</div>

<a target="_blank" rel="noopener" href="/blog/first-session/writing.webp"><img class="terminal-screenshot" src="/blog/first-session/writing.webp" alt="A post titled A little room to think with a bold word, a heading, and a list" width="1000" height="350" loading="eager" decoding="async"></a>

The text is formatted as you type: `**writing**` becomes bold, `##` starts a
heading, and `-` starts a list. The Markdown marks stay visible, so you can
always see and edit what you typed.

Your post **saves automatically as you write**. You don't need to press a
save button to keep your changes.

## 5. Keep a version

Press Ctrl+K to open the menu. It lists the commands and their shortcuts.
You can click a command, move to it with the arrow keys and press Enter,
or type part of its name to find it.

<a target="_blank" rel="noopener" href="/blog/first-session/menu.webp"><img class="terminal-screenshot" src="/blog/first-session/menu.webp" alt="The command menu showing Save version, History, Export as markdown, and other commands" width="1000" height="600" loading="eager" decoding="async"></a>

Choose **Save version**, or close the menu with Esc and press Ctrl+S.
Name the version **First thoughts**, then press Enter.

<a target="_blank" rel="noopener" href="/blog/first-session/save-version.webp"><img class="terminal-screenshot" src="/blog/first-session/save-version.webp" alt="The Save version dialog with First thoughts entered as the version name" width="800" height="260" loading="eager" decoding="async"></a>

A version keeps a snapshot of your post. It's useful when you want to
try a different direction without losing the earlier one. Open **History**
from the menu, or press Ctrl+R, to see the version you just saved. Press
Esc to return to writing.

## 6. Come back to it

Press Esc in the editor to return to your posts. Click your post to open it,
or select it with the arrow keys and press Enter.

<a target="_blank" rel="noopener" href="/blog/first-session/posts.webp"><img class="terminal-screenshot" src="/blog/first-session/posts.webp" alt="The post list with A little room to think saved and selected" width="1000" height="280" loading="eager" decoding="async"></a>

When you're done for now, press Ctrl+Q to quit. Next time, run `writui`,
enter your password, and you'll be back where you left off. To lock it
without quitting, press Ctrl+O; unlocking brings you back to the same place.

To keep a Markdown copy outside writui, open the menu and choose
**Export as markdown**. Exported files are ordinary text files, so store
them somewhere suitable for what you've written.

## How did it go?

[Tell us about your writing session](https://github.com/aakashns/writui/issues/new?template=writing-feedback.yml).
What did you want to write? Did anything get in your way? Would you come
back to it tomorrow? If you've already been using writui, we'd like to
hear what makes you keep using it too.

GitHub feedback is public, so describe your experience without including
private writing or your password.
