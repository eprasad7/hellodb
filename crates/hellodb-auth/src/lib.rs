//! hellodb Authorization Layer
//!
//! Consent proofs, delegation credentials, and namespace access control.
//! Ensures apps only access their own namespace, and cross-namespace
//! reads require cryptographic consent.

pub mod consent;
pub mod delegation;
pub mod access;
pub mod error;

pub use consent::{ConsentAction, ConsentProof};
pub use delegation::{DelegationCredential, DelegationScope};
pub use access::{AccessDecision, AccessGate};
pub use error::AuthError;
