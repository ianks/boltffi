// boltffi_bindgen/src/render/ruby/mod.rs
mod emit;
mod lower;
mod naming;
mod plan;
mod templates;

pub use emit::{RubyEmitter, RubyOutputFile, RubyPackageSources};
pub use lower::RubyLowerer;
pub use plan::RubyModule;
