mod effects;
mod reducer;

pub mod infra;

pub mod utils;

pub mod prelude;

// 公共导出
#[allow(unused_imports)]
pub use effects::{CoreDispatch, CoreEffect, CoreEffects};
pub use reducer::spawn_app_actor;
