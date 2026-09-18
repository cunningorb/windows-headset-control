use thiserror::Error;

#[derive(Debug, Error)]
pub enum DeviceError {
    #[error("no matching device is present")]
    DongleNotFound,

    #[error("dongle is present but the headset is not reachable over the wireless link")]
    WirelessLinkUnavailable,

    #[error("access to the device was denied; another process may hold it exclusively")]
    AccessDenied,

    #[error("device is busy")]
    Busy,

    #[error("device was disconnected during the operation")]
    DisconnectedDuringOp,

    #[error("device did not respond within {0:?}")]
    Timeout(std::time::Duration),

    #[error("{0} devices matched; disambiguate with an explicit selector")]
    AmbiguousDevice(usize),

    #[error("response failed validation: {0}")]
    ProtocolMismatch(String),

    /// The device acknowledged nothing at all within the exchange window.
    ///
    /// Deliberately distinct from [`DeviceError::ProtocolMismatch`], which means
    /// the device answered and the answer was wrong. Silence is a different
    /// condition with a different cause and a different remedy, and folding the
    /// two together left callers unable to tell them apart: the tray could only
    /// match on an error string, so it treated a dongle that had stopped
    /// talking as a device it had not finished finding.
    #[error(
        "no response for parameter {param:#04x} within {waited:?}; {events_seen} unrelated \
         event(s) arrived while waiting"
    )]
    NoResponse {
        param: u8,
        waited: std::time::Duration,
        events_seen: usize,
    },

    #[error("device firmware is not supported: {0}")]
    UnsupportedFirmware(String),

    #[error("descriptor value outside the expected range: {0}")]
    UnexpectedDescriptor(String),

    #[error("refusing to open the audio-stack collection for I/O")]
    RefusedAudioCollection,

    #[error("this transport was not opened for writing")]
    WriteNotSupported,

    #[error(
        "the control collection does not have the descriptor shape this protocol was \
         derived from ({0}); refusing to write to a device whose framing may differ"
    )]
    UnexpectedControlShape(String),

    #[error("windows error: {0}")]
    Os(String),
}
