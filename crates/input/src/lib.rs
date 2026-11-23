use std::collections::BTreeMap;

use gpui::{Action, KeyBinding, actions};
use ui::{
    Activate, Backspace, BackspaceToStart, BackspaceWord, Copy, Cut, Delete, DeleteToEnd,
    DeleteWord, Deselect, Dismiss, End, FORM_CONTEXT, Home, INPUT_CONTEXT, Left, MENU_CONTEXT,
    MoveRowDown, MoveRowUp, NextField, Paste, PreviousField, Remove, Right, SelectAll, SelectEnd,
    SelectFirst, SelectHome, SelectLast, SelectLeft, SelectNext, SelectPageDown, SelectPageUp,
    SelectPrevious, SelectRight, SelectWordLeft, SelectWordRight, ShowCharacterPalette, Space,
    Submit, TABLE_CONTEXT, WordLeft, WordRight,
};

mod shortcuts;

pub use shortcuts::{
    Section, Shortcut, assign, clear, keystroke_of, reset, shortcut, shortcuts, shown,
};

actions!(
    sonora,
    [
        Quit,
        SignOut,
        Refresh,
        TogglePlayback,
        SongPrevious,
        SongNext,
        VolumeUp,
        VolumeDown,
        SeekBack,
        SeekForward,
        ToggleShuffle,
        ToggleRepeat,
        NavigateBack,
        NavigateForward,
        OpenFilter,
        OpenSearch,
        OpenSettings,
        ToggleFullscreen,
        ToggleQueue,
        ToggleLyrics,
        TogglePowerbar,
        PowerbarDetails,
        ShowMembers,
        CloseWindow,
        MinimizeWindow,
        ZoomWindow,
        ToggleWindowFullscreen,
        Hide,
        HideOthers,
        ShowAll,
        IncreaseFontSize,
        DecreaseFontSize,
        ResetFontSize,
        Undo,
        Redo,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        HintJump,
        FocusField,
        ShowKeys,
        NextSection,
        PreviousSection,
        GoHome,
        GoHistory,
        GoSongs,
        GoAlbums,
        GoArtists,
        GoPlaylists
    ]
);

/// A digit typed away from text, which plays that row of the numbered list on the page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Action)]
#[action(namespace = sonora, no_json)]
pub struct TypeDigit(pub char);

pub const WORKSPACE_CONTEXT: &str = "Workspace";
/// The context of the cover viewer, open above an album, playlist or artist page.
pub const COVER_VIEWER_CONTEXT: &str = "CoverViewer";
pub const SEARCH_CONTEXT: &str = "Search";
pub const POWERBAR_CONTEXT: &str = "Powerbar";
/// The context of the title bar's page filter, the field `/` opens.
pub const TOOLBAR_CONTEXT: &str = "Toolbar";

/// Every key binding, in precedence order: GPUI prefers the deepest matching context and,
/// within one, the binding registered last, so the macOS set at the end overrides the shared one.
/// The shortcuts come before the shared set so that a key the page takes for a list, such as
/// Up and Down, keeps the list's binding when both match. Once the list lets the key go, GPUI
/// tries the next binding, which is how the arrow keys reach the volume and seek shortcuts.
/// `overrides` holds the keys the user chose for shortcuts under Settings, Keybinds, by
/// shortcut id. A shortcut missing from it keeps its defaults, and an empty list unbinds it.
pub fn bindings(overrides: &BTreeMap<String, Vec<String>>) -> Vec<KeyBinding> {
    let mut bindings = shortcuts::bindings(overrides);
    bindings.extend(shared());
    if cfg!(target_os = "macos") {
        bindings.extend(macos());
    }
    bindings.extend(vim(overrides));
    bindings
}

/// Vim's motions, and Vimium's for moving around the app. On every list j and k step, gg
/// and G jump to the ends, Ctrl+D and Ctrl+U move half a page, and n and N step through what
/// `/` left in the list. Away from text, H and L go back and forward, J and K move down and up
/// the sidebar, g with a letter opens a page and ? lists every key. In the search field and the
/// quick search, Ctrl+J, Ctrl+N and Ctrl+P step through the results without leaving the field,
/// as fzf and Telescope do; Ctrl+K stays the quick search's own toggle. The search results take
/// h and l for the columns and / or i to type again. The quick search has a normal mode of its
/// own: Escape leaves the field for the list, where these keys work, and i or / go back to
/// typing. They come last, so they win over the Mac's Emacs keys in a field of the search page
/// or the quick search. A key the user gave a shortcut under Settings, Keybinds is left to it.
fn vim(overrides: &BTreeMap<String, Vec<String>>) -> Vec<KeyBinding> {
    let table = Some(TABLE_CONTEXT);
    let browsing = shortcuts::browsing_context();
    let results = format!("{SEARCH_CONTEXT} && !{INPUT_CONTEXT}");
    let typing = format!("{SEARCH_CONTEXT} > {INPUT_CONTEXT}");
    let quick_typing = format!("{POWERBAR_CONTEXT} > {INPUT_CONTEXT}");
    let quick_normal = format!("{POWERBAR_CONTEXT} && !{INPUT_CONTEXT}");
    let pages = format!(
        "{WORKSPACE_CONTEXT} && !{INPUT_CONTEXT} && !{MENU_CONTEXT} && !{FORM_CONTEXT} && !{COVER_VIEWER_CONTEXT}"
    );

    let mut keys = VimKeys::new(overrides);
    for context in [
        table,
        Some(browsing.as_str()),
        Some(&results),
        Some(&quick_normal),
    ] {
        keys.bind("j", SelectNext, context);
        keys.bind("k", SelectPrevious, context);
        keys.bind("n", SelectNext, context);
        keys.bind("shift-n", SelectPrevious, context);
        keys.bind("g g", SelectFirst, context);
        keys.bind("shift-g", SelectLast, context);
        keys.bind("home", SelectFirst, context);
        keys.bind("end", SelectLast, context);
        keys.bind("ctrl-d", SelectPageDown, context);
        keys.bind("ctrl-u", SelectPageUp, context);
        keys.bind("pagedown", SelectPageDown, context);
        keys.bind("pageup", SelectPageUp, context);
    }
    for context in [Some(typing.as_str()), Some(&quick_typing)] {
        keys.bind("ctrl-j", SelectNext, context);
        keys.bind("ctrl-n", SelectNext, context);
        keys.bind("ctrl-p", SelectPrevious, context);
    }
    keys.bind("h", SelectLeft, Some(&results));
    keys.bind("l", SelectRight, Some(&results));
    keys.bind("o", Activate, Some(&results));
    keys.bind("/", FocusField, Some(&results));
    keys.bind("i", FocusField, Some(&results));
    keys.bind("a", FocusField, Some(&results));
    keys.bind("o", Activate, table);
    keys.bind("enter", Submit, Some(&quick_normal));
    keys.bind("o", PowerbarDetails, Some(&quick_normal));
    keys.bind("i", FocusField, Some(&quick_normal));
    keys.bind("a", FocusField, Some(&quick_normal));
    keys.bind("/", FocusField, Some(&quick_normal));
    keys.bind("q", Dismiss, Some(&quick_normal));
    keys.bind("escape", Dismiss, Some(&quick_normal));
    // Enter in the field `/` opens keeps the filter and goes back to the list, as Vim's search
    // does, so n and N walk the matches.
    let filtering = format!("{TOOLBAR_CONTEXT} > {INPUT_CONTEXT}");
    keys.bind("enter", Submit, Some(&filtering));

    let pages = Some(pages.as_str());
    keys.bind("shift-h", NavigateBack, pages);
    keys.bind("shift-l", NavigateForward, pages);
    keys.bind("shift-j", NextSection, pages);
    keys.bind("shift-k", PreviousSection, pages);
    keys.bind("g t", NextSection, pages);
    keys.bind("g shift-t", PreviousSection, pages);
    keys.bind("g h", GoHome, pages);
    keys.bind("g s", OpenSearch, pages);
    keys.bind("g r", GoHistory, pages);
    keys.bind("g l", GoSongs, pages);
    keys.bind("g a", GoAlbums, pages);
    keys.bind("g shift-a", GoArtists, pages);
    keys.bind("g p", GoPlaylists, pages);
    keys.bind("?", ShowKeys, pages);
    keys.bind("shift-/", ShowKeys, pages);
    keys.bind("?", ShowKeys, Some(&quick_normal));
    for digit in "0123456789.".chars() {
        keys.bind(&digit.to_string(), TypeDigit(digit), pages);
    }
    keys.bindings
}

/// The vim bindings being gathered, less any the user's own keys under Settings, Keybinds
/// would collide with, so a key chosen there is never quietly taken over. A single key the
/// user bound takes every vim binding that starts with it; a sequence takes only itself.
struct VimKeys {
    taken: Vec<Vec<gpui::Keystroke>>,
    bindings: Vec<KeyBinding>,
}

impl VimKeys {
    fn new(overrides: &BTreeMap<String, Vec<String>>) -> Self {
        let taken = overrides
            .values()
            .flatten()
            .filter_map(|keys| parse(keys))
            .collect();
        Self {
            taken,
            bindings: Vec::new(),
        }
    }

    fn bind<A: gpui::Action>(&mut self, keys: &str, action: A, context: Option<&str>) {
        let Some(ours) = parse(keys) else {
            return;
        };
        let collides = self.taken.iter().any(|theirs| match theirs.len() {
            1 => ours.first() == theirs.first(),
            _ => *theirs == ours,
        });
        if !collides {
            self.bindings.push(KeyBinding::new(keys, action, context));
        }
    }
}

/// The keystrokes of a binding spelled like `g shift-a`, or `None` if one does not parse.
fn parse(keys: &str) -> Option<Vec<gpui::Keystroke>> {
    keys.split(' ')
        .map(|key| gpui::Keystroke::parse(key).ok())
        .collect()
}

/// The fixed bindings every platform gets: quitting, equalizer undo, lists, menus, search, forms
/// and text editing. Both `cmd-` and `ctrl-` are registered for each shortcut.
fn shared() -> Vec<KeyBinding> {
    let editing = Some(INPUT_CONTEXT);
    let away_from_text = format!("{WORKSPACE_CONTEXT} && !{INPUT_CONTEXT}");
    let table = Some(TABLE_CONTEXT);
    let form = Some(FORM_CONTEXT);
    let menu = Some(MENU_CONTEXT);
    let search = Some(SEARCH_CONTEXT);
    let browsing = shortcuts::browsing_context();
    let results = format!("{SEARCH_CONTEXT} && !{INPUT_CONTEXT}");
    let typing = format!("{SEARCH_CONTEXT} > {INPUT_CONTEXT}");

    vec![
        KeyBinding::new("down", SelectNext, table),
        KeyBinding::new("up", SelectPrevious, table),
        KeyBinding::new("shift-down", SelectNext, table),
        KeyBinding::new("shift-up", SelectPrevious, table),
        KeyBinding::new("enter", Activate, table),
        KeyBinding::new("delete", Remove, table),
        KeyBinding::new("escape", Deselect, table),
        KeyBinding::new("alt-up", MoveRowUp, table),
        KeyBinding::new("alt-down", MoveRowDown, table),
        KeyBinding::new("down", SelectNext, Some(&browsing)),
        KeyBinding::new("up", SelectPrevious, Some(&browsing)),
        KeyBinding::new("shift-down", SelectNext, Some(&browsing)),
        KeyBinding::new("shift-up", SelectPrevious, Some(&browsing)),
        KeyBinding::new("enter", Activate, Some(&browsing)),
        KeyBinding::new("delete", Remove, Some(&browsing)),
        KeyBinding::new("escape", Deselect, Some(&browsing)),
        KeyBinding::new("down", SelectNext, search),
        KeyBinding::new("up", SelectPrevious, search),
        KeyBinding::new("left", SelectLeft, Some(&results)),
        KeyBinding::new("right", SelectRight, Some(&results)),
        KeyBinding::new("enter", Activate, Some(&results)),
        KeyBinding::new("escape", Deselect, Some(&results)),
        KeyBinding::new("enter", Submit, Some(&typing)),
        KeyBinding::new("down", SelectNext, menu),
        KeyBinding::new("up", SelectPrevious, menu),
        KeyBinding::new("tab", SelectNext, menu),
        KeyBinding::new("shift-tab", SelectPrevious, menu),
        KeyBinding::new("enter", Submit, menu),
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("ctrl-q", Quit, None),
        KeyBinding::new("down", SelectNext, Some(POWERBAR_CONTEXT)),
        KeyBinding::new("up", SelectPrevious, Some(POWERBAR_CONTEXT)),
        KeyBinding::new("enter", Submit, Some(POWERBAR_CONTEXT)),
        KeyBinding::new("ctrl-enter", PowerbarDetails, Some(POWERBAR_CONTEXT)),
        KeyBinding::new("cmd-enter", PowerbarDetails, Some(POWERBAR_CONTEXT)),
        KeyBinding::new("cmd-z", Undo, Some(&away_from_text)),
        KeyBinding::new("ctrl-z", Undo, Some(&away_from_text)),
        KeyBinding::new("shift-cmd-z", Redo, Some(&away_from_text)),
        KeyBinding::new("shift-ctrl-z", Redo, Some(&away_from_text)),
        KeyBinding::new("ctrl-y", Redo, Some(&away_from_text)),
        KeyBinding::new("escape", Dismiss, Some(WORKSPACE_CONTEXT)),
        KeyBinding::new("backspace", Backspace, editing),
        KeyBinding::new("ctrl-backspace", BackspaceWord, editing),
        KeyBinding::new("delete", Delete, editing),
        KeyBinding::new("ctrl-delete", DeleteWord, editing),
        KeyBinding::new("left", Left, editing),
        KeyBinding::new("right", Right, editing),
        KeyBinding::new("ctrl-left", WordLeft, editing),
        KeyBinding::new("ctrl-right", WordRight, editing),
        KeyBinding::new("shift-left", SelectLeft, editing),
        KeyBinding::new("shift-right", SelectRight, editing),
        KeyBinding::new("shift-ctrl-left", SelectWordLeft, editing),
        KeyBinding::new("shift-ctrl-right", SelectWordRight, editing),
        KeyBinding::new("home", Home, editing),
        KeyBinding::new("end", End, editing),
        KeyBinding::new("cmd-left", Home, editing),
        KeyBinding::new("cmd-right", End, editing),
        KeyBinding::new("shift-home", SelectHome, editing),
        KeyBinding::new("shift-end", SelectEnd, editing),
        KeyBinding::new("shift-cmd-left", SelectHome, editing),
        KeyBinding::new("shift-cmd-right", SelectEnd, editing),
        KeyBinding::new("cmd-a", SelectAll, editing),
        KeyBinding::new("ctrl-a", SelectAll, editing),
        KeyBinding::new("cmd-v", Paste, editing),
        KeyBinding::new("ctrl-v", Paste, editing),
        KeyBinding::new("cmd-c", Copy, editing),
        KeyBinding::new("ctrl-c", Copy, editing),
        KeyBinding::new("cmd-x", Cut, editing),
        KeyBinding::new("ctrl-x", Cut, editing),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, editing),
        KeyBinding::new("space", Space, editing),
        KeyBinding::new("escape", Dismiss, editing),
        KeyBinding::new("enter", Submit, form),
        KeyBinding::new("escape", Dismiss, form),
        KeyBinding::new("tab", NextField, form),
        KeyBinding::new("shift-tab", PreviousField, form),
        KeyBinding::new("+", ZoomIn, Some(COVER_VIEWER_CONTEXT)),
        KeyBinding::new("=", ZoomIn, Some(COVER_VIEWER_CONTEXT)),
        KeyBinding::new("-", ZoomOut, Some(COVER_VIEWER_CONTEXT)),
        KeyBinding::new("0", ZoomReset, Some(COVER_VIEWER_CONTEXT)),
    ]
}

/// What a Mac user expects on top of the shared set: the standard window and application
/// shortcuts, and the Cocoa text-field conventions, `alt` for words, `cmd` for the line and the
/// Emacs control keys.
fn macos() -> Vec<KeyBinding> {
    let editing = Some(INPUT_CONTEXT);

    vec![
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-m", MinimizeWindow, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("alt-left", WordLeft, editing),
        KeyBinding::new("alt-right", WordRight, editing),
        KeyBinding::new("shift-alt-left", SelectWordLeft, editing),
        KeyBinding::new("shift-alt-right", SelectWordRight, editing),
        KeyBinding::new("alt-backspace", BackspaceWord, editing),
        KeyBinding::new("alt-delete", DeleteWord, editing),
        KeyBinding::new("cmd-backspace", BackspaceToStart, editing),
        KeyBinding::new("cmd-delete", DeleteToEnd, editing),
        KeyBinding::new("cmd-up", Home, editing),
        KeyBinding::new("cmd-down", End, editing),
        KeyBinding::new("shift-cmd-up", SelectHome, editing),
        KeyBinding::new("shift-cmd-down", SelectEnd, editing),
        KeyBinding::new("ctrl-a", Home, editing),
        KeyBinding::new("ctrl-e", End, editing),
        KeyBinding::new("ctrl-b", Left, editing),
        KeyBinding::new("ctrl-f", Right, editing),
        KeyBinding::new("ctrl-h", Backspace, editing),
        KeyBinding::new("ctrl-d", Delete, editing),
        KeyBinding::new("ctrl-k", DeleteToEnd, editing),
    ]
}
