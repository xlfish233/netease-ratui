---
name: ui-worker
description: UI 层交互改造 worker，处理键盘/鼠标/菜单/进度条/Toast 相关功能
---

# UI Worker

NOTE: Startup and cleanup are handled by `worker-base`. This skill defines the WORK PROCEDURE.

## When to Use This Skill

所有涉及 UI 交互改造的 feature：键盘处理、鼠标支持、Toast 改造、操作菜单、进度条、分页。

## Required Skills

None. 所有工作通过 `cargo test`/`cargo clippy`/`cargo fmt` 完成。

## Work Procedure

1. **Read context**: Read `mission.md`, `AGENTS.md`, and `.factory/library/architecture.md` for full context.
2. **Write tests FIRST (TDD)**:
   - Identify the assertions from `validation-contract.md` that this feature fulfills.
   - Write failing unit tests that verify the expected behavior.
   - Tests go in `#[cfg(test)]` modules within the relevant source files (follow project convention).
   - For keyboard tests: construct `AppSnapshot`, `KeyEvent`, call `handle_key()`, assert on `rx.try_recv()`.
   - For mouse tests: construct `AppSnapshot`, `MouseEvent`, call `handle_mouse()`, assert on commands.
   - For reducer tests: construct `CoreState`, call handler function, assert on state changes and effects.
   - For render tests: use `ratatui::TestBackend`, call draw functions, assert on buffer content.
3. **Implement the feature**:
   - Modify the appropriate files in `src/ui/tui/`, `src/messages/app.rs`, `src/app/state.rs`, `src/core/reducer/`.
   - Add new `AppCommand` variants to `src/messages/app.rs` as needed.
   - Add new `App`/`AppSnapshot` fields as needed.
   - Follow existing code patterns and style.
4. **Run tests**: `cargo test` — all tests must pass.
5. **Run lint**: `cargo clippy --all-targets -- -D warnings` — no warnings.
6. **Run format**: `cargo fmt --check` — no issues.
7. **Verify manually** (if possible): `cargo run` and test the interaction.
8. **Commit** with descriptive message.

## Example Handoff

```json
{
  "salientSummary": "Implemented Space key conflict fix in search mode. When search input has focus, Space sends SearchInputChar instead of PlayerTogglePause. Added 5 unit tests in keyboard.rs covering all View/focus combinations. All tests pass, clippy clean.",
  "whatWasImplemented": "Modified handle_key() in keyboard.rs to check for Search+HeaderSearch focus before triggering PlayerTogglePause on Space. Added unit tests for Space behavior in Search/Playlists/Lyrics views with different focus states.",
  "whatWasLeftUndone": "",
  "verification": {
    "commandsRun": [
      { "command": "cargo test -- keyboard", "exitCode": 0, "observation": "12 tests passed, 0 failed" },
      { "command": "cargo clippy --all-targets -- -D warnings", "exitCode": 0, "observation": "No warnings" },
      { "command": "cargo fmt --check", "exitCode": 0, "observation": "Formatted correctly" }
    ],
    "interactiveChecks": []
  },
  "tests": {
    "added": [
      {
        "file": "src/ui/tui/keyboard.rs",
        "cases": [
          { "name": "space_in_search_input_sends_char", "verifies": "VAL-SPACE-001" },
          { "name": "space_in_search_input_not_toggle", "verifies": "VAL-SPACE-002" },
          { "name": "space_in_search_body_center_is_toggle", "verifies": "VAL-SPACE-003" },
          { "name": "space_in_playlists_is_toggle", "verifies": "VAL-SPACE-004" },
          { "name": "space_in_lyrics_is_toggle", "verifies": "VAL-SPACE-005" }
        ]
      }
    ]
  },
  "discoveredIssues": []
}
```

## When to Return to Orchestrator

- Need to add AppCommand variants that affect multiple feature modules
- Need architectural guidance on new module structure (e.g., keybindings/)
- Tests reveal issues with existing code that need separate fix features
- Build or test infrastructure issues
