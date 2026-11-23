# The `custom` fork

`custom` on [eyeveil/sonora](https://github.com/eyeveil/sonora/tree/custom) is upstream's
latest release (`upstream/stable`) with this fork's commits on top. The nebula repository (`~/conf` on the Mac,
`~/work/nebula` on ashtwin) installs it on both machines from its flake input `sonora`.

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
  | `n` `N` | Next and previous row of what `/` left in the list |
  | `Enter`, `o` | Play or open the row |
  | `h` `l` | Previous and next column of search results |
  | `/` `i` `a` | Back to the search field (search page and quick search) |

  Away from text, Vimium's keys move around the app:

  | Keys | What they do |
  | --- | --- |
  | `H` `L` | Back and forward |
  | `J` `K`, `gt` `gT` | Next and previous sidebar page, wrapping, skipping hidden rows |
  | `gh` `gs` `gr` | Home, search, recently played |
  | `gl` `ga` `gA` `gp` | Songs, albums, artists, playlists |
  | `?` | The list of every key, with your own keys for rebindable shortcuts |
  | `/` | Filter the page (upstream's key). Enter keeps the filter and goes back to the list |

  `l` keeps upstream's meaning on a page (the lyrics), so `h` and `l` move between columns only
  in the search results. In the search field and the quick search field, `Ctrl+J` and `Ctrl+N`
  step down and `Ctrl+P` steps up without leaving the field. `Ctrl+K` stays the quick search's
  toggle. In the quick search (`Cmd+K` or `Ctrl+K`), Escape leaves the field for normal mode,
  where the keys in the first table work, `q` or Escape closes it, and `o` opens the song's page.
  A key you give a shortcut under Settings, Keybinds wins over the vim key. The page holds the
  keyboard from launch, so these keys work before the first click.
- **Quick jump.** `f`, or `Ctrl+;` (and `Cmd+;` on the Mac), puts a label on every button,
  card and track row on screen, the way Rex's fastcopy labels a pane. Labels are one letter
  while 26 or fewer targets show and two letters after that, home row first. Type a label to
  press it. Type it with Shift to play a card instead of opening it, or to only select a row.
  Backspace removes a letter and its Shift, and Escape or a click closes the labels. A page with
  nothing to press closes them at once, and they don't open over a dialog. Nothing under the player bar,
  the page header, a menu, a dialog or the cover viewer gets a label. Upstream's `f` for
  the fullscreen player moves to `Shift+F`. Both keys can be changed under Settings, Keybinds.
- **Track numbers.** On a page with a numbered track list (an artist's Popular list, an album,
  a playlist, Songs), typing the number a row shows plays it. On a list of five, `3` plays at
  once. On a list of forty, `3` waits 0.65 seconds for a second digit, so `3` and `34` both
  work. A local album numbered by its own track numbers matches those, and a two-disc album
  takes `2.3`. A key that names no row starts a new number. Digits never reach a text field,
  and a number still waiting when you leave the page plays nothing.

The code lives in `crates/ui/src/hints.rs` (labels and number dial), `crates/input/src/lib.rs`
(the `vim()` bindings), `crates/views/src/shells/workspace.rs` (digits),
`crates/views/src/chrome/keys_help.rs` (the `?` list), `crates/views/src/chrome/sidebar_left.rs`
(`step_section`),
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
