//! Protocol-neutral networking boundary.
//! Existing-server compatibility is not implemented by this foundation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug)]
pub struct Session {
    pub state: ConnectionState,
}

impl Session {
    pub fn new() -> Self {
        Self {
            state: ConnectionState::Disconnected,
        }
    }
}
