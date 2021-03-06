//! An experimental frontend web framework in pure rust.

#![no_std]

extern crate alloc;

pub mod component;
pub mod error;
pub mod html;
pub mod store;
pub mod style;

pub mod prelude {
    //! Use `wasmide::prelude::*;` to import common stores, components, and styles.

    pub use crate::component::Component;
    pub use crate::html;
    pub use crate::store::{Store, Subscribable, Value};
    pub use crate::style::Style;
}