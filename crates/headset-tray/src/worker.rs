//! The device thread.
//!
//! Owns the one `ControlSession` and is the only thing that talks to the
//! headset. The UI thread sends `Command`s and receives `HeadsetState`
//! snapshots; it never blocks on HID I/O, which is what keeps the tray menu
//! responsive while an exchange is in flight.
//!
//! Reconnect is handled here too: if the session dies (the dongle re-enumerates
//! on replug, and its path changes when it does) the thread re-resolves rather
//! than exiting.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use headset_device::{ControlSession, DeviceError, HidBackend};
use headset_protocol::{
    encode_noise_write, encode_read, encode_write, NoiseControl, Param, ResultCode,
};

use crate::state::HeadsetState;

/// Sidetone writes are preceded by `0x18 = 01`, reproducing what the vendor
/// software was observed doing before every level write.
const SIDETONE_ENABLE: u8 = 0x18;

/// How long to sit in `listen` between polls. Events arrive on their own, so
/// this is not a polling interval for values — it is how often the thread wakes
/// to notice commands and connection changes.
const LISTEN_SLICE: Duration = Duration::from_millis(500);

/// How often to re-read everything even without an event, as a backstop against
/// a missed push. Deliberately slow: the device pushes changes, so this exists
/// only to heal a missed one, not to drive the UI.
const FULL_REFRESH: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    SetSidetone(u8),
    SetGameChat(u8),
    /// Both bytes of parameter `0x12`. Never one: the device holds mode and
    /// level together, so the caller composes the whole state.
    SetNoise(NoiseControl),
    Refresh,
    Shutdown,
}

/// Runs until a `Shutdown` command arrives or the command channel closes.
///
/// `notify` is called after every state change so the UI can repaint. It is
/// invoked from this thread, so implementations must be cheap and thread-safe —
/// posting a window message is the intended use.
pub fn run<B: HidBackend, F: Fn(HeadsetState)>(
    backend: &B,
    commands: Receiver<Command>,
    notify: F,
) {
    // Device-owned state only. `warn_vendor_software` and `mic_mute_os` belong
    // to the UI thread; setting them here would be a value this thread has no
    // business having an opinion about, and it used to be computed once at
    // startup and then quietly stamped over the user's choice.
    let mut state = HeadsetState::default();
    let mut session: Option<ControlSession> = None;
    let mut since_refresh = Duration::ZERO;

    loop {
        // (Re)establish the session. A failure here is normal, not fatal: the
        // dongle may be unplugged, so back off and try again rather than exit.
        if session.is_none() {
            match ControlSession::open(backend) {
                Ok(s) => {
                    // Take the name from the descriptor the moment the device
                    // resolves, so the header is right before any read lands.
                    state.device_name = s.info().product.clone();
                    session = Some(s);
                    since_refresh = FULL_REFRESH; // force an immediate read
                }
                Err(e) => {
                    tracing::debug!("control session unavailable: {e}");
                    state.connected = None;
                    notify(state.clone());
                    match commands.recv_timeout(Duration::from_secs(3)) {
                        Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
                        _ => continue,
                    }
                }
            }
        }
        let Some(s) = session.as_mut() else { continue };

        if since_refresh >= FULL_REFRESH {
            match refresh_all(s, &mut state) {
                Ok(()) => {
                    since_refresh = Duration::ZERO;
                    notify(state.clone());
                }
                Err(e) if is_fatal(&e) => {
                    tracing::warn!("session lost, will re-resolve: {e}");
                    session = None;
                    continue;
                }
                Err(e) => tracing::debug!("refresh failed: {e}"),
            }
        }

        match commands.recv_timeout(LISTEN_SLICE) {
            Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(cmd) => {
                if let Err(e) = apply(s, cmd, &mut state) {
                    if is_fatal(&e) {
                        session = None;
                        continue;
                    }
                    tracing::warn!("command failed: {e}");
                }
                notify(state.clone());
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }

        // No command pending: spend the slice listening for pushes.
        match s.listen(LISTEN_SLICE) {
            Ok(events) => {
                if !events.is_empty() {
                    for e in &events {
                        state.apply(e);
                    }
                    // A link transition invalidates everything; re-read rather
                    // than wait for individual pushes that may not come.
                    if events.iter().any(|e| e.param == Param::LinkState.id()) {
                        since_refresh = FULL_REFRESH;
                    }
                    notify(state.clone());
                }
            }
            Err(e) if is_fatal(&e) => {
                session = None;
                continue;
            }
            Err(e) => tracing::debug!("listen failed: {e}"),
        }
        since_refresh += LISTEN_SLICE;
    }
}

/// Errors that mean the handle is gone rather than the device being slow.
fn is_fatal(e: &DeviceError) -> bool {
    matches!(
        e,
        DeviceError::DisconnectedDuringOp
            | DeviceError::DongleNotFound
            | DeviceError::AccessDenied
            | DeviceError::Os(_)
    )
}

fn read_into(
    s: &mut ControlSession,
    param: Param,
    state: &mut HeadsetState,
) -> Result<(), DeviceError> {
    let req = encode_read(param.id(), None).expect("named parameters are allowlisted");
    let frame = s.exchange(&req, param.id(), false)?;
    state.apply(&frame);
    Ok(())
}

fn refresh_all(s: &mut ControlSession, state: &mut HeadsetState) -> Result<(), DeviceError> {
    // Link first: if the headset is unreachable every other read returns the
    // refusal byte, and `apply` would have to discard them all anyway.
    read_into(s, Param::LinkState, state)?;
    if state.connected == Some(false) {
        return Ok(());
    }
    for p in [
        Param::Battery,
        Param::Sidetone,
        Param::GameChatBalance,
        Param::MicMute,
        Param::NoiseCancellation,
    ] {
        read_into(s, p, state)?;
    }
    Ok(())
}

fn apply(
    s: &mut ControlSession,
    cmd: Command,
    state: &mut HeadsetState,
) -> Result<(), DeviceError> {
    match cmd {
        Command::Shutdown => Ok(()),
        Command::Refresh => refresh_all(s, state),
        Command::SetSidetone(v) => {
            let enable = encode_write(SIDETONE_ENABLE, 1).expect("allowlisted");
            let ack = s.exchange(&enable, SIDETONE_ENABLE, true)?;
            reject_unless_ok(&ack)?;
            let req = encode_write(Param::Sidetone.id(), v).expect("allowlisted");
            let ack = s.exchange(&req, Param::Sidetone.id(), true)?;
            reject_unless_ok(&ack)?;
            // Read back rather than assuming: the device is the source of truth.
            read_into(s, Param::Sidetone, state)
        }
        Command::SetGameChat(v) => {
            let req = encode_write(Param::GameChatBalance.id(), v).expect("allowlisted");
            let ack = s.exchange(&req, Param::GameChatBalance.id(), true)?;
            reject_unless_ok(&ack)?;
            read_into(s, Param::GameChatBalance, state)
        }
        Command::SetNoise(want) => {
            // No preamble was ever observed for this parameter, unlike
            // sidetone's 0x18. The vendor software's read before the write is
            // the UI's job here: it composed `want` from the state it holds.
            let req = encode_noise_write(want)
                .map_err(|e| DeviceError::ProtocolMismatch(e.to_string()))?;
            let ack = s.exchange(&req, Param::NoiseCancellation.id(), true)?;
            reject_unless_ok(&ack)?;
            read_into(s, Param::NoiseCancellation, state)
        }
    }
}

fn reject_unless_ok(ack: &headset_protocol::ParamFrame) -> Result<(), DeviceError> {
    match ack.result() {
        Some(ResultCode::Ok) | None => Ok(()),
        Some(ResultCode::Refused) => Err(DeviceError::ProtocolMismatch(
            "device refused the write (0xff); the mic mute switch or an unreachable \
             headset are the observed causes"
                .into(),
        )),
        Some(ResultCode::Unknown(b)) => Err(DeviceError::ProtocolMismatch(format!(
            "unrecognised result {b:#04x}; not treating it as success"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use headset_device::{resolve_control_device, FakeHidBackend};
    use headset_protocol::{checksum, NoiseMode, CONTROL_REPORT_LEN};
    use std::sync::mpsc;

    const FIXTURE: &str =
        include_str!("../../headset-device/tests/fixtures/blackshark-v3-pro-ps.json");

    fn response(param: u8, is_write: bool, payload: &[u8]) -> Vec<u8> {
        let mut b = vec![0u8; CONTROL_REPORT_LEN];
        b[0] = 0x02;
        b[1] = 0x02;
        b[2] = 0x60;
        b[6] = 4 + payload.len() as u8;
        b[9] = 0x80;
        b[10] = if is_write { param | 0x80 } else { param };
        b[11] = 0x01;
        b[12] = payload.len() as u8;
        b[13..13 + payload.len()].copy_from_slice(payload);
        b[62] = checksum(&b);
        b
    }

    /// Runs the worker to completion with `Shutdown` already queued, returning
    /// every state it published.
    ///
    /// Collects into a shared `Vec` rather than a channel on purpose: a channel
    /// receiver blocks until every sender is dropped, and a notify closure that
    /// captures the sender by reference keeps one alive for the whole test. That
    /// deadlocks the test, not the code under test.
    fn run_to_completion(b: &FakeHidBackend) -> Vec<HeadsetState> {
        let (tx, rx) = mpsc::channel();
        tx.send(Command::Shutdown).unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        run(b, rx, move |s| sink.lock().unwrap().push(s));
        let out = seen.lock().unwrap().clone();
        out
    }

    #[test]
    fn a_disconnected_headset_short_circuits_the_refresh() {
        let mut b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let info = resolve_control_device(&b).unwrap();
        b.push_read(
            &info.id,
            response(Param::LinkState.id(), false, &[0x00, 0x00]),
        );

        let states = run_to_completion(&b);
        let last = states.last().expect("at least one update");
        assert_eq!(last.connected, Some(false));
        assert_eq!(last.battery, None);
        // Link read only: the four proxied reads must be skipped.
        assert_eq!(b.writes().len(), 1);
    }

    #[test]
    fn a_full_refresh_reads_every_displayed_parameter() {
        let mut b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let info = resolve_control_device(&b).unwrap();
        for r in [
            response(Param::LinkState.id(), false, &[0x01, 0x00]),
            response(Param::Battery.id(), false, &[49]),
            response(Param::Sidetone.id(), false, &[7]),
            response(Param::GameChatBalance.id(), false, &[10]),
            response(Param::MicMute.id(), false, &[1]),
            response(Param::NoiseCancellation.id(), false, &[0x01, 0x03]),
        ] {
            b.push_read(&info.id, r);
        }

        let states = run_to_completion(&b);
        let last = states.last().unwrap();
        assert_eq!(last.connected, Some(true));
        assert_eq!(last.battery, Some(49));
        assert_eq!(last.sidetone, Some(7));
        assert_eq!(last.game_chat, Some(10));
        assert_eq!(last.mic_mute_hardware, Some(true));
        assert_eq!(last.effectively_muted(), Some(true));
        assert_eq!(
            last.noise,
            Some(NoiseControl {
                mode: NoiseMode::Anc,
                anc_level: 3
            })
        );
    }

    #[test]
    fn setting_noise_writes_both_bytes_and_reads_back() {
        let mut b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let info = resolve_control_device(&b).unwrap();
        // Link read short-circuits the rest of the refresh, so the only
        // exchanges after it are the write and its read-back.
        for r in [
            response(Param::LinkState.id(), false, &[0x00, 0x00]),
            response(Param::NoiseCancellation.id(), true, &[0x00]),
            response(Param::NoiseCancellation.id(), false, &[0x50, 0x03]),
        ] {
            b.push_read(&info.id, r);
        }

        let (tx, rx) = mpsc::channel();
        tx.send(Command::SetNoise(NoiseControl {
            mode: NoiseMode::Ambient,
            anc_level: 3,
        }))
        .unwrap();
        tx.send(Command::Shutdown).unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        run(&b, rx, move |s| sink.lock().unwrap().push(s));
        let states = seen.lock().unwrap().clone();

        let writes = b.writes();
        let cmd = writes
            .iter()
            .find(|w| w[10] == 0x92)
            .expect("a noise write must reach the wire");
        assert_eq!(cmd[12], 2, "both bytes go out together");
        // Ambient carries the byte the vendor software was observed sending,
        // not the retained level.
        assert_eq!(&cmd[13..15], &[0x50, 0x01]);

        assert_eq!(
            states.last().unwrap().noise,
            Some(NoiseControl {
                mode: NoiseMode::Ambient,
                anc_level: 3
            }),
            "the state must come from the read-back, not from what was asked"
        );
    }

    #[test]
    fn a_refused_read_does_not_reach_the_screen_as_255() {
        let mut b = FakeHidBackend::from_fixture_str(FIXTURE).unwrap();
        let info = resolve_control_device(&b).unwrap();
        for r in [
            response(Param::LinkState.id(), false, &[0x01, 0x00]),
            response(Param::Battery.id(), false, &[0xFF]),
            response(Param::Sidetone.id(), false, &[0xFF]),
            response(Param::GameChatBalance.id(), false, &[0xFF]),
            response(Param::MicMute.id(), false, &[0xFF]),
            response(Param::NoiseCancellation.id(), false, &[0xFF]),
        ] {
            b.push_read(&info.id, r);
        }

        let states = run_to_completion(&b);
        let last = states.last().unwrap();
        assert_eq!(last.battery, None);
        assert_eq!(last.sidetone, None);
        assert_eq!(last.noise, None);
        assert!(!last.tooltip().contains("255"), "{}", last.tooltip());
    }
}
