import QtQuick

// Finds the audio or video file a notification wants played. Apps that want a player set one of the hints x-notify-video, x-notify-audio
// or x-notify-media to a local file; x-kde-urls (the files a notification is about) is honoured too. Anything that is not an absolute local
// path to a known audio or video type is ignored, so a notification can never make a popup fetch a URL.
QtObject {
    function kindOf(path) {
        const ext = (String(path).split("?")[0].split(".").pop() || "").toLowerCase()
        if (["mp4", "mkv", "webm", "mov", "avi", "m4v", "ogv"].indexOf(ext) >= 0) return "video"
        if (["mp3", "ogg", "oga", "opus", "flac", "wav", "m4a", "aac"].indexOf(ext) >= 0) return "audio"
        return ""
    }
    function local(v) {
        let u = String(v || "")
        if (u.startsWith("file://")) u = decodeURIComponent(u.slice(7))
        if (!u.startsWith("/") || u.indexOf("\u0000") >= 0) return null
        const kind = kindOf(u)
        return kind === "" ? null : { kind: kind, url: "file://" + encodeURI(u) }
    }
    // { kind: "audio" | "video", url } or null
    function fileFor(h) {
        const list = []
        for (const k of ["x-notify-video", "x-notify-audio", "x-notify-media"]) if (h[k]) list.push(h[k])
        const urls = h["x-kde-urls"]
        if (urls) for (const u of (Array.isArray(urls) ? urls : String(urls).split(","))) list.push(u)
        for (const c of list) { const m = local(c); if (m) return m }
        return null
    }
}
