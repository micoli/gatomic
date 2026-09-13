use super::Strings;

pub static FR: Strings = Strings {
    files_title: "Fichiers",
    hunks_title: "Hunks",
    hunks_title_with_path: "Hunks — {path}",
    commits_title: "Commits (x = fixup)",
    commit_show_title: "git show",
    show_error: "erreur : {err}",

    help_switch_pane: "Tab / Shift+Tab : changer de pane",
    help_navigate: "Haut/Bas : naviguer",
    help_reopen_triage: "t : rouvrir le triage des associations évidentes",
    help_new_commit: "c : nouveau commit avec les hunks stagés",
    help_quit: "q / Esc / Ctrl+C : quitter",
    help_hunk_accept_reject_advance: "y / n : accepter / rejeter ce hunk et avancer",
    help_hunk_accept_reject_rest: "a / d : accepter / rejeter ce hunk et tout le reste du fichier",
    help_hunk_next_prev: "j / k : hunk suivant / précédent (sans décider)",
    help_hunk_next_prev_undecided: "J / K : prochain / précédent hunk non décidé",
    help_hunk_split: "s : découper ce hunk",
    help_hunk_toggle: "Espace/Entrée : toggle sélection (hunk ou ligne)",
    help_commit_fixup: "x : git commit --fixup sur le commit sélectionné",
    help_commit_green_check: "✓ vert : le fichier sélectionné appartient à ce commit",
    help_language: "l : changer de langue",
    help_popup_title: "Aide (touche quelconque pour fermer)",

    commit_form_title: "Nouveau commit (Entrée : nouvelle ligne, Ctrl+Entrée : valider, Esc : annuler)",
    commit_form_diff_title: "Diff du commit",
    commit_form_validate: "Valider",
    commit_form_cancel: "Annuler",

    status_fixup_created: "{n} commit(s) fixup créé(s)",
    status_all_hunks_decided: "Tous les hunks sont décidés",
    status_review_complete: "Revue terminée pour tous les fichiers",
    status_hunk_not_splittable: "Hunk non divisible",
    status_hunk_split: "Hunk découpé en {n} parties",
    status_empty_commit_message: "Message de commit vide, commit annulé",
    status_new_commit_created: "Nouveau commit créé",

    triage_list_title: "Associations évidentes fichier -> commit",
    triage_select_file_prompt: "Sélectionnez un fichier pour voir son diff.",
    triage_file_diff_title: "Diff du fichier sélectionné",
    triage_help: "Tab : changer de pane   Espace : inclure/exclure   a : valider en lot   Entrée/d/t/Esc : revue détaillée",
};
