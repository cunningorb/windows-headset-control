//! Which Windows output device the sound goes to.
//!
//! Two halves with very different standing, and the difference matters:
//!
//! - **Listing** the outputs and reading which one is default is ordinary,
//!   documented Core Audio (`IMMDeviceEnumerator`). Nothing here is guessed.
//! - **Setting** the default has no documented API at all. Windows exposes no
//!   supported way to do it from code — the Sound settings page and `mmsys.cpl`
//!   are the only sanctioned routes, and both need a human. So this reaches the
//!   same undocumented `IPolicyConfig` interface every audio switcher uses.
//!
//! That second half is a deliberate exception, recorded in
//! `docs/undocumented-apis.md` rather than left for a reader to discover. It is
//! not the kind of guess `CONTRIBUTING.md` forbids — those rules govern HID
//! identifiers sent to the headset, and nothing in this module touches the
//! device. But it is unsupported, so every call site treats failure as normal:
//! a switch that does not happen leaves the user exactly where they were.

#![allow(unsafe_code)]

use std::ffi::c_void;

use windows::core::{IUnknown, Interface, GUID, HRESULT, PCWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    eCommunications, eConsole, eMultimedia, eRender, ERole, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL, STGM_READ};

/// One selectable Windows output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputDevice {
    /// The endpoint id, of the form `{0.0.0.00000000}.{guid}`.
    ///
    /// This is what the setting stores, not the friendly name: the name is
    /// whatever the driver currently calls it and changes under the user's feet
    /// (a re-installed driver renames "Speakers (Realtek Audio)"), whereas the
    /// id survives reboots, replugs, and renames.
    pub id: String,
    /// The name as the Sound settings page shows it.
    pub name: String,
}

/// Every active render endpoint, sorted by name so the picker's order is stable
/// between openings rather than following enumeration order.
///
/// Active only: a disabled or unplugged endpoint cannot be made the default, so
/// offering it would be offering something that silently fails.
pub fn render_outputs() -> Vec<OutputDevice> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("could not enumerate audio endpoints: {e}");
                    return Vec::new();
                }
            };
        let Ok(collection) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else {
            return Vec::new();
        };
        let count = collection.GetCount().unwrap_or(0);
        let mut out = Vec::with_capacity(count as usize);
        for i in 0..count {
            let Ok(device) = collection.Item(i) else {
                continue;
            };
            // No id means nothing can be stored or restored for this endpoint,
            // so it is skipped rather than listed as unselectable.
            let Some(id) = device_id(&device) else {
                continue;
            };
            let name = friendly_name(&device).unwrap_or_else(|| id.clone());
            out.push(OutputDevice { id, name });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }
}

/// The endpoint currently receiving ordinary playback.
///
/// Read against `eConsole`. Windows keeps three defaults and they can disagree;
/// this asks the one that answers "where does sound go", which is the value the
/// switch has to remember in order to put it back.
pub fn default_output_id() -> Option<String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        device_id(&device)
    }
}

/// Whether an endpoint id is currently present and active.
///
/// Used to tell "you chose speakers that are unplugged" from "you chose nothing",
/// which are different things to say to a user.
pub fn is_present(id: &str) -> bool {
    render_outputs().iter().any(|d| d.id == id)
}

/// Makes `id` the default output for all three roles.
///
/// All three deliberately: Windows keeps separate defaults for console,
/// multimedia, and communications, and moving only some of them is how you end
/// up with the game on the speakers and the voice chat still in a powered-off
/// headset. This is what picking a device in the Sound settings page does.
///
/// Returns whether every role took. A partial failure is reported as failure —
/// the caller must not record a successful switch it cannot fully reverse.
pub fn set_default_output(id: &str) -> bool {
    let Some(policy) = PolicyConfig::open() else {
        tracing::warn!("the default-output policy interface is unavailable on this system");
        return false;
    };
    let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
    let mut ok = true;
    for role in [eConsole, eMultimedia, eCommunications] {
        let hr = policy.set_default(&wide, role);
        if hr.is_err() {
            tracing::warn!(
                "setting the default output for role {} failed: {hr:?}",
                role.0
            );
            ok = false;
        }
    }
    ok
}

unsafe fn device_id(device: &IMMDevice) -> Option<String> {
    let raw = device.GetId().ok()?;
    if raw.is_null() {
        return None;
    }
    let s = raw.to_string().ok();
    // The id is allocated by the callee with CoTaskMemAlloc; the caller frees it.
    CoTaskMemFree(Some(raw.0 as *const c_void));
    s
}

unsafe fn friendly_name(device: &IMMDevice) -> Option<String> {
    let store = device.OpenPropertyStore(STGM_READ).ok()?;
    let value = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
    // The one place this module depends on the `windows` crate's PROPVARIANT
    // ergonomics rather than on a Win32 shape. If a future bump changes it,
    // this is the single line to adapt.
    let name = value.to_string();
    (!name.trim().is_empty()).then_some(name)
}

// ---------------------------------------------------------- IPolicyConfig ---
// The undocumented half. See this module's header for why it exists at all.

/// `CPolicyConfigClient`, the coclass that implements the interface below.
const CLSID_POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
/// `IPolicyConfig`, as shipped since Windows 7 and still present on Windows 11.
const IID_POLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);

/// The vtable, declared only as far as the one method this project calls.
///
/// The ten slots before it are real methods (device format, processing period,
/// share mode, and property get/set) that nothing here calls. They are left
/// deliberately unnamed and untyped: their position is what matters, because a
/// vtable is positional and `set_default_endpoint` must land at index 13. Naming
/// them would claim knowledge of signatures this project has no use for and no
/// way to check.
#[repr(C)]
struct PolicyConfigVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    _unused: [*const c_void; 10],
    set_default_endpoint: unsafe extern "system" fn(*mut c_void, PCWSTR, ERole) -> HRESULT,
}

/// An owned reference to the policy-config object. Releases on drop.
struct PolicyConfig(*mut c_void);

impl PolicyConfig {
    fn open() -> Option<PolicyConfig> {
        unsafe {
            let unknown: IUnknown =
                CoCreateInstance(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL).ok()?;
            let mut raw: *mut c_void = std::ptr::null_mut();
            if unknown.query(&IID_POLICY_CONFIG, &mut raw).is_err() || raw.is_null() {
                return None;
            }
            Some(PolicyConfig(raw))
        }
    }

    unsafe fn vtbl(&self) -> &PolicyConfigVtbl {
        &**(self.0 as *const *const PolicyConfigVtbl)
    }

    /// `id` must be NUL-terminated UTF-16.
    fn set_default(&self, id: &[u16], role: ERole) -> HRESULT {
        unsafe { (self.vtbl().set_default_endpoint)(self.0, PCWSTR(id.as_ptr()), role) }
    }
}

impl Drop for PolicyConfig {
    fn drop(&mut self) {
        unsafe {
            (self.vtbl().release)(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vtable's shape is the whole contract with an interface nobody
    /// documents: `set_default_endpoint` works only if it sits at index 13, and
    /// a stray field or a wrong-sized filler would call some other method
    /// entirely — with arguments meant for this one.
    #[test]
    fn set_default_endpoint_sits_at_vtable_index_13() {
        let slot = std::mem::size_of::<*const c_void>();
        let offset = std::mem::offset_of!(PolicyConfigVtbl, set_default_endpoint);
        assert_eq!(offset, slot * 13, "the vtable layout must not drift");
        assert_eq!(std::mem::size_of::<PolicyConfigVtbl>(), slot * 14);
    }

    /// Proves the undocumented interface actually works on this machine —
    /// CLSID, IID, vtable index, and calling convention together — without
    /// moving anything: it sets the console default to the endpoint that is
    /// *already* the console default.
    ///
    /// One role, not all three, precisely so it stays a no-op. Setting all
    /// three would overwrite a communications default that legitimately differs
    /// from the console one, which is a real change to the machine and not this
    /// test's business.
    ///
    /// `#[ignore]`d because it touches the live audio configuration, following
    /// the convention the hardware tests use. Run it when the interface is in
    /// question, which is the moment a Windows update is suspected of having
    /// moved it:
    ///
    /// ```text
    /// cargo test -p headset-tray --target x86_64-pc-windows-gnu -- --ignored policy
    /// ```
    #[test]
    #[ignore]
    fn the_policy_interface_accepts_a_no_op_default() {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }

        let current = default_output_id().expect("a default output must exist");
        let policy = PolicyConfig::open().expect("CPolicyConfigClient must be creatable");
        let wide: Vec<u16> = current.encode_utf16().chain(std::iter::once(0)).collect();
        let hr = policy.set_default(&wide, eConsole);
        assert!(hr.is_ok(), "SetDefaultEndpoint returned {hr:?}");
    }

    /// Enumeration is read-only and needs no hardware, but it does need COM to
    /// have been initialised on this thread. The tray does that at startup; the
    /// test harness does not, so this only pins that the call is total — it
    /// returns a list or an empty one, and never panics or hangs.
    #[test]
    fn enumeration_is_total_even_without_com() {
        let _ = render_outputs();
        let _ = default_output_id();
    }
}
