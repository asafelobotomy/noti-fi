.pragma library

// The model hands most of its data to QML as JSON text. parse() reads it, with a fallback for empty or broken text,
// so every page reads it the same way.
function parse(text, fallback) {
    if (typeof text !== "string" || text === "") return fallback
    try { return JSON.parse(text) } catch (e) { return fallback }
}
