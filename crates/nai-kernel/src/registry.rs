use nai_contracts::{CAPABILITY_IDS, v1::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn error(code: ErrorCode, reason: &str) -> ContractError {
    ContractError {
        code: code as i32,
        category: "binding".into(),
        message: reason.into(),
        ..Default::default()
    }
}
type Result<T> = std::result::Result<T, ContractError>;
fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn known(cap: &str) -> bool {
    CAPABILITY_IDS.binary_search(&cap).is_ok()
}
fn hard_dependencies(cap: &str) -> &'static [&'static str] {
    match cap {
        "output.deliver@1" | "tts.synthesize@1" => &["guard.check@1"],
        "avatar.render@1" => &["avatar.profile@1", "guard.check@1"],
        _ => &[],
    }
}

#[derive(Clone)]
pub struct CallContext {
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    utc_base: u64,
    started: Instant,
}
impl CallContext {
    pub fn new(timeout: Duration) -> Result<Self> {
        if timeout > Duration::from_secs(30) {
            return Err(error(ErrorCode::InvalidArgument, "CALL_BUDGET_INVALID"));
        }
        let started = Instant::now();
        let utc_base = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| error(ErrorCode::Internal, "CLOCK_INVALID"))?
            .as_millis()
            .try_into()
            .map_err(|_| error(ErrorCode::Internal, "CLOCK_RANGE"))?;
        Ok(Self {
            deadline: started + timeout,
            cancelled: Arc::new(AtomicBool::new(false)),
            utc_base,
            started,
        })
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
    pub fn check(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(error(ErrorCode::Cancelled, "CALL_CANCELLED"));
        }
        if Instant::now() >= self.deadline {
            return Err(error(ErrorCode::DeadlineExceeded, "CALL_EXPIRED"));
        }
        Ok(())
    }
    pub fn now_utc_ms(&self) -> u64 {
        self.utc_base
            .saturating_add(self.started.elapsed().as_millis().min(u64::MAX as u128) as u64)
    }
}
#[derive(Clone, Copy)]
pub struct Limits {
    pub plugins: usize,
    pub sessions: usize,
    pub dependencies: usize,
    pub depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            plugins: 256,
            sessions: 32,
            dependencies: 64,
            depth: 32,
        }
    }
}
/// Only trusted composition/installer code supplies independently verified
/// evidence. This value is not deserialized from a plugin's claims.
#[derive(Clone, Debug, PartialEq)]
pub struct Admission {
    pub provider: BoundProvider,
    pub review_ref: String,
    pub license_ref: String,
    pub resource_profile_ref: String,
    pub certification_ref: String,
    pub host_boundary_ref: String,
    pub compiled_inproc: bool,
}
#[derive(Clone)]
struct Health {
    response: HealthResponse,
    expires: u64,
}
#[derive(Clone)]
struct Plugin {
    registration: PluginRegistration,
    host: i32,
    health: Option<Health>,
}
#[derive(Clone)]
struct Frozen {
    snapshot: BindingSnapshot,
    plan: BindingPlan,
}

pub struct Registry {
    boot_id: String,
    limits: Limits,
    revision: u64,
    sequence: u64,
    admissions: BTreeMap<String, Admission>,
    plugins: BTreeMap<String, Plugin>,
    permissions: BTreeMap<(String, String), PermissionSnapshot>,
    automatic: BTreeMap<String, AutomaticReadiness>,
    frozen: BTreeMap<String, Frozen>,
}
impl Registry {
    pub fn new(boot_id: &str, limits: Limits) -> Result<Self> {
        if !identifier(boot_id)
            || limits.plugins == 0
            || limits.plugins > 1024
            || limits.sessions == 0
            || limits.sessions > 1024
            || limits.dependencies == 0
            || limits.dependencies > 64
            || limits.depth == 0
            || limits.depth > 64
        {
            return Err(error(ErrorCode::InvalidArgument, "REGISTRY_LIMITS_INVALID"));
        }
        Ok(Self {
            boot_id: boot_id.into(),
            limits,
            revision: 0,
            sequence: 0,
            admissions: BTreeMap::new(),
            plugins: BTreeMap::new(),
            permissions: BTreeMap::new(),
            automatic: BTreeMap::new(),
            frozen: BTreeMap::new(),
        })
    }
    fn changed(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| error(ErrorCode::ResourceExhausted, "REVISION_EXHAUSTED"))?;
        Ok(())
    }
    fn referenced(&self, id: &str) -> bool {
        self.frozen.values().any(|f| {
            f.snapshot.bindings.iter().any(|b| {
                b.selected.as_ref().is_some_and(|p| p.plugin_id == id)
                    || b.approved_fallbacks.iter().any(|p| p.plugin_id == id)
            })
        })
    }
    pub fn admit(&mut self, ctx: &CallContext, evidence: Admission) -> Result<()> {
        ctx.check()?;
        let provider = &evidence.provider;
        if !identifier(&provider.plugin_id)
            || !identifier(&provider.plugin_version)
            || !digest(&provider.artifact_hash)
            || !digest(&provider.manifest_hash)
            || !identifier(&provider.policy_scope_ref)
            || !matches!(provider.host, 1..=4)
            || [
                &evidence.review_ref,
                &evidence.license_ref,
                &evidence.resource_profile_ref,
                &evidence.certification_ref,
                &evidence.host_boundary_ref,
            ]
            .iter()
            .any(|v| !identifier(v))
        {
            return Err(error(
                ErrorCode::PermissionDenied,
                "ADMISSION_EVIDENCE_INVALID",
            ));
        }
        if provider.host == HostKind::Inproc as i32 && !evidence.compiled_inproc {
            return Err(error(
                ErrorCode::PermissionDenied,
                "INPROC_NOT_COMPILED_REVIEWED",
            ));
        }
        if self.referenced(&provider.plugin_id) {
            return Err(error(ErrorCode::Conflict, "VERSION_FROZEN"));
        }
        if !self.admissions.contains_key(&provider.plugin_id)
            && self.admissions.len() >= self.limits.plugins
        {
            return Err(error(ErrorCode::ResourceExhausted, "ADMISSION_LIMIT"));
        }
        self.admissions.insert(provider.plugin_id.clone(), evidence);
        self.changed()
    }
    pub fn revoke_admission(&mut self, ctx: &CallContext, id: &str) -> Result<()> {
        ctx.check()?;
        self.admissions.remove(id);
        self.changed()
    }
    pub fn register(
        &mut self,
        ctx: &CallContext,
        registration: PluginRegistration,
        host: HostKind,
    ) -> Result<()> {
        ctx.check()?;
        if !identifier(&registration.plugin_id)
            || registration.validated_kernel_version != env!("CARGO_PKG_VERSION")
            || registration.protocol_ranges.is_empty()
            || nai_contracts::negotiate(
                &[ProtocolRange {
                    major: 1,
                    min_minor: 0,
                    max_minor: 0,
                }],
                &registration.protocol_ranges,
            )
            .is_err()
        {
            return Err(error(
                ErrorCode::VersionIncompatible,
                "REGISTRATION_TARGET_INVALID",
            ));
        }
        if registration.contracts.is_empty()
            || registration.contracts.len() > self.limits.dependencies
            || registration.requires.len() > self.limits.dependencies
            || registration
                .contracts
                .iter()
                .chain(&registration.requires)
                .any(|c| !known(c))
            || registration.contracts.iter().collect::<BTreeSet<_>>().len()
                != registration.contracts.len()
            || registration.requires.iter().collect::<BTreeSet<_>>().len()
                != registration.requires.len()
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "CONTRACT_DECLARATION_INVALID",
            ));
        }
        let evidence = self
            .admissions
            .get(&registration.plugin_id)
            .ok_or_else(|| error(ErrorCode::PermissionDenied, "ADMISSION_ABSENT"))?;
        let provider = &evidence.provider;
        if provider.plugin_version != registration.plugin_version
            || provider.artifact_hash != registration.artifact_hash
            || provider.manifest_hash != registration.manifest_hash
            || provider.host != host as i32
            || !registration.supported_hosts.contains(&(host as i32))
            || evidence.license_ref != registration.license_ref
            || evidence.resource_profile_ref != registration.resource_profile_ref
            || evidence.certification_ref != registration.certification_ref
        {
            return Err(error(ErrorCode::PermissionDenied, "ADMISSION_MISMATCH"));
        }
        if self.referenced(&registration.plugin_id) {
            return Err(error(ErrorCode::Conflict, "VERSION_FROZEN"));
        }
        if !self.plugins.contains_key(&registration.plugin_id)
            && self.plugins.len() >= self.limits.plugins
        {
            return Err(error(ErrorCode::ResourceExhausted, "PLUGIN_LIMIT"));
        }
        let mut tentative = self.plugins.clone();
        tentative.insert(
            registration.plugin_id.clone(),
            Plugin {
                registration,
                host: host as i32,
                health: None,
            },
        );
        let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for plugin in tentative.values() {
            ctx.check()?;
            for cap in &plugin.registration.contracts {
                graph
                    .entry(cap.clone())
                    .or_default()
                    .extend(plugin.registration.requires.iter().cloned());
                graph
                    .entry(cap.clone())
                    .or_default()
                    .extend(hard_dependencies(cap).iter().map(|cap| (*cap).into()));
            }
        }
        fn visit(
            ctx: &CallContext,
            graph: &BTreeMap<String, BTreeSet<String>>,
            node: &str,
            path: &mut BTreeSet<String>,
            done: &mut BTreeSet<String>,
            depth: usize,
            max: usize,
        ) -> Result<()> {
            ctx.check()?;
            if depth > max {
                return Err(error(ErrorCode::ResourceExhausted, "DEPENDENCY_DEPTH"));
            }
            if done.contains(node) {
                return Ok(());
            }
            if !path.insert(node.into()) {
                return Err(error(ErrorCode::Conflict, "DEPENDENCY_CYCLE"));
            }
            if let Some(edges) = graph.get(node) {
                for edge in edges {
                    visit(ctx, graph, edge, path, done, depth + 1, max)?;
                }
            }
            path.remove(node);
            done.insert(node.into());
            Ok(())
        }
        let mut done = BTreeSet::new();
        for node in graph.keys() {
            visit(
                ctx,
                &graph,
                node,
                &mut BTreeSet::new(),
                &mut done,
                0,
                self.limits.depth,
            )?;
        }
        ctx.check()?;
        self.plugins = tentative;
        self.changed()
    }
    pub fn remove(&mut self, ctx: &CallContext, id: &str) -> Result<()> {
        ctx.check()?;
        if self.referenced(id) {
            return Err(error(ErrorCode::Conflict, "VERSION_FROZEN"));
        }
        self.plugins.remove(id);
        self.permissions.retain(|(_, plugin), _| plugin != id);
        self.changed()
    }
    pub fn health(
        &mut self,
        ctx: &CallContext,
        id: &str,
        response: HealthResponse,
        expires: u64,
    ) -> Result<()> {
        ctx.check()?;
        if !identifier(&response.revision) {
            return Err(error(ErrorCode::InvalidArgument, "HEALTH_SOURCE_ABSENT"));
        }
        let plugin = self
            .plugins
            .get_mut(id)
            .ok_or_else(|| error(ErrorCode::CapabilityAbsent, "PLUGIN_ABSENT"))?;
        plugin.health = Some(Health { response, expires });
        self.changed()
    }
    pub fn permissions(
        &mut self,
        ctx: &CallContext,
        session: &str,
        plugin: &str,
        snapshot: PermissionSnapshot,
    ) -> Result<()> {
        ctx.check()?;
        if !identifier(session)
            || !self.plugins.contains_key(plugin)
            || snapshot.capabilities.len() > self.limits.dependencies
        {
            return Err(error(
                ErrorCode::InvalidArgument,
                "PERMISSION_SCOPE_INVALID",
            ));
        }
        let key = (session.into(), plugin.into());
        if !self.permissions.contains_key(&key)
            && self.permissions.len() >= self.limits.plugins * self.limits.sessions
        {
            return Err(error(ErrorCode::ResourceExhausted, "PERMISSION_LIMIT"));
        }
        self.permissions.insert(key, snapshot);
        self.changed()
    }
    pub fn automatic_conditions(
        &mut self,
        ctx: &CallContext,
        session: &str,
        conditions: AutomaticReadiness,
    ) -> Result<()> {
        ctx.check()?;
        if !identifier(session) {
            return Err(error(ErrorCode::InvalidArgument, "SESSION_INVALID"));
        }
        if !self.automatic.contains_key(session) && self.automatic.len() >= self.limits.sessions {
            return Err(error(ErrorCode::ResourceExhausted, "SESSION_LIMIT"));
        }
        self.automatic.insert(session.into(), conditions);
        self.changed()
    }
    fn eligible(&self, ctx: &CallContext, session: &str, plugin: &str, cap: &str) -> Result<()> {
        ctx.check()?;
        let installed = self
            .plugins
            .get(plugin)
            .ok_or_else(|| error(ErrorCode::CapabilityAbsent, "PLUGIN_ABSENT"))?;
        if !installed.registration.contracts.iter().any(|c| c == cap) {
            return Err(error(ErrorCode::CapabilityAbsent, "CAPABILITY_ABSENT"));
        }
        let admitted = self
            .admissions
            .get(plugin)
            .ok_or_else(|| error(ErrorCode::PermissionDenied, "ADMISSION_REVOKED"))?;
        if admitted.provider.plugin_version != installed.registration.plugin_version
            || admitted.provider.artifact_hash != installed.registration.artifact_hash
            || admitted.provider.manifest_hash != installed.registration.manifest_hash
            || admitted.provider.host != installed.host
        {
            return Err(error(ErrorCode::PermissionDenied, "ADMISSION_CHANGED"));
        }
        let health = installed
            .health
            .as_ref()
            .ok_or_else(|| error(ErrorCode::CapabilityNotReady, "HEALTH_ABSENT"))?;
        if !health.response.ready || health.expires <= ctx.now_utc_ms() {
            return Err(error(
                ErrorCode::CapabilityNotReady,
                "HEALTH_FAILED_OR_EXPIRED",
            ));
        }
        let permission = self
            .permissions
            .get(&(session.into(), plugin.into()))
            .ok_or_else(|| error(ErrorCode::PermissionDenied, "PERMISSION_UNKNOWN"))?;
        if permission.revision.is_empty()
            || permission.authority_ref.is_empty()
            || permission
                .valid_until_utc_ms
                .is_none_or(|until| until <= ctx.now_utc_ms())
            || permission.capabilities.get(cap) != Some(&(PermissionState::Granted as i32))
        {
            return Err(error(
                ErrorCode::PermissionDenied,
                "PERMISSION_DENIED_UNKNOWN_OR_EXPIRED",
            ));
        }
        Ok(())
    }
    fn auto_ready(&self, ctx: &CallContext, session: &str) -> Result<()> {
        let conditions = self
            .automatic
            .get(session)
            .ok_or_else(|| error(ErrorCode::PermissionDenied, "AUTO_PREREQUISITES_ABSENT"))?;
        let permissions = conditions
            .platform_permissions
            .as_ref()
            .ok_or_else(|| error(ErrorCode::PermissionDenied, "PLATFORM_PERMISSION_UNKNOWN"))?;
        if permissions.revision.is_empty()
            || permissions.authority_ref.is_empty()
            || permissions
                .valid_until_utc_ms
                .is_none_or(|expiry| expiry <= ctx.now_utc_ms())
            || permissions.capabilities.get("auto.supervised")
                != Some(&(PermissionState::Granted as i32))
        {
            return Err(error(
                ErrorCode::PermissionDenied,
                "PLATFORM_AUTO_NOT_GRANTED",
            ));
        }
        if conditions.supervision_lease_ref.is_empty()
            || conditions.supervision_expires_at_utc_ms <= ctx.now_utc_ms()
            || conditions.budget_admission_ref.is_empty()
        {
            return Err(error(
                ErrorCode::CapabilityNotReady,
                "SUPERVISION_OR_BUDGET_ABSENT",
            ));
        }
        Ok(())
    }
    fn resolve(
        &self,
        ctx: &CallContext,
        plan: &BindingPlan,
        choices: &BTreeMap<String, BindingSelection>,
        cap: &str,
        selected: &mut BTreeMap<String, Binding>,
        path: &mut BTreeSet<String>,
    ) -> Result<bool> {
        ctx.check()?;
        if selected.contains_key(cap) {
            return Ok(false);
        }
        if path.len() >= self.limits.depth {
            return Err(error(ErrorCode::ResourceExhausted, "DEPENDENCY_DEPTH"));
        }
        if !path.insert(cap.into()) {
            return Err(error(ErrorCode::Conflict, "DEPENDENCY_CYCLE"));
        }
        let result = (|| {
            let selection = if let Some(choice) = choices.get(cap) {
                choice.clone()
            } else {
                let providers: Vec<_> = self
                    .plugins
                    .values()
                    .filter(|p| p.registration.contracts.iter().any(|c| c == cap))
                    .collect();
                if providers.len() != 1 {
                    return Err(error(
                        if providers.is_empty() {
                            ErrorCode::CapabilityAbsent
                        } else {
                            ErrorCode::Conflict
                        },
                        if providers.is_empty() {
                            "CAPABILITY_ABSENT"
                        } else {
                            "BINDING_AMBIGUOUS"
                        },
                    ));
                }
                let id = &providers[0].registration.plugin_id;
                let admitted = self
                    .admissions
                    .get(id)
                    .ok_or_else(|| error(ErrorCode::PermissionDenied, "ADMISSION_REVOKED"))?;
                BindingSelection {
                    capability_id: cap.into(),
                    primary_plugin_id: id.clone(),
                    policy_scope_ref: admitted.provider.policy_scope_ref.clone(),
                    ..Default::default()
                }
            };
            let chain: Vec<_> = std::iter::once(selection.primary_plugin_id.clone())
                .chain(selection.fallback_plugin_ids.iter().cloned())
                .collect();
            if !identifier(&selection.policy_scope_ref)
                || chain.len() > 17
                || chain.iter().collect::<BTreeSet<_>>().len() != chain.len()
            {
                return Err(error(ErrorCode::InvalidArgument, "BINDING_CHAIN_INVALID"));
            }
            for id in &chain {
                if self
                    .admissions
                    .get(id)
                    .is_some_and(|a| a.provider.policy_scope_ref != selection.policy_scope_ref)
                {
                    return Err(error(
                        ErrorCode::PermissionDenied,
                        "FALLBACK_POLICY_SCOPE_MISMATCH",
                    ));
                }
            }
            let mut failure = error(ErrorCode::CapabilityAbsent, "CAPABILITY_ABSENT");
            for (index, id) in chain.iter().enumerate() {
                if let Err(error) = self.eligible(ctx, &plan.session_id, id, cap) {
                    if matches!(error.code, 14 | 8) {
                        return Err(error);
                    }
                    failure = error;
                    continue;
                }
                let plugin = &self.plugins[id];
                let mut tentative = selected.clone();
                let mut degraded = index > 0;
                let dependencies: BTreeSet<_> = plugin
                    .registration
                    .requires
                    .iter()
                    .cloned()
                    .chain(hard_dependencies(cap).iter().map(|c| (*c).into()))
                    .collect();
                let mut broken = None;
                for dependency in dependencies {
                    match self.resolve(ctx, plan, choices, &dependency, &mut tentative, path) {
                        Ok(fallback) => degraded |= fallback,
                        Err(error) => {
                            broken = Some(error);
                            break;
                        }
                    }
                }
                if let Some(error) = broken {
                    if matches!(error.code, 14 | 8) {
                        return Err(error);
                    }
                    failure = error;
                    continue;
                }
                let provider = self.admissions[id].provider.clone();
                let approved_fallbacks = chain
                    .iter()
                    .filter(|other| {
                        *other != id
                            && self
                                .plugins
                                .get(*other)
                                .is_some_and(|p| p.registration.contracts.iter().any(|c| c == cap))
                    })
                    .filter_map(|other| self.admissions.get(other).map(|a| a.provider.clone()))
                    .collect::<Vec<_>>();
                tentative.insert(
                    cap.into(),
                    Binding {
                        capability_id: cap.into(),
                        primary_plugin_id: id.clone(),
                        fallback_plugin_ids: approved_fallbacks
                            .iter()
                            .map(|p| p.plugin_id.clone())
                            .collect(),
                        selected: Some(provider),
                        approved_fallbacks,
                    },
                );
                *selected = tentative;
                return Ok(degraded);
            }
            Err(failure)
        })();
        path.remove(cap);
        result
    }
    fn evaluate(&self, ctx: &CallContext, plan: &BindingPlan) -> Result<PrepareReport> {
        ctx.check()?;
        if !identifier(&plan.session_id)
            || !matches!(plan.mode, 1..=3)
            || plan.requirements.is_empty()
            || plan.requirements.len() > self.limits.dependencies
            || plan.selections.len() > self.limits.dependencies
        {
            return Err(error(ErrorCode::InvalidArgument, "PLAN_INVALID"));
        }
        let mut required = BTreeMap::new();
        let mut choices = BTreeMap::new();
        for requirement in &plan.requirements {
            if !known(&requirement.capability_id)
                || required
                    .insert(requirement.capability_id.clone(), requirement.required)
                    .is_some()
            {
                return Err(error(ErrorCode::InvalidArgument, "REQUIREMENT_INVALID"));
            }
        }
        for choice in &plan.selections {
            if !known(&choice.capability_id)
                || !identifier(&choice.primary_plugin_id)
                || !identifier(&choice.policy_scope_ref)
                || choice.fallback_plugin_ids.len() > 16
                || choices
                    .insert(choice.capability_id.clone(), choice.clone())
                    .is_some()
            {
                return Err(error(ErrorCode::InvalidArgument, "SELECTION_INVALID"));
            }
        }
        if plan.emits_output {
            required.insert("output.deliver@1".into(), true);
            required.insert("guard.check@1".into(), true);
        }
        let mut issues = Vec::new();
        let mut not_ready = false;
        let mut degraded = false;
        if plan.mode == SessionMode::AutoSupervised as i32 {
            for cap in [
                "platform.events@1",
                "platform.capabilities@1",
                "platform.lifecycle@1",
                "knowledge.retrieve@1",
                "guard.check@1",
            ] {
                required.insert(cap.into(), true);
            }
            if let Err(error) = self.auto_ready(ctx, &plan.session_id) {
                not_ready = true;
                issues.push(BindingIssue {
                    capability_id: "platform.capabilities@1".into(),
                    code: error.code,
                    reason_code: error.message,
                    required: true,
                    ..Default::default()
                });
            }
        }
        let mut selected = BTreeMap::new();
        for (cap, mandatory) in required {
            let mut tentative = selected.clone();
            match self.resolve(
                ctx,
                plan,
                &choices,
                &cap,
                &mut tentative,
                &mut BTreeSet::new(),
            ) {
                Ok(fallback) => {
                    selected = tentative;
                    if fallback {
                        degraded = true;
                        issues.push(BindingIssue {
                            capability_id: cap,
                            reason_code: "EXPLICIT_FALLBACK_SELECTED".into(),
                            required: mandatory,
                            ..Default::default()
                        });
                    }
                }
                Err(error) => {
                    ctx.check()?;
                    not_ready |= mandatory;
                    degraded |= !mandatory;
                    issues.push(BindingIssue {
                        capability_id: cap,
                        code: error.code,
                        reason_code: error.message,
                        required: mandatory,
                        ..Default::default()
                    });
                }
            }
        }
        let mut descriptors = Vec::new();
        for plugin in self.plugins.values() {
            ctx.check()?;
            for cap in &plugin.registration.contracts {
                let eligible =
                    self.eligible(ctx, &plan.session_id, &plugin.registration.plugin_id, cap);
                let permission = self
                    .permissions
                    .get(&(
                        plan.session_id.clone(),
                        plugin.registration.plugin_id.clone(),
                    ))
                    .filter(|p| {
                        !p.authority_ref.is_empty()
                            && !p.revision.is_empty()
                            && p.valid_until_utc_ms
                                .is_some_and(|expiry| expiry > ctx.now_utc_ms())
                    })
                    .and_then(|p| p.capabilities.get(cap))
                    .copied()
                    .filter(|value| matches!(value, 1 | 2))
                    .unwrap_or(0);
                let bound = selected
                    .get(cap)
                    .and_then(|b| b.selected.as_ref())
                    .is_some_and(|p| p.plugin_id == plugin.registration.plugin_id);
                descriptors.push(CapabilityDescriptor {
                    capability_id: cap.clone(),
                    plugin_id: plugin.registration.plugin_id.clone(),
                    plugin_version: plugin.registration.plugin_version.clone(),
                    host: plugin.host,
                    state: if bound && eligible.is_ok() { 4 } else { 3 },
                    permission,
                    required_capabilities: plugin.registration.requires.clone(),
                    reason_code: eligible
                        .err()
                        .map(|error| error.message)
                        .unwrap_or_else(|| {
                            if bound {
                                "BOUND".into()
                            } else {
                                "NOT_SELECTED".into()
                            }
                        }),
                    artifact_hash: plugin.registration.artifact_hash.clone(),
                    manifest_hash: plugin.registration.manifest_hash.clone(),
                    policy_scope_ref: self
                        .admissions
                        .get(&plugin.registration.plugin_id)
                        .map(|a| a.provider.policy_scope_ref.clone())
                        .unwrap_or_default(),
                });
            }
        }
        ctx.check()?;
        Ok(PrepareReport {
            readiness: if not_ready {
                1
            } else if degraded {
                3
            } else {
                2
            },
            capability_view: Some(CapabilityView {
                session_id: plan.session_id.clone(),
                revision: self.revision.to_string(),
                capabilities: descriptors,
            }),
            snapshot: (!not_ready).then(|| BindingSnapshot {
                session_id: plan.session_id.clone(),
                revision: self.revision.to_string(),
                bindings: selected.into_values().collect(),
                ..Default::default()
            }),
            issues,
        })
    }
    pub fn prepare(&self, ctx: &CallContext, plan: &BindingPlan) -> Result<PrepareReport> {
        self.evaluate(ctx, plan)
    }
    pub fn freeze(&mut self, ctx: &CallContext, plan: &BindingPlan) -> Result<PrepareReport> {
        ctx.check()?;
        if self.frozen.contains_key(&plan.session_id) {
            return Err(error(ErrorCode::Conflict, "SESSION_ALREADY_FROZEN"));
        }
        if self.frozen.len() >= self.limits.sessions {
            return Err(error(ErrorCode::ResourceExhausted, "SESSION_LIMIT"));
        }
        let mut report = self.evaluate(ctx, plan)?;
        if let Some(snapshot) = report.snapshot.as_mut() {
            self.sequence = self.sequence.checked_add(1).ok_or_else(|| {
                error(ErrorCode::ResourceExhausted, "SNAPSHOT_SEQUENCE_EXHAUSTED")
            })?;
            snapshot.snapshot_id =
                format!("{}:{}:{}", self.boot_id, plan.session_id, self.sequence);
            ctx.check()?;
            self.frozen.insert(
                plan.session_id.clone(),
                Frozen {
                    snapshot: snapshot.clone(),
                    plan: plan.clone(),
                },
            );
        }
        Ok(report)
    }
    pub fn recheck(&self, ctx: &CallContext, snapshot: &BindingSnapshot) -> Result<()> {
        ctx.check()?;
        let frozen = self
            .frozen
            .get(&snapshot.session_id)
            .ok_or_else(|| error(ErrorCode::Conflict, "SNAPSHOT_NOT_OWNED"))?;
        if frozen.snapshot != *snapshot {
            return Err(error(ErrorCode::Conflict, "SNAPSHOT_CHANGED"));
        }
        if frozen.plan.mode == SessionMode::AutoSupervised as i32 {
            self.auto_ready(ctx, &snapshot.session_id)?;
        }
        for binding in &snapshot.bindings {
            let selected = binding
                .selected
                .as_ref()
                .ok_or_else(|| error(ErrorCode::VersionIncompatible, "UNVERSIONED_BINDING"))?;
            self.eligible(
                ctx,
                &snapshot.session_id,
                &selected.plugin_id,
                &binding.capability_id,
            )?;
            if self
                .admissions
                .get(&selected.plugin_id)
                .map(|a| &a.provider)
                != Some(selected)
            {
                return Err(error(ErrorCode::Conflict, "PROVIDER_IDENTITY_CHANGED"));
            }
        }
        Ok(())
    }
    pub fn release(&mut self, ctx: &CallContext, session: &str) -> Result<()> {
        ctx.check()?;
        self.frozen.remove(session);
        self.permissions.retain(|(id, _), _| id != session);
        self.automatic.remove(session);
        self.changed()
    }
}

#[cfg(test)]
mod tests;
