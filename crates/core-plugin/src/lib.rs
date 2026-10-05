//! Same-build plugins depend only on these types, never on the host or its storage.
pub struct Manifest {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub commands: &'static [Command],
    pub resources: &'static [Resource],
    pub setup_note: &'static str,
    pub setup_files: &'static [SetupFile],
}
pub struct Command {
    pub name: &'static str,
    pub capture: Option<Operation>,
}
#[non_exhaustive]
pub enum Operation {
    Route,
}
pub struct Resource {
    pub kind: ResourceKind,
    pub name: &'static str,
    pub revision: u32,
    pub files: &'static [ResourceFile],
}
#[non_exhaustive]
pub enum ResourceKind {
    AgentSkill,
}
pub struct ResourceFile {
    pub path: &'static str,
    pub bytes: &'static [u8],
}
pub struct SetupFile {
    pub label: &'static str,
    pub resource: &'static str,
    pub path: &'static str,
}
#[non_exhaustive]
pub enum Begin {
    Route(RouteBegin),
}
pub struct RouteBegin {
    pub router_model: String,
    pub router_version: Option<String>,
    pub rules_version: Option<String>,
    pub summary: Vec<u8>,
    pub request: Vec<u8>,
}
#[non_exhaustive]
pub enum End {
    Route(RouteEnd),
}
pub struct RouteEnd {
    pub response: Captured,
    pub suggestion: Captured,
}
pub enum Captured {
    Bytes(Vec<u8>),
    Missing(Missing),
}
#[non_exhaustive]
pub enum Missing {
    NotAvailable,
    Unrecognized,
    TooLarge,
}
pub trait Recorder {
    /// Call at most once, immediately before the external effect. No capture feedback is
    /// exposed: the business operation must not branch on storage availability.
    fn begin(&mut self, begin: Begin);
}
pub struct Call<'a> {
    pub command: &'a str,
    pub stdin: &'a mut dyn std::io::Read,
    pub recorder: &'a mut dyn Recorder,
}
pub struct Completion {
    /// Business codes are 0..=124; higher values are host errors with unknown outcome.
    pub exit_code: u8,
    pub stdout: Vec<u8>,
    pub end: Option<End>,
}
pub trait CorePlugin: Sync {
    fn manifest(&self) -> &'static Manifest;
    /// Return output bytes; do not write stdout/stderr directly.
    fn run(&self, call: Call<'_>) -> Completion;
}
