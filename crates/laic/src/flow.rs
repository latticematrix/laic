//! Credit-based flow control for LAIC transport.
//!
//! Provides a sender-side credit tracker that enforces backpressure.
//! The controller is **external** to `Transport` — callers decide which
//! messages require credit checks (data messages) and which are exempt
//! (control, heartbeat).

use crate::error::FlowError;

// ---------------------------------------------------------------------------
// CreditController
// ---------------------------------------------------------------------------

/// Sender-side credit tracker for backpressure enforcement.
///
/// WHY: external to Transport — control/heartbeat messages should not
/// be blocked by flow control. Keeping credits separate lets the caller
/// decide which messages consume credits (Mechanism not Policy).
///
/// CONSTRAINT: credits are `u16` (0..65535), matching the protocol
/// header's `credit_grant` field width.
///
/// # Usage
///
/// ```ignore
/// let mut credit = CreditController::new(100);
/// credit.acquire()?;                    // decrements by 1
/// transport.send(&msg).await?;
/// // ...
/// let reply = transport.receive().await?;
/// if reply.header().credit_grant > 0 {
///     credit.replenish(reply.header().credit_grant);
/// }
/// ```
pub struct CreditController {
    available: u16,
}

impl CreditController {
    /// Create a new controller with `initial_credits` available.
    #[must_use]
    pub const fn new(initial_credits: u16) -> Self {
        Self {
            available: initial_credits,
        }
    }

    /// Consume one credit. Returns `Err(FlowError::CreditExhausted)`
    /// if no credits remain.
    ///
    /// # Errors
    ///
    /// Returns [`FlowError::CreditExhausted`] when `available == 0`.
    /// Callers in a `Result<_, LaicError>` context can propagate with `?`
    /// — `From<FlowError>` converts automatically.
    pub fn acquire(&mut self) -> Result<(), FlowError> {
        if self.available == 0 {
            return Err(FlowError::CreditExhausted);
        }
        self.available -= 1;
        Ok(())
    }

    /// Add credits granted by the peer. Uses saturating addition to
    /// prevent overflow (`u16::MAX` is the protocol ceiling).
    pub fn replenish(&mut self, credits: u16) {
        self.available = self.available.saturating_add(credits);
    }

    /// Current number of available credits.
    #[must_use]
    pub const fn available(&self) -> u16 {
        self.available
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_initial_credits() {
        let c = CreditController::new(100);
        assert_eq!(c.available(), 100);
    }

    #[test]
    fn acquire_decrements() {
        let mut c = CreditController::new(3);
        assert!(c.acquire().is_ok());
        assert_eq!(c.available(), 2);
        assert!(c.acquire().is_ok());
        assert_eq!(c.available(), 1);
        assert!(c.acquire().is_ok());
        assert_eq!(c.available(), 0);
    }

    #[test]
    fn acquire_exhausted_returns_flow_error() {
        let mut c = CreditController::new(0);
        let err = c.acquire().unwrap_err();
        assert_eq!(err.code().as_u16(), 0x0401);
        assert!(err.is_retryable());
    }

    #[test]
    fn replenish_adds_credits() {
        let mut c = CreditController::new(0);
        c.replenish(50);
        assert_eq!(c.available(), 50);
    }

    #[test]
    fn replenish_saturates_at_max() {
        let mut c = CreditController::new(u16::MAX - 10);
        c.replenish(20);
        assert_eq!(c.available(), u16::MAX);
    }

    #[test]
    fn acquire_then_replenish_cycle() {
        let mut c = CreditController::new(2);
        assert!(c.acquire().is_ok());
        assert!(c.acquire().is_ok());
        assert!(c.acquire().is_err());
        c.replenish(1);
        assert!(c.acquire().is_ok());
        assert!(c.acquire().is_err());
    }

    #[test]
    fn zero_replenish_is_noop() {
        let mut c = CreditController::new(5);
        c.replenish(0);
        assert_eq!(c.available(), 5);
    }
}
