# How to add or change a Tauri command

Every command the frontend can call into Rust is defined once, in `src-tauri/src/lib.rs`, and the typed TypeScript wrapper in `frontend/src/lib/bindings.ts` is **generated** from it — never hand-edited. This guide covers both adding a brand-new command and changing an existing one's signature.

## Prerequisites

- A working `cargo build` in `src-tauri/`
- Understand which state the command needs — most commands take a `tauri::State<...>` parameter for whatever they read or mutate (see existing commands in `lib.rs` for the pattern)

## Steps

1. **Write the command function** in `src-tauri/src/lib.rs`, following the existing pattern:

   ```rust
   #[tauri::command]
   #[specta::specta]
   fn my_new_command(some_arg: String, state: tauri::State<SomeType>) -> Result<MyDto, String> {
       // ...
       Ok(result)
   }
   ```

   - `Result<T, String>` if the command can meaningfully fail — the frontend gets `{status: "ok"|"error", ...}`, not a thrown exception (see [`reference-commands.md`](reference-commands.md)'s "Calling convention" for why this matters on the frontend side).
   - A bare return type (no `Result`) only for commands that genuinely cannot fail in a way the caller needs to handle.
   - `async fn` + `tauri::async_runtime::spawn_blocking(...)` for anything that blocks — a plain sync command runs on the same thread that dispatches the IPC message (the main/UI thread), so a blocking call there stalls the whole UI. `open_permission_settings` and `request_permission` in `lib.rs` are the reference examples.

2. **If it takes or returns a custom struct**, derive `specta::Type` on it (alongside `serde::Serialize`/`Deserialize` as needed):

   ```rust
   #[derive(serde::Serialize, specta::Type)]
   struct MyDto {
       field: String,
   }
   ```

   If the struct already exists as a domain type elsewhere (e.g. in `history.rs`) with fields that shouldn't be exposed as-is, write a small `...Dto` wire-format twin with a `From<DomainType>` impl — see `MetricsDto`/`LatencyStatsDto` in `lib.rs` for the pattern. Don't derive `specta::Type` directly on an internal domain type just to avoid writing the twin.

3. **Register it** in `specta_builder()`'s `collect_commands!` list *and* nowhere else — `invoke_handler(specta_builder.invoke_handler())` in `run()` picks up everything registered there automatically; there's no separate `generate_handler!` list to keep in sync.

   ```rust
   tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
       // ...existing commands...
       my_new_command,
   ])
   ```

4. **Regenerate the frontend bindings** — this is a `cargo test`, not something that runs automatically or at app startup:

   ```bash
   cd src-tauri
   cargo test --lib export_bindings -- --ignored
   ```

   This rewrites `frontend/src/lib/bindings.ts` in place. Do **not** run this from inside a launched `.app` bundle — exporting from the real app (even a debug build) triggers a macOS TCC "would like to access files in your Documents folder" consent dialog that blocks the window from rendering. `cargo test` runs as a plain CLI process and never hits this.

5. **Call it from the frontend**:

   ```ts
   const res = await commands.myNewCommand("some arg");
   if (res.status === "error") { /* handle res.error */ }
   ```

   The generated name is the camelCase version of the Rust function name.

## Verification

```bash
cd src-tauri && cargo build && cargo clippy --all-targets && cargo fmt --check
cd ../frontend && npm run build
```

`npm run build` runs `tsc -b` first — a mismatch between what you call from the frontend and what the regenerated `bindings.ts` actually exports fails here with a real TypeScript error, not a runtime surprise.

## Troubleshooting

- **`bindings.ts` didn't change after running the export test** — check the test actually ran (`cargo test --lib export_bindings -- --ignored` needs the exact `--ignored` flag; it's skipped by a plain `cargo test`) and that you registered the command in `collect_commands!` (step 3).
- **Frontend TypeScript can't find `commands.myNewCommand`** — same as above, or you named the Rust function something the camelCase conversion doesn't map the way you expect; check the actual generated name in `bindings.ts`.
- **The app silently does nothing when you call the new command** — check `res.status === "error"` is actually being checked; a `Result<T, String>` command's `Err` resolves normally, it does not throw (see [`reference-commands.md`](reference-commands.md)).

## Related

- [`reference-commands.md`](reference-commands.md) — every existing command, for reference patterns
- [How to add a new Settings toggle](howto-add-a-settings-toggle.md) — a common special case of this (reuses `set_bool_setting`, no new command needed)
