//! `brewlike` — a package-manager query client over one frozen cellar.
//!
//! `cellar` owns the behaviour and knows nothing about the CLI; `view` owns
//! the serializable documents the machine surface is made of; `cli` is the
//! shell adapter — clap surface, handlers, and the configured `App`.

pub mod cellar;
pub mod cli;
pub mod view;
