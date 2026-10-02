//! Bounded selected-tool requests and raw scratch ELF proof inputs.
//!
//! Loading owns the normal checkpoint/consumer dependencies. These structural
//! requests and proof values confer no native source or process authority.
//! Only compact selector keys enter DICE's engine-lifetime key catalogue; full
//! prefix/blob pins remain credited values and are released at actual last Drop.

mod observation;
mod request;
mod types;

pub use observation::*;
pub use request::*;
pub use types::*;

#[cfg(test)]
mod tests;
