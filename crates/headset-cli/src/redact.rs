use headset_device::DeviceId;
use sha2::{Digest, Sha256};

/// Controls whether machine-identifying values reach the output.
///
/// `main.rs` is a placeholder until Task 7 wires real commands on top of
/// this type, so most of the API below is unused by production code for
/// now (the unit tests below do exercise `path`/`serial`/the field, so
/// their `expect` is scoped to `not(test)`). Each unused item carries its
/// own `#[expect(dead_code)]`: deliberately, item-scoped, and
/// self-expiring — once Task 7 actually calls an item from production
/// code, its expectation goes unfulfilled and `-D warnings` turns that
/// into a build failure, forcing the attribute's removal instead of
/// leaving dead-code detection blinded.
#[derive(Clone, Copy, Debug)]
pub struct Redactor {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read once Task 7 wires real commands on Redactor")
    )]
    include_sensitive: bool,
}

impl Redactor {
    pub fn new(include_sensitive: bool) -> Self {
        Self { include_sensitive }
    }

    #[expect(
        dead_code,
        reason = "called once Task 7 wires real commands on Redactor"
    )]
    pub fn include_sensitive(&self) -> bool {
        self.include_sensitive
    }

    /// Device paths identify a machine and a USB topology. By default they are
    /// reduced to a truncated, unsalted SHA-256 so records still correlate
    /// across runs and across bug reports without leaking the value.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "called once Task 7 wires real commands on Redactor"
        )
    )]
    pub fn path(&self, id: &DeviceId) -> String {
        if self.include_sensitive {
            return id.raw().to_string();
        }
        let digest = Sha256::digest(id.raw().to_ascii_lowercase().as_bytes());
        format!(
            "path:sha256:{:02x}{:02x}{:02x}{:02x}",
            digest[0], digest[1], digest[2], digest[3]
        )
    }

    /// Presence is reported; the value never is unless explicitly requested.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "called once Task 7 wires real commands on Redactor"
        )
    )]
    pub fn serial(&self, present: bool) -> String {
        match (present, self.include_sensitive) {
            (false, _) => "<absent>".to_string(),
            (true, false) => "<present, redacted>".to_string(),
            (true, true) => "<present>".to_string(),
        }
    }

    /// Header printed above any output that contains machine-identifying data.
    #[expect(
        dead_code,
        reason = "called once Task 7 wires real commands on Redactor"
    )]
    pub fn warning_banner(&self) -> Option<&'static str> {
        self.include_sensitive.then_some(
            "WARNING: --include-sensitive is set. This output contains machine-identifying \
             values. Do not paste it into a public issue.",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use headset_device::DeviceId;

    const PATH: &str = "\\\\?\\hid#vid_1532&pid_101b&mi_05&col04#7&2f9a1b&0&0000";

    #[test]
    fn redacted_path_hides_the_raw_value() {
        let out = Redactor::new(false).path(&DeviceId::new(PATH));
        assert!(!out.contains("2f9a1b"));
        assert!(out.starts_with("path:sha256:"));
    }

    #[test]
    fn redacted_path_is_stable_across_calls() {
        let id = DeviceId::new(PATH);
        assert_eq!(
            Redactor::new(false).path(&id),
            Redactor::new(false).path(&id)
        );
    }

    #[test]
    fn different_paths_redact_differently() {
        let a = Redactor::new(false).path(&DeviceId::new(PATH));
        let b = Redactor::new(false).path(&DeviceId::new(
            "\\\\?\\hid#vid_1532&pid_101b&mi_05&col02#7&2f9a1b&0&0001",
        ));
        assert_ne!(a, b);
    }

    #[test]
    fn sensitive_mode_reveals_the_raw_path() {
        assert_eq!(Redactor::new(true).path(&DeviceId::new(PATH)), PATH);
    }

    #[test]
    fn serial_presence_is_reported_without_the_value() {
        assert_eq!(Redactor::new(false).serial(true), "<present, redacted>");
        assert_eq!(Redactor::new(false).serial(false), "<absent>");
    }

    #[test]
    fn redacted_digest_is_truncated_to_eight_hex_chars() {
        let out = Redactor::new(false).path(&DeviceId::new(PATH));
        let digest = out.strip_prefix("path:sha256:").unwrap();
        assert_eq!(digest.len(), 8);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn redaction_is_case_insensitive() {
        let lower = DeviceId::new(PATH);
        let upper = DeviceId::new(PATH.to_ascii_uppercase());
        assert_eq!(
            Redactor::new(false).path(&lower),
            Redactor::new(false).path(&upper)
        );
    }
}
