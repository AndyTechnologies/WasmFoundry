//! What an invocation is allowed to do, and how long it may do it.

use std::fmt;
use std::time::Duration;

/// Access a guest has to a mounted directory.
///
/// The domain names access in its own terms and the adapter translates to whatever the
/// engine exposes today, so the model does not inherit an engine's vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FilesystemAccess {
    /// Read only. The default, because a default is a security decision.
    #[default]
    ReadOnly,
    /// Read and write.
    ReadWrite,
}

impl fmt::Display for FilesystemAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            FilesystemAccess::ReadOnly => "read-only",
            FilesystemAccess::ReadWrite => "read-write",
        })
    }
}

/// A host directory made visible to a guest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    host: String,
    guest: String,
    access: FilesystemAccess,
}

impl Mount {
    /// Builds a mount.
    ///
    /// `guest` must be absolute: a guest path is a name inside the guest's own world,
    /// and a relative one would resolve against a working directory the host does not
    /// control.
    pub fn new(
        host: impl Into<String>,
        guest: impl Into<String>,
        access: FilesystemAccess,
    ) -> Self {
        Mount {
            host: host.into(),
            guest: guest.into(),
            access,
        }
    }

    /// The host directory.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The path the guest sees it at.
    pub fn guest(&self) -> &str {
        &self.guest
    }

    /// What the guest may do with it.
    pub fn access(&self) -> FilesystemAccess {
        self.access
    }
}

/// Bounds on one invocation.
///
/// A guest that runs forever or grows without bound is not something a user should have
/// to notice after the fact, so the limits are part of the policy rather than an option
/// a caller has to remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExecutionLimits {
    wall_clock: Option<Duration>,
    memory_pages: Option<u64>,
}

impl ExecutionLimits {
    /// No limits at all.
    pub fn unlimited() -> Self {
        ExecutionLimits::default()
    }

    /// A wall-clock budget and a memory ceiling, in 64 KiB pages.
    pub fn bounded(wall_clock: Option<Duration>, memory_pages: Option<u64>) -> Self {
        ExecutionLimits {
            wall_clock,
            memory_pages,
        }
    }

    /// How long an invocation may run.
    pub fn wall_clock(&self) -> Option<Duration> {
        self.wall_clock
    }

    /// How much memory the guest may hold, in 64 KiB pages, or `None` for unbounded.
    pub fn memory_pages(&self) -> Option<u64> {
        self.memory_pages
    }
}

/// What one invocation may do.
///
/// Everything is denied unless the policy says otherwise. A guest that was never granted
/// a capability cannot reach it, which is a stronger guarantee than a guest that is
/// trusted not to try.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimePolicy {
    wasi: bool,
    mounts: Vec<Mount>,
    environment: bool,
    limits: ExecutionLimits,
}

impl RuntimePolicy {
    /// The default: everything denied, no limits.
    ///
    /// A guest runs its own code and reaches nothing else. Limits are absent here too,
    /// because a default that silently truncates a legitimate long build would be a
    /// surprise of its own; a caller that runs untrusted code sets them.
    pub fn denied() -> Self {
        RuntimePolicy::default()
    }

    /// A policy with WASI, its mounts and its limits.
    pub fn wasi(mounts: Vec<Mount>, limits: ExecutionLimits) -> Self {
        RuntimePolicy {
            wasi: true,
            mounts,
            environment: false,
            limits,
        }
    }

    /// The policy a caller described in full: every capability is granted or denied
    /// explicitly, nothing is assumed.
    pub fn granted(
        wasi: bool,
        mounts: Vec<Mount>,
        environment: bool,
        limits: ExecutionLimits,
    ) -> Self {
        RuntimePolicy {
            wasi,
            mounts,
            environment,
            limits,
        }
    }

    /// Whether the guest may use WASI at all.
    pub fn wasi_enabled(&self) -> bool {
        self.wasi
    }

    /// The directories the guest may see, in declaration order.
    pub fn mounts(&self) -> &[Mount] {
        &self.mounts
    }

    /// Whether the host's environment is visible to the guest.
    ///
    /// Off by default: an environment variable is a secret as often as a setting.
    pub fn environment_visible(&self) -> bool {
        self.environment
    }

    /// The bounds this policy imposes.
    pub fn limits(&self) -> ExecutionLimits {
        self.limits
    }

    /// Checks that this policy can be applied.
    ///
    /// The rules here are ones the engine cannot check for us: a mount that names
    /// nothing, a guest path that is not absolute, or two mounts claiming one path would
    /// otherwise become a confusing failure — or, worse, a permission nobody chose.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();

        for mount in &self.mounts {
            if mount.host().is_empty() {
                problems.push("a mount must name a host directory".to_owned());
            }
            if mount.guest().is_empty() {
                problems.push(format!(
                    "the mount of `{}` must name a guest path",
                    mount.host()
                ));
            } else if !mount.guest().starts_with('/') {
                problems.push(format!(
                    "the guest path `{}` is not absolute; a guest path is a name inside \
                     the guest's own world",
                    mount.guest()
                ));
            }
        }

        let mut seen: Vec<&str> = Vec::new();
        for mount in &self.mounts {
            if seen.contains(&mount.guest()) {
                problems.push(format!(
                    "two mounts claim the guest path `{}`; only one directory can be there",
                    mount.guest()
                ));
            } else {
                seen.push(mount.guest());
            }
        }

        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default is the one the plan asks for: denied.
    #[test]
    fn the_default_grants_nothing() {
        let policy = RuntimePolicy::denied();
        assert!(!policy.wasi_enabled());
        assert!(policy.mounts().is_empty());
        assert!(!policy.environment_visible());
    }

    #[test]
    fn an_environment_is_not_visible_unless_asked_for() {
        // An environment variable is a secret as often as a setting.
        assert!(!RuntimePolicy::denied().environment_visible());
    }

    #[test]
    fn access_defaults_to_read_only() {
        // The legacy defaulted mounts to read-write. A default is a security decision,
        // so the new one is the narrower answer.
        assert_eq!(FilesystemAccess::default(), FilesystemAccess::ReadOnly);
        assert_eq!(FilesystemAccess::ReadOnly.to_string(), "read-only");
        assert_eq!(FilesystemAccess::ReadWrite.to_string(), "read-write");
    }

    #[test]
    fn a_policy_without_limits_is_unlimited() {
        let limits = ExecutionLimits::unlimited();
        assert_eq!(limits.wall_clock(), None);
        assert_eq!(limits.memory_pages(), None);
    }

    #[test]
    fn limits_carry_both_bounds() {
        let limits = ExecutionLimits::bounded(Some(Duration::from_secs(5)), Some(256));
        assert_eq!(limits.wall_clock(), Some(Duration::from_secs(5)));
        assert_eq!(limits.memory_pages(), Some(256));
    }

    #[test]
    fn wasi_comes_with_its_mounts_and_still_no_environment() {
        let policy = RuntimePolicy::wasi(
            vec![Mount::new("./data", "/data", FilesystemAccess::ReadOnly)],
            ExecutionLimits::unlimited(),
        );

        assert!(policy.wasi_enabled());
        assert_eq!(policy.mounts().len(), 1);
        assert!(
            !policy.environment_visible(),
            "opting into WASI is not opting into secrets"
        );
    }

    #[test]
    fn a_guest_path_must_be_absolute() {
        let policy = RuntimePolicy::wasi(
            vec![Mount::new("./data", "data", FilesystemAccess::ReadOnly)],
            ExecutionLimits::unlimited(),
        );

        let problems = policy.validate();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("data"), "{}", problems[0]);
        assert!(problems[0].contains("absolute"), "{}", problems[0]);
    }

    #[test]
    fn two_mounts_may_not_claim_one_path() {
        let policy = RuntimePolicy::wasi(
            vec![
                Mount::new("./a", "/shared", FilesystemAccess::ReadOnly),
                Mount::new("./b", "/shared", FilesystemAccess::ReadOnly),
            ],
            ExecutionLimits::unlimited(),
        );

        let problems = policy.validate();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("/shared"), "{}", problems[0]);
    }

    #[test]
    fn a_mount_without_a_guest_path_is_reported() {
        let policy = RuntimePolicy::wasi(
            vec![Mount::new("./data", "", FilesystemAccess::ReadOnly)],
            ExecutionLimits::unlimited(),
        );

        let problems = policy.validate();
        assert_eq!(problems.len(), 1, "{problems:?}");
    }

    #[test]
    fn a_sound_policy_reports_nothing() {
        let policy = RuntimePolicy::wasi(
            vec![
                Mount::new("./a", "/a", FilesystemAccess::ReadOnly),
                Mount::new("./b", "/b", FilesystemAccess::ReadWrite),
            ],
            ExecutionLimits::bounded(Some(Duration::from_secs(1)), Some(16)),
        );
        assert!(policy.validate().is_empty());
    }
}
