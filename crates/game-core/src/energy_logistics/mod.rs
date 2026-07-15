//! Physical Energy logistics domain.
//!
//! Root session orchestration remains authoritative for tick scheduling. Once
//! contracts are implemented, only this module's contract executor may mutate
//! Energy logistics state.

#[cfg(test)]
mod tests;
