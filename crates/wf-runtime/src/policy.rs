//! Applying a [`RuntimePolicy`] to one invocation.
//!
//! The domain describes what an invocation may do; this module is where that description
//! becomes engine configuration. Nothing in `wf-core` learns what a `WasiCtx` is, what a
//! preopen directory costs, or that a wall-clock budget is enforced by an epoch.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use wasmtime::{Config, Engine, Linker, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::filesystem::FsPerms;

use wf_core::{ExecutionLimits, FilesystemAccess, RuntimePolicy};

use crate::RuntimeError;

/// One page of linear memory, the unit a memory limit is expressed in.
pub const PAGE_BYTES: u64 = 64 * 1024;

/// Everything one invocation carries besides its modules.
pub(crate) struct HostState {
    /// The WASI context, present only when the policy granted WASI.
    pub(crate) wasi: Option<wasmtime_wasi::p1::WasiP1Ctx>,
    /// The bounds this invocation runs under, for reporting which one was hit.
    pub(crate) limits: ExecutionLimits,
    /// The memory ceiling itself, so a refused growth can be told from any other trap.
    pub(crate) memory_limit: Option<u64>,
    /// The memory ceiling the engine enforces.
    pub(crate) engine_limits: StoreLimits,
    /// The reason a host function failed, if one did.
    pub(crate) host_error: Option<String>,
    /// Set by the watchdog when an invocation ran out of wall-clock time.
    pub(crate) timed_out: Arc<AtomicBool>,
}

/// Builds an engine configured for the policy's limits.
///
/// Epoch interruption is enabled only when a wall-clock budget exists: a policy without
/// one should not pay for a mechanism it will never use, and a caller can tell from the
/// configuration whether an invocation can be interrupted at all.
pub(crate) fn engine_for(policy: &RuntimePolicy) -> Engine {
    let mut config = Config::new();
    if policy.limits().wall_clock().is_some() {
        config.epoch_interruption(true);
    }
    Engine::new(&config).expect("the default engine configuration is always valid")
}

/// Builds the state one invocation starts with.
///
/// Fallible because a mount that cannot be opened is a caller's mistake to report, not
/// the runtime's to panic over: `wf run` has to be able to say which path failed.
pub(crate) fn state_for(policy: &RuntimePolicy) -> Result<HostState, RuntimeError> {
    let mut limits = StoreLimitsBuilder::new();
    if let Some(pages) = policy.limits().memory_pages() {
        limits = limits
            .memory_size((pages * PAGE_BYTES) as usize)
            // Failing the growth by returning -1 would let a guest ignore the answer and
            // fault later somewhere far from the cause. A trap names the allocation.
            .trap_on_grow_failure(true);
    }

    let wasi = if policy.wasi_enabled() {
        Some(wasi_context(policy)?)
    } else {
        None
    };

    Ok(HostState {
        wasi,
        limits: policy.limits(),
        memory_limit: policy.limits().memory_pages(),
        engine_limits: limits.build(),
        host_error: None,
        timed_out: Arc::new(AtomicBool::new(false)),
    })
}

/// Builds the WASI context a policy describes.
///
/// Only what the policy grants appears in it: the standard streams so a guest's output is
/// visible, the mounts it declared, and nothing else. The host environment and arguments
/// are deliberately absent — the default is that a guest sees no secrets and learns
/// nothing about the machine it is running on.
fn wasi_context(policy: &RuntimePolicy) -> Result<wasmtime_wasi::p1::WasiP1Ctx, RuntimeError> {
    let mut builder = WasiCtxBuilder::new();

    // The guest's own output is the point of running it, so the streams are inherited.
    // Everything else about the host stays hidden.
    builder.inherit_stdio();

    for mount in policy.mounts() {
        let permissions = match mount.access() {
            FilesystemAccess::ReadOnly => FsPerms::ReadOnly,
            FilesystemAccess::ReadWrite => FsPerms::ReadWrite,
        };
        builder
            .preopened_dir(mount.host(), mount.guest(), permissions)
            .map_err(|error| RuntimeError::Host {
                message: format!(
                    "cannot mount `{}` at `{}` for the guest: {error}",
                    mount.host(),
                    mount.guest()
                ),
            })?;
    }

    if policy.environment_visible() {
        builder.inherit_env();
    }

    Ok(builder.build_p1())
}

/// Registers WASI in the linker, but only when the policy granted it.
///
/// When it did not, the guest's WASI imports have no provider at all, which is a stronger
/// answer than providing them with every permission removed.
pub(crate) fn install_wasi(
    linker: &mut Linker<HostState>,
    policy: &RuntimePolicy,
) -> Result<(), RuntimeError> {
    if !policy.wasi_enabled() {
        return Ok(());
    }

    wasmtime_wasi::p1::add_to_linker_sync(linker, |state| {
        state
            .wasi
            .as_mut()
            .expect("WASI is registered only when the policy built a context for it")
    })
    .map_err(|error| RuntimeError::Host {
        message: format!("cannot register WASI for the guest: {error}"),
    })
}

/// Lets the engine interrupt an invocation that ran out of wall-clock time.
///
/// Returns the flag the watchdog will set, so the caller can tell a timeout from a trap:
/// both surface as a failed call, and they are not the same diagnosis.
pub(crate) fn arm_timeout(
    engine: &Engine,
    store: &mut Store<HostState>,
    wall_clock: Option<Duration>,
) -> Arc<AtomicBool> {
    let timed_out = store.data().timed_out.clone();

    let Some(wall_clock) = wall_clock else {
        return timed_out;
    };

    store.set_epoch_deadline(1);

    let engine = engine.clone();
    let flag = Arc::clone(&timed_out);
    std::thread::spawn(move || {
        std::thread::sleep(wall_clock);
        // The engine is shared, and only stores with a pending deadline can be
        // interrupted, so this cannot stop an invocation that is not over budget.
        engine.increment_epoch();
        flag.store(true, Ordering::SeqCst);
    });

    timed_out
}

/// Whether an invocation was stopped for running out of time.
pub(crate) fn was_interrupted(timed_out: &Arc<AtomicBool>) -> bool {
    timed_out.load(Ordering::SeqCst)
}

/// How an execution limit failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// The invocation ran longer than its wall-clock budget.
    WallClock,
    /// The guest asked for more memory than it was allowed.
    Memory,
}

impl std::fmt::Display for Limit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Limit::WallClock => "the guest ran past its wall-clock budget",
            Limit::Memory => "the guest asked for more memory than it was allowed",
        })
    }
}

/// The phrase Wasmtime uses when a growth is refused by the store's limits.
///
/// Pinned by a test rather than trusted: the runtime classifies a failure by looking for
/// this, so the test fails loudly if a future Wasmtime words it differently.
pub(crate) const MEMORY_LIMIT_PHRASE: &str = "forcing trap when growing memory";

/// The message for a limit failure, with the policy's numbers in it.
pub(crate) fn limit_message(limit: Limit, limits: &ExecutionLimits) -> String {
    match limit {
        Limit::WallClock => format!(
            "the guest ran past its {}s budget and was stopped",
            limits
                .wall_clock()
                .map(|wall_clock| wall_clock.as_secs_f64())
                .unwrap_or_default()
        ),
        Limit::Memory => format!(
            "the guest asked for more memory than the {} pages it was allowed",
            limits.memory_pages().unwrap_or_default()
        ),
    }
}
