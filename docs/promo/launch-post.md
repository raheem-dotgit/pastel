# Pastel launch drafts

Drafts for announcing Pastel. Check every link and claim against the current
release before posting.

Screenshot to attach: `docs/screenshot-dark.png` (or `docs/screenshot-light.png`),
also at https://raw.githubusercontent.com/raheem-dotgit/pastel/main/docs/screenshot-dark.png

---

## a) Reddit post (r/gnome, r/linux)

**Title:** I made Pastel, a Win+V style clipboard history for GNOME (GTK4 + libadwaita, Rust)

I missed Windows' Win+V clipboard history on Ubuntu, so I built **Pastel**.
Press **Super+V** and you get a searchable list of everything you've copied.
Pick a clip, press Enter, and paste it with Ctrl+V.

- Native GTK4 + libadwaita UI that follows your light/dark theme
- Instant search, pinned clips, and hide-after-copy
- Skips clips that password managers such as KeePassXC mark as secret
- History is stored locally, readable only by your user
- Small (~600 KB) and written in Rust

Install with one command. It adds an app menu entry and binds Super+V on GNOME:

```bash
curl -fsSL https://raw.githubusercontent.com/raheem-dotgit/pastel/main/install.sh | sh
```

To be upfront about one limit: on GNOME, Wayland only lets apps see clipboard
changes while their window is focused. So Pastel records copies made while
it's open, plus your latest copy each time you open it. On Sway, Hyprland and
other wlroots compositors it records every copy. It's text only for now.

It's free and open source (MIT): https://github.com/raheem-dotgit/pastel

Feedback, bug reports and PRs are very welcome!

[screenshot]

---

## b) awesome-gnome entry

For the **Applications → Utilities** section of
https://github.com/Kazhnuz/awesome-gnome, matching the existing line format:

```markdown
- [Pastel](https://github.com/raheem-dotgit/pastel) - Win+V style clipboard history with search and pinned clips.
```

---

## c) OMG! Ubuntu tip

**Pastel brings Windows-style clipboard history (Win+V) to Ubuntu**

Pastel is a new open-source clipboard manager for Ubuntu and GNOME that works
like Win+V on Windows. Press Super+V to open a searchable list of text you've
copied, pin the clips you use often, and press Enter to copy one back, ready
to paste with Ctrl+V. It's a native GTK4 and libadwaita app written in Rust,
so it follows your light or dark theme and fits in with the rest of GNOME.

Install it with a single command from the project's GitHub page, which also
binds Super+V for you. One thing to know: on GNOME, Wayland only lets Pastel
see copies made while its window is open, plus the latest copy whenever you
open it; on Sway and Hyprland it records everything. Pastel is free under
the MIT license: https://github.com/raheem-dotgit/pastel
