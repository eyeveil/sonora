# The `custom` fork

`custom` on [eyeveil/sonora](https://github.com/eyeveil/sonora/tree/custom) is upstream's
latest release (`upstream/stable`) with this fork's commits on top. `~/conf` installs it on the
Mac and on ashtwin from the flake input `sonora`.

## What the fork adds

- **The search polish patch.** It is the branch `fix/polish-search-releases`, open upstream as
  sonorahq/sonora#1553. The fork carries its code but leaves `CHANGELOG.md` as upstream has it.
- **Vim motions.** These keys work on every track table, on the list a page shows, in the
  search results and in the quick search's normal mode:

  | Keys | What they do |
  | --- | --- |
  | `j` `k` | Next and previous row |
  | `gg` `G`, Home End | First and last row |
  | `Ctrl+D` `Ctrl+U`, Page Down Page Up | Half a page down and up |
  | `Enter`, `o` | Play or open the row |
  | `h` `l` | Previous and next column of search results |
  | `/` `i` `a` | Back to the search field (search page and quick search) |

  In the search field and the quick search field, `Ctrl+J` and `Ctrl+N` step down and `Ctrl+K`
  and `Ctrl+P` step up without leaving the field. In the quick search (`Cmd+K` or `Ctrl+K`),
  Escape leaves the field for normal mode, where the keys in the table work, `q` or Escape
  closes it, and `o` opens the song's page.
- **Quick jump.** `f`, or `Ctrl+;` (and `Cmd+;` on the Mac), puts a two-letter label on every
  button, card and track row on screen, the way Rex's fastcopy labels a pane. Type a label to
  press it. Type it with Shift to play a card instead of opening it, or to only select a row.
  Backspace removes a letter, and Escape or a click closes the labels. Upstream's `f` for the
  fullscreen player moves to `Shift+F`. Both keys can be changed under Settings, Keybinds.
- **Track numbers.** On a page with a numbered track list (an artist's Popular list, an album,
  a playlist, Songs), typing a row's number plays it. On a list of five, `3` plays at once. On
  a list of forty, `3` waits 0.65 seconds for a second digit, so `3` and `34` both work. A
  digit that names no row starts a new number. Digits never reach a text field.

The code lives in `crates/ui/src/hints.rs` (labels and number dial), `crates/input/src/lib.rs`
(the `vim()` bindings), `crates/views/src/shells/workspace.rs` (digits),
`crates/views/src/screens/search.rs` and `crates/views/src/shared/powerbar.rs`.

## Following upstream

`scripts/fork-sync.sh` rebases `custom` onto `upstream/stable`, runs the checks and
force-pushes. Run it after each upstream release, then move `~/conf` onto the new head:

```sh
scripts/fork-sync.sh              # or: scripts/fork-sync.sh upstream/main
cd ~/conf && nix flake update sonora
```

Then build and push `~/conf` the usual way. Rebase conflicts stop the script. Git's `rerere`
remembers each resolution, so a conflict you resolved once resolves itself the next time.

When upstream merges #1553, its commits conflict with their squashed copy. Skip each of them
with `git rebase --skip`, since upstream already has the change.
