#pragma once

// Follow uQ's theme: the KDE colour scheme, icon theme and interface font uqd renders into
// `$XDG_STATE_HOME/uq/theme`. Applied once at startup and again whenever those files change.
// Without uQ (no such folder) the desktop's own theme stays as it is.
// Same approach as hauler's window (hauler-desktop/cpp/theme.cpp): Kirigami caches a colour
// scheme by its path, so kdeglobals changing under it isn't enough to recolour a live window.
void notifi_follow_uq_theme();
