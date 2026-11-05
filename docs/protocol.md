# notifd control protocol (version 1)

How tools talk to the running daemon. The Rust types live in `crates/notify-protocol`, the blocking client in
`crates/notify-client`; the daemon side is the `api*` functions and the socket section of `notifd/shell.qml`.

## Transport

- A Unix socket at `$XDG_RUNTIME_DIR/notifd.sock` (override with `NOTIFD_SOCKET`). `$XDG_RUNTIME_DIR` is mode 0700, so
  only your user can connect.
- One JSON object per line (`\n`-terminated), UTF-8, in both directions. Requests larger than 1 MiB are refused.
- At most 16 simultaneous connections; the 17th receives an `error` event and is closed.

## Conversation

```
server -> client   {"event":"hello","protocol":1,"daemon":"0.8.0","pid":123,"seq":0}      (always first)
client -> server   {"id":1,"method":"status","params":{}}
server -> client   {"id":1,"result":{...}}
                   {"id":1,"error":{"code":"bad_params","message":"mode must be on, off or toggle"}}
```

- `id` is any number or string you choose and is echoed back. Answers arrive in request order.
- A client must check `hello.protocol` and refuse a daemon with a different number (`notify-client` does).
- After `subscribe`, the server also pushes **events** (below) interleaved with answers. Events have an `event` key;
  answers have an `id` key.

### Error codes

| code | meaning |
|---|---|
| `bad_request` | the line was not JSON, or lacked a string `method` (the connection stays open) |
| `too_large` | request over 1 MiB |
| `unknown_method` | no such method |
| `bad_params` | the method exists but the parameters were wrong (also: invalid draft config) |
| `not_found` | for example `reply` to a notification that is not on screen |
| `failed` | anything else |

## Methods

| method | params | result |
|---|---|---|
| `ping` | — | `{pong, t}` |
| `status` | — | `Status`: `dnd`, `muted[]`, `zones[]`, `active`, `queued`, `perZone{}`, `cap`, `history`, `cfgError`, `auto{fullscreen,share}`, `holding`, `previewing` |
| `configStatus` | — | `{ok, error, rules, zones[], screenFound}` |
| `configGet` | — | `{raw, effective, error}`: the file text, and the merged config the daemon runs |
| `dnd` | `{mode: "on"\|"off"\|"toggle"}` | `Status` |
| `mute` | `{zone, on}` | `Status` |
| `setAuto` | `{kind: "fullscreen"\|"share", on}` | `Status` (normally set by the watchers; for tests and tools) |
| `history` | `{limit}` (max 1000) | list of `{t,id,app,summary,body,zone,origin,urgency,shown,why,updates?}`, newest first |
| `focus` | `{mode}` (a mode name, or `off`) | `Status` (with `focus`); an unknown name is `bad_params` and lists the modes that exist |
| `stack` | `{zone, key, action: toggle\|clear\|scroll, delta?}` | `Status` (`stacks[]` lists stacks of 2+: `{zone,key,count,expanded}`) |
| `samples` | `{id}` | the transfer measurements of a notification on screen: `[{t, pct, bps}]` (`pct` -1 and `bps` null when unknown) |
| `seenApps` | — | list of `{key,app,desktopEntry,count,last,types[]}`: every app that ever sent a notification (kept across restarts, independent of history), newest first. `types[]` = `{key,category,channel,caps[],count,last}`: the kinds it has sent (category hint + channel `app`/`portal`/`web`; caps from actions, reply, image, icon, progress, resident, transient, low, critical, sound) |
| `clearHistory` | — | `Status` |
| `dismissAll` | — | `Status` |
| `reply` | `{id, text}` | `{ok}` (the app receives `NotificationReplied`) |
| `validate` | `{config: {...}}` or `{text: "..."}` | `{ok, error?, rules?, zones?}`; never applies anything |
| `simulate` | `{config?, notification:{appName,desktopEntry,origin,category,channel,summary,body,urgency,expireTimeout}, ignoreState?}` | `{ok, zone, timeout, silent, wouldShow, hold, matched[], decidedBy{zone,timeout,silent,+style keys}, style{opacity,clickThrough,noFocus,width,fontFamily,fontSize,group}}` |
| `preview` | `{config?, samples?[], seconds?}` | `{ok, shown, seconds}`; shows real popups, restores the real config afterwards |
| `endPreview` | — | `Status` |
| `subscribe` | — | `{status, seq}`: a snapshot, and the event number to continue from |
| `unsubscribe` | — | `{}` |

`simulate` and `preview` run through the same `decide()` function as real notifications, so the answer is what will happen.
`timeout` is milliseconds; `0` means the popup stays until dismissed. `hold` is why a notification would not be shown:
`dnd`, `fullscreen`, `screen sharing`, `muted <zone>`, `silent rule`, or empty.

## Events (after `subscribe`)

Every event has `seq` (strictly increasing, shared across all clients) and `t` (milliseconds since the epoch).

| event | when | payload |
|---|---|---|
| `state` | Do Not Disturb, mutes, counts, holds change (coalesced to at most one per 100 ms) | the same fields as `Status` |
| `notification` | a notification arrived, shown or held | `{id, app, summary, body (≤300 chars), zone, origin, urgency, shown, why, transient}` |
| `updated` | an app replaced a notification that is on screen | `{id, summary, body, zone}` |
| `closed` | a popup went away | `{id}` |
| `config` | rules.json was loaded or rejected | `{ok, error, rules, zones[]}` |
| `preview` | a preview started or ended | `{state: "started"\|"ended", shown?, seconds?}` |
| `error` | the daemon refuses this connection | `{code, message}` (no `seq`) |

To stay in sync across a reconnect: connect, `subscribe`, use the returned `status` as the baseline, then apply events
whose `seq` is greater than the returned `seq`.

## Compatibility

`qs ipc call notifd <fn>` keeps working unchanged (`status`, `configStatus`, `dnd`, `mute`, `setAuto`, `reply`, `validate`,
`simulate`, `preview`, `endPreview`, `history`, `clearHistory`, `dismissAll`); it shares the same implementation.
Adding methods or event fields is not a protocol change; renaming or removing them, or changing a meaning, bumps `protocol`.

## Tested

`tests/test_notifd.py` (socket, events, limits, stress: 1000 pipelined requests, 300 connect/disconnect cycles with no
fd or memory growth, 400-event flood with no loss, a client that never reads, p99 latency under six concurrent clients,
recovery from a hard-killed daemon with a stale socket file) and
`crates/notify-client/tests/contract.rs` (every message parses into the Rust types; the daemon accepts what Rust writes).
