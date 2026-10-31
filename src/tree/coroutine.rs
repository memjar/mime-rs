//! # Coroutine contract
//!
//! The generator shape of the crate's I/O-free coroutines, as io-imap's: a
//! [`resume`](crate::tree::parse::MimeParse::resume) either yields a request
//! the caller answers with any code it likes, or completes.

/// Result of one coroutine resume.
#[derive(Debug)]
pub enum MimeCoroutineState<Y, R> {
    /// The coroutine needs an answer before it can progress.
    Yielded(Y),
    /// The coroutine is done; resuming it again yields nothing useful.
    Complete(R),
}
