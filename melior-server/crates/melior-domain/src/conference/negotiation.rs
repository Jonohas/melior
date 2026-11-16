use super::DomainError;

/// Where a participant's offer/answer exchange with the server stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationPhase {
    /// Joined, but the client has not sent its first offer yet.
    AwaitingClientOffer,
    /// The first offer is answered; the media connection is still being established.
    Connecting,
    /// No exchange in flight.
    Stable,
    /// The server sent an offer and waits for the client's answer.
    AwaitingAnswer,
}

/// Whether an answered client offer was the participant's first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientOffer {
    Initial,
    Renegotiation,
}

/// The negotiation rules for one participant.
///
/// The client makes the first offer. Once its media connection is up the participant is
/// "ready", and from then on the server renegotiates whenever the set of tracks the
/// participant receives changes, one exchange at a time: a change that arrives while an
/// offer is outstanding is remembered and negotiated once the answer is in, instead of
/// sending a second, overlapping offer. The server never offers before the connection
/// is established, so a renegotiation cannot race the initial connection setup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Negotiation {
    phase: NegotiationPhase,
    pending_changes: bool,
}

impl Default for Negotiation {
    fn default() -> Self {
        Self { phase: NegotiationPhase::AwaitingClientOffer, pending_changes: false }
    }
}

impl Negotiation {
    #[must_use]
    pub fn phase(&self) -> NegotiationPhase {
        self.phase
    }

    /// True once the media connection is established; only then can tracks be sent.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self.phase, NegotiationPhase::Stable | NegotiationPhase::AwaitingAnswer)
    }

    /// A client offer cannot be processed while a server offer is outstanding (glare).
    pub fn ensure_can_accept_offer(&self) -> Result<(), DomainError> {
        match self.phase {
            NegotiationPhase::AwaitingAnswer => Err(DomainError::OfferDuringRenegotiation),
            _ => Ok(()),
        }
    }

    /// Records that a client offer has been answered.
    pub fn offer_answered(&mut self) -> ClientOffer {
        match self.phase {
            NegotiationPhase::AwaitingClientOffer => {
                self.phase = NegotiationPhase::Connecting;
                ClientOffer::Initial
            }
            NegotiationPhase::Connecting => ClientOffer::Renegotiation,
            NegotiationPhase::Stable | NegotiationPhase::AwaitingAnswer => {
                self.phase = NegotiationPhase::Stable;
                ClientOffer::Renegotiation
            }
        }
    }

    /// Records that the media connection is up. Returns true the first time, when the
    /// participant becomes ready; later reconnects change nothing.
    pub fn connected(&mut self) -> bool {
        if self.phase == NegotiationPhase::Connecting {
            self.phase = NegotiationPhase::Stable;
            true
        } else {
            false
        }
    }

    /// Asks for a server offer. Returns true if one must be sent now; otherwise the
    /// request is deferred (an exchange is in flight) or unnecessary (not ready yet:
    /// the participant receives everything once it is connected).
    pub fn request_offer(&mut self) -> bool {
        match self.phase {
            NegotiationPhase::Stable => {
                self.phase = NegotiationPhase::AwaitingAnswer;
                self.pending_changes = false;
                true
            }
            NegotiationPhase::AwaitingAnswer => {
                self.pending_changes = true;
                false
            }
            NegotiationPhase::AwaitingClientOffer | NegotiationPhase::Connecting => false,
        }
    }

    /// The offer announced by [`Self::request_offer`] could not be produced or sent.
    pub fn offer_failed(&mut self) {
        if self.phase == NegotiationPhase::AwaitingAnswer {
            self.phase = NegotiationPhase::Stable;
        }
    }

    pub fn ensure_can_accept_answer(&self) -> Result<(), DomainError> {
        match self.phase {
            NegotiationPhase::AwaitingAnswer => Ok(()),
            _ => Err(DomainError::UnexpectedAnswer),
        }
    }

    /// Records an applied answer. Returns true if changes piled up meanwhile and
    /// another offer should be requested.
    pub fn answer_applied(&mut self) -> bool {
        self.offer_failed();
        std::mem::take(&mut self.pending_changes)
    }

    /// The client's answer was unusable; the exchange is abandoned.
    pub fn answer_rejected(&mut self) {
        self.offer_failed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Negotiation {
        let mut n = Negotiation::default();
        n.offer_answered();
        assert!(n.connected());
        n
    }

    #[test]
    fn starts_waiting_for_the_client_offer() {
        let mut n = Negotiation::default();
        assert!(!n.is_ready());
        assert!(!n.request_offer(), "no server offer before the first client offer");
        assert_eq!(n.ensure_can_accept_answer(), Err(DomainError::UnexpectedAnswer));
        assert!(!n.connected(), "cannot be connected before the first offer");
        assert_eq!(n.offer_answered(), ClientOffer::Initial);
        assert_eq!(n.phase(), NegotiationPhase::Connecting);
        assert!(!n.is_ready());
        assert!(!n.request_offer(), "no server offer while connecting");

        assert!(n.connected());
        assert!(n.is_ready());
        assert!(!n.connected(), "only the first connect makes a participant ready");
        assert_eq!(n.offer_answered(), ClientOffer::Renegotiation);
        assert_eq!(n.phase(), NegotiationPhase::Stable);
    }

    #[test]
    fn only_one_server_offer_is_outstanding() {
        let mut n = ready();
        assert!(n.request_offer());
        assert_eq!(n.phase(), NegotiationPhase::AwaitingAnswer);
        assert!(!n.request_offer(), "second change is deferred");
        assert!(!n.request_offer());

        assert!(n.answer_applied(), "deferred changes need another offer");
        assert!(n.request_offer());
        assert!(!n.answer_applied(), "nothing piled up this time");
        assert_eq!(n.phase(), NegotiationPhase::Stable);
    }

    #[test]
    fn client_offer_during_server_offer_is_glare() {
        let mut n = ready();
        assert!(n.ensure_can_accept_offer().is_ok());
        n.request_offer();
        assert_eq!(n.ensure_can_accept_offer(), Err(DomainError::OfferDuringRenegotiation));
    }

    #[test]
    fn failed_or_rejected_exchange_returns_to_stable() {
        let mut n = ready();
        n.request_offer();
        n.offer_failed();
        assert_eq!(n.phase(), NegotiationPhase::Stable);

        n.request_offer();
        n.answer_rejected();
        assert_eq!(n.phase(), NegotiationPhase::Stable);
        assert!(n.request_offer(), "a later change can start a fresh exchange");
    }
}
