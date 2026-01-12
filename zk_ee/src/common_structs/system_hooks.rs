use crate::internal_error;
use crate::storage_types::MAX_EVENT_TOPICS;
use crate::system::errors::internal::InternalError;
use crate::system::{
    errors::system::SystemError, CompletedExecution, ExternalCallRequest, System, SystemTypes,
};
use crate::types_config::SystemIOTypesConfig;
use alloc::collections::BTreeMap;
use core::{alloc::Allocator, mem::MaybeUninit};

/// System call hooks process the given call request.
///
/// The inputs are:
/// - call request
/// - caller ee(logic may depend on it some cases)
/// - system
/// - output buffer
pub struct SystemCallHook<S: SystemTypes>(
    for<'a> fn(
        ExternalCallRequest<S>,
        u8,
        &mut System<S>,
        &'a mut [MaybeUninit<u8>],
    ) -> Result<(CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]), SystemError>,
);

impl<S: SystemTypes> SystemCallHook<S> {
    pub fn new(
        f: for<'a> fn(
            ExternalCallRequest<S>,
            u8,
            &mut System<S>,
            &'a mut [MaybeUninit<u8>],
        ) -> Result<
            (CompletedExecution<'a, S>, &'a mut [MaybeUninit<u8>]),
            SystemError,
        >,
    ) -> Self {
        Self(f)
    }
}

/// System event hooks process the given event.
/// These are just used to report information from
/// system contracts to ZKsync OS.
///
/// The inputs are:
/// - topics
/// - data
/// - caller ee(logic may depend on it some cases)
/// - system
pub struct SystemEventHook<S: SystemTypes>(
    for<'a> fn(
        &arrayvec::ArrayVec<<S::IOTypes as SystemIOTypesConfig>::EventKey, MAX_EVENT_TOPICS>,
        &[u8],
        u8,
        &mut System<S>,
        &mut S::Resources,
    ) -> Result<(), SystemError>,
);

impl<S: SystemTypes> SystemEventHook<S> {
    pub fn new(
        f: for<'a> fn(
            &arrayvec::ArrayVec<<S::IOTypes as SystemIOTypesConfig>::EventKey, MAX_EVENT_TOPICS>,
            &[u8],
            u8,
            &mut System<S>,
            &mut S::Resources,
        ) -> Result<(), SystemError>,
    ) -> Self {
        Self(f)
    }
}

///
/// System hooks storage.
/// Stores hooks implementations and processes calls to system addresses.
///
pub struct HooksStorage<S: SystemTypes, A: Allocator + Clone> {
    call_hooks: BTreeMap<u16, SystemCallHook<S>, A>,
    event_hooks: BTreeMap<u32, SystemEventHook<S>, A>,
}

impl<S: SystemTypes, A: Allocator + Clone> HooksStorage<S, A> {
    ///
    /// Creates empty hooks storage with a given allocator.
    ///
    pub fn new_in(allocator: A) -> Self {
        Self {
            call_hooks: BTreeMap::new_in(allocator.clone()),
            event_hooks: BTreeMap::new_in(allocator),
        }
    }

    ///
    /// Adds a new call hook into a given address.
    /// Fails if there was another hook registered there before.
    ///
    pub fn add_call_hook(
        &mut self,
        for_address_low: u16,
        hook: SystemCallHook<S>,
    ) -> Result<(), InternalError> {
        let existing = self.call_hooks.insert(for_address_low, hook);
        if existing.is_some() {
            return Err(internal_error!("System call hook already registered"));
        }
        Ok(())
    }

    ///
    /// Adds a new event hook into a given address.
    /// Fails if there was another hook registered there before.
    ///
    pub fn add_event_hook(
        &mut self,
        for_address_low: u32,
        hook: SystemEventHook<S>,
    ) -> Result<(), InternalError> {
        let existing = self.event_hooks.insert(for_address_low, hook);
        if existing.is_some() {
            return Err(internal_error!("System event hook already registered"));
        }
        Ok(())
    }

    ///
    /// Intercepts calls to low addresses (< 2^16) and executes hooks
    /// stored under that address. If no hook is stored there, return `Ok(None)`.
    /// Always return unused return_memory.
    ///
    pub fn try_intercept<'a>(
        &mut self,
        address_low: u16,
        request: ExternalCallRequest<S>,
        caller_ee: u8,
        system: &mut System<S>,
        return_memory: &'a mut [MaybeUninit<u8>],
    ) -> Result<(Option<CompletedExecution<'a, S>>, &'a mut [MaybeUninit<u8>]), SystemError> {
        let Some(hook) = self.call_hooks.get(&address_low) else {
            return Ok((None, return_memory));
        };
        let (res, remaining_memory) = hook.0(request, caller_ee, system, return_memory)?;

        Ok((Some(res), remaining_memory))
    }

    /// Intercepts events emitted from low addresses (< 2^16) and executes hooks
    /// stored under that address. If no hook is stored there, return `Ok(None)`.
    ///
    pub fn try_intercept_event(
        &mut self,
        address_low: u32,
        topics: &arrayvec::ArrayVec<
            <S::IOTypes as SystemIOTypesConfig>::EventKey,
            MAX_EVENT_TOPICS,
        >,
        data: &[u8],
        caller_ee: u8,
        system: &mut System<S>,
        resources: &mut S::Resources,
    ) -> Result<Option<()>, SystemError> {
        let Some(hook) = self.event_hooks.get(&address_low) else {
            return Ok(None);
        };
        hook.0(topics, data, caller_ee, system, resources)?;

        Ok(Some(()))
    }

    ///
    /// Checks if there is a call hook stored for a given low address (<16 bits).
    ///
    pub fn has_hook_for(&mut self, address_low: u16) -> bool {
        self.call_hooks.contains_key(&address_low)
    }

    ///
    /// Iterate over all addresses with a call hook.
    ///
    pub fn all_call_hooked_addresses_iter(&'_ self) -> impl Iterator<Item = u16> + '_ {
        self.call_hooks.keys().copied()
    }
}
