.pragma library

// For controls whose label cannot be set to plain text (buttons, list rows): Qt would read text that looks like HTML as rich
// text, so a notification's app name could load a remote image. Angle brackets become look-alike quotes, so it is never HTML.
function label(t) {
    return String(t ?? "").replace(/</g, "‹").replace(/>/g, "›")
}
