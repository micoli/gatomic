//! Minimal i18n: a `Strings` table of every UI-facing string, with one
//! per-language file providing a `'static` instance. Dynamic strings keep
//! `{placeholder}` tokens the caller fills with `str::replace` — there's no
//! runtime `format!`, so templates stay plain data per language.

mod en;
mod fr;

/// UI language. English is the default so the TUI is usable out of the box
/// regardless of the user's locale; `l` cycles through the embedded
/// languages at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Fr,
}

impl Lang {
    pub fn cycle(self) -> Self {
        match self {
            Lang::En => Lang::Fr,
            Lang::Fr => Lang::En,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "EN",
            Lang::Fr => "FR",
        }
    }

    pub fn strings(self) -> &'static Strings {
        match self {
            Lang::En => &en::EN,
            Lang::Fr => &fr::FR,
        }
    }
}

pub struct Strings {
    pub files_title: &'static str,
    pub hunks_title: &'static str,
    /// `{path}` placeholder.
    pub hunks_title_with_path: &'static str,
    pub commits_title: &'static str,
    pub commit_show_title: &'static str,
    /// `{err}` placeholder.
    pub show_error: &'static str,

    pub help_switch_pane: &'static str,
    pub help_navigate: &'static str,
    pub help_reopen_triage: &'static str,
    pub help_new_commit: &'static str,
    pub help_quit: &'static str,
    pub help_hunk_accept_reject_advance: &'static str,
    pub help_hunk_accept_reject_rest: &'static str,
    pub help_hunk_next_prev: &'static str,
    pub help_hunk_next_prev_undecided: &'static str,
    pub help_hunk_split: &'static str,
    pub help_hunk_toggle: &'static str,
    pub help_commit_fixup: &'static str,
    pub help_commit_green_check: &'static str,
    pub help_language: &'static str,
    pub help_popup_title: &'static str,

    /// Compact counterparts of the `help_*` lines above, used in the
    /// bottom help bar (always on screen, so it must fit on one line);
    /// the popup opened with `?` still shows the full-length versions.
    pub short_switch_pane: &'static str,
    pub short_navigate: &'static str,
    pub short_reopen_triage: &'static str,
    pub short_new_commit: &'static str,
    pub short_quit: &'static str,
    pub short_hunk_accept_reject_advance: &'static str,
    pub short_hunk_accept_reject_rest: &'static str,
    pub short_hunk_next_prev: &'static str,
    pub short_hunk_next_prev_undecided: &'static str,
    pub short_hunk_split: &'static str,
    pub short_hunk_toggle: &'static str,
    pub short_commit_fixup: &'static str,
    pub short_commit_green_check: &'static str,
    pub short_language: &'static str,

    pub commit_form_title: &'static str,
    pub commit_form_diff_title: &'static str,
    pub commit_form_validate: &'static str,
    pub commit_form_cancel: &'static str,

    /// `{n}` placeholder.
    pub status_fixup_created: &'static str,
    pub status_all_hunks_decided: &'static str,
    pub status_review_complete: &'static str,
    pub status_hunk_not_splittable: &'static str,
    /// `{n}` placeholder.
    pub status_hunk_split: &'static str,
    pub status_empty_commit_message: &'static str,
    pub status_new_commit_created: &'static str,

    pub triage_list_title: &'static str,
    pub triage_select_file_prompt: &'static str,
    pub triage_file_diff_title: &'static str,
    pub triage_help: &'static str,
}
