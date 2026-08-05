# Undocumented Windows APIs

This project uses exactly one Windows interface that Microsoft does not document, and
this file is the record of it. Nothing else in the workspace depends on undocumented
behaviour, and nothing here touches the headset.

## `IPolicyConfig`, to set the default audio output

**Used by:** `crates/headset-tray/src/win32/audio.rs::set_default_output`, which the
"switch output when powered off" setting calls.

**Why there is no alternative.** Windows exposes no supported API for changing the
default playback device. Core Audio's `IMMDeviceEnumerator` can *read* the current
default (`GetDefaultAudioEndpoint`) and enumerate every endpoint, both documented and
both used here — but there is no matching setter, and there never has been. The Sound
settings page and `mmsys.cpl` change it through a private interface. A feature that moves
your sound off a powered-down headset cannot be built without it.

**What is used:**

```text
CLSID   870af99c-171d-4f9e-af0d-e63df40c2bc9   (CPolicyConfigClient)
IID     f8679f50-850a-41cf-9c72-430f290290c8   (IPolicyConfig)
method  vtable index 13, SetDefaultEndpoint(PCWSTR endpointId, ERole role)
```

The vtable is declared only as far as that one method. The ten slots before it are real
methods (device format, processing period, share mode, property get/set) that this
project never calls; they are left deliberately unnamed and untyped, because a vtable is
positional and only their *count* matters. Naming them would claim knowledge of
signatures nothing here uses or could check. A unit test asserts the offset, since a
stray field would silently call a different method with this one's arguments.

`SetDefaultEndpoint` is called once for each of the three roles Windows keeps separately
— console, multimedia, and communications — because that is what choosing a device in the
Sound settings page does. Moving only some of them is how the game ends up on the
speakers with voice chat still in a headset that is switched off.

**Risk, and how it is contained.** The interface is unsupported: it can change or vanish
in a Windows update without notice. So every call site treats failure as an ordinary
outcome rather than an error to surface loudly:

- Failing to acquire the interface, or a refused `SetDefaultEndpoint`, leaves the user
  exactly where they were. Nothing is half-moved.
- The record of "we owe this user a move back" is written **only after** a switch
  succeeds, so a failed switch cannot cause a later, unearned one.
- A partial success across the three roles is reported as failure, because a switch that
  cannot be fully reversed must not be recorded as reversible.
- The feature is off by default. A user who never turns it on never loads the interface.

**How this relates to the project's rules.** `CONTRIBUTING.md` forbids speculative
identifiers and undocumented guesses. Those rules govern **HID commands sent to the
headset**, where a wrong guess reaches hardware whose firmware nobody here can inspect,
and where the whole clean-room posture is at stake. This is a different category: a
Windows API, called on the local machine, that changes a setting the user can change by
hand in the Sound settings page and can change straight back. It is still an exception,
which is why it is written down here rather than left in a comment.

**What was not done.** Shelling out to a third-party switcher (`nircmd`, `SoundVolumeView`)
was rejected: it adds a dependency the user has to install and trust, and those tools call
this same interface anyway — it would hide the exception rather than remove it.
