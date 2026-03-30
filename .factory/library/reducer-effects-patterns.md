# Reducer & Effects Patterns

## SetToast Effect Handling

When a reducer calls `effects.set_toast()`, the toast is NOT immediately available on `state.app.toast`. The toast is applied in the main reducer loop (`src/core/reducer.rs` spawn_app_actor) AFTER the reducer returns but BEFORE `run_effects()` is called:

```rust
// src/core/reducer.rs:223-226
for effect in &effects.actions {
    if let CoreEffect::SetToast(toast) = effect {
        state.app.toast = Some(toast.clone());
    }
}
```

Workers writing reducers should be aware that `app.toast` reflects the current state BEFORE the reducer's own `SetToast` effect. Do not read `app.toast` after calling `set_toast()` expecting it to contain the new toast within the same reducer invocation.
