#pragma once

// Follow uQ's theme: the KDE colour scheme, icon theme and interface font uqd renders into
// `$XDG_STATE_HOME/uq/theme`. Applied once at startup and again whenever those files change.
// Without uQ (no such folder) the desktop's own theme stays as it is.
// The work is uQ's shared uqkdetheme (cpp/uqkdetheme.*, from uQ's native/uq-theme), which hauler
// uses too: Kirigami caches a colour scheme by its path, so each change is handed over as a fresh
// copy.
void notifi_follow_uq_theme();
