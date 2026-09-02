# Tauri command reference

The complete IPC surface between the frontend and the Rust backend. Every command below is defined as a `#[tauri::command]` in `src-tauri/src/lib.rs` and exposed to the frontend as a typed async function on the `commands` object exported from `frontend/src/lib/bindings.ts` — a **generated** file (see [How to add or change a Tauri command](howto-add-a-tauri-command.md) for the regeneration step; never hand-edit it).

## Calling convention

Every command is called as `commands.<camelCaseName>(...)` and returns a `Promise`. Two return shapes exist, and the difference matters:

- **Commands that return `Result<T, String>` on the Rust side** resolve to `{ status: "ok", data: T } | { status: "error", error: string }` — a Rust `Err` does **not** throw. You must check `res.status === "error"` explicitly; a bare `await commands.x()` with no status check silently swallows failures.
- **Commands that return a bare `T`** (no `Result`) resolve directly to `T` and can throw only on a genuine transport error.

```ts
const res = await commands.getMetrics();
if (res.status === "error") {
  // handle res.error (a string)
} else {
  // use res.data
}
```

## Commands

### Metrics & history (dashboard Stats/History panels)

| Command | Signature | Returns | Notes |
|---|---|---|---|
| `getMetrics` | `()` | `Result<MetricsDto, string>` | `sessions`, `words`, `average_wpm`, `time_saved_minutes`, `total_dictation_minutes` — from `HistoryStore::metrics()`'s running aggregate, not a table scan |
| `getLanguageBreakdown` | `()` | `Result<LanguageStatDto[], string>` | Per-language count + average WPM, most-used first |
| `getDailyActivity` | `(days: number)` | `Result<DailyActivityDto[], string>` | Session counts per local-timezone calendar day, oldest first, bounded to the trailing `days`-day window |
| `getLatencyStats` | `()` | `Result<LatencyStatsDto, string>` | p50/p95/sample-count for both `recording` and `inference` stages, trailing 14 days (`LATENCY_WINDOW_DAYS`) |
| `getHistoryPage` | `(page: number, pageSize: number)` | `Result<HistoryEntryDto[], string>` | Most-recent-first, paginated — never loads the whole table |
| `copyHistoryText` | `(text: string)` | `Result<null, string>` | Plain clipboard write, no paste — backs History's "copy" button |

### Permissions

| Command | Signature | Returns | Notes |
|---|---|---|---|
| `getPermissionStatus` | `()` | `PermissionStatusDto` (bare, not `Result`) | Live-queried mic/accessibility/system_audio status strings: `"not_requested" \| "denied" \| "granted" \| "unavailable"` |
| `openPermissionSettings` | `(kind: PermissionKind)` | `Result<null, string>` | Deep-links to System Settings' matching pane. `PermissionKind` = `"microphone" \| "accessibility" \| "screen_recording"` |
| `requestPermission` | `(kind: PermissionKind)` | `Result<boolean, string>` | Shows the real native active-request prompt for any of the three permission kinds — mic (`AVCaptureDevice`), Accessibility (`AXIsProcessTrustedWithOptions`), or Screen Recording (`CGRequestScreenCaptureAccess`) |

### Session control

| Command | Signature | Returns | Notes |
|---|---|---|---|
| `cancelRecording` | `()` | `Result<null, string>` | Manual equivalent of pressing Escape — used by the pill's cancel button, since a webview click can't itself register as a global-shortcut press |
| `quitApp` | `()` | `void` | `app.exit(0)` — backs the dashboard sidebar's quit button |

### Settings

| Command | Signature | Returns | Notes |
|---|---|---|---|
| `getSettings` | `()` | `AppSettings` (bare) | The full persisted settings struct |
| `setHotkey` | `(mode: string, shortcut: string)` | `Result<null, string>` | `mode` is `"mic"` or `"system_audio"`. Registers the new shortcut before unregistering the old one — a bad new spec leaves the old binding intact |
| `setBoolSetting` | `(field: SettingField, enabled: boolean)` | `Result<null, string>` | The one generic command backing all seven output toggles — see [`reference-settings.md`](reference-settings.md) |
| `setGrammarLlmCleanupEnabled` | `(enabled: boolean)` | `Result<null, string>` | Separate from `setBoolSetting` — predates it, kept as its own dedicated command |
| `setPillContentWidth` | `(width: number)` | `void` | Internal — the pill's `ResizeObserver` reports its own content width so the native window can resize to fit. Not something UI code should call directly |

### Onboarding & recovery

| Command | Signature | Returns | Notes |
|---|---|---|---|
| `completeOnboarding` | `()` | `Result<null, string>` | Persists `onboarding_completed = true`, closes the onboarding window, opens the dashboard |
| `getRecoveryInfo` | `()` | `string \| null` (bare) | The pre-migration backup path, only set when `HistoryStore::open()` returned `MigrationFailed` |

## Adding a new command

See [How to add or change a Tauri command](howto-add-a-tauri-command.md).

## Related

- [`reference-architecture.md`](reference-architecture.md) — where each command's backing module lives
- [`reference-settings.md`](reference-settings.md) — the `AppSettings`/`SettingField` shape `setBoolSetting` operates on
- [`reference-history-schema.md`](reference-history-schema.md) — what the metrics/history DTOs are computed from
