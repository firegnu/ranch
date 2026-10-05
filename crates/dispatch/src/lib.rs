//! Optional routing business and resources. Only the binary composition root imports this crate.
mod json;
mod route;
#[cfg(test)]
mod route_tests;
mod rules;
mod transport;

use saddle_core_plugin::{
    Call, Command, Completion, CorePlugin, Manifest, Operation, Resource, ResourceFile,
    ResourceKind, SetupFile,
};

pub struct Dispatch;
pub static PLUGIN: Dispatch = Dispatch;

static MANIFEST: Manifest = Manifest {
    id: "dispatch",
    name: "Dispatch",
    version: env!("CARGO_PKG_VERSION"),
    commands: &[Command {
        name: "route",
        capture: Some(Operation::Route),
    }],
    resources: &[Resource {
        kind: ResourceKind::AgentSkill,
        name: "corral-dispatch",
        revision: 3,
        files: &[
            ResourceFile {
                path: "SKILL.md",
                bytes: include_bytes!("../resources/corral-dispatch/SKILL.md"),
            },
            ResourceFile {
                path: "项目AGENTS模板.md",
                bytes: include_bytes!("../resources/corral-dispatch/项目AGENTS模板.md"),
            },
            ResourceFile {
                path: "遥测操作.md",
                bytes: include_bytes!("../resources/corral-dispatch/遥测操作.md"),
            },
            ResourceFile {
                path: "README.md",
                bytes: include_bytes!("../resources/corral-dispatch/README.md"),
            },
        ],
    }],
    setup_note: "Routing: saddle plugin run dispatch route\nThe corral-dispatch skill is installed with this plugin.\n\nProject setup\nTo use controller-led dispatch by default, copy section 1 of the template below into the project's AGENTS.md. Replace the placeholders and reconcile existing rules. Other projects are unchanged; explicit requests can also opt in for one conversation.\n\nRequirements\nSet TYPESAFE_API_KEY for routing. Without it, the controller chooses the route; delegation remains available.\n\nTelemetry\nInitially off. Settings → General controls recording; routing capture also needs an explicit recording context.",
    setup_files: &[SetupFile {
        label: "Template",
        resource: "corral-dispatch",
        path: "项目AGENTS模板.md",
    }],
};

impl CorePlugin for Dispatch {
    fn manifest(&self) -> &'static Manifest {
        &MANIFEST
    }
    fn run(&self, call: Call<'_>) -> Completion {
        route::run(
            call,
            std::env::var("TYPESAFE_API_KEY").ok().as_deref(),
            transport::post,
            std::thread::sleep,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NoBegin;
    impl saddle_core_plugin::Recorder for NoBegin {
        fn begin(&mut self, _: saddle_core_plugin::Begin) {
            panic!("no key must not begin");
        }
    }
    #[test]
    fn absent_key_is_a_business_failure_without_begin() {
        let result = route::run(
            Call {
                command: "route",
                stdin: &mut &b"synthetic"[..],
                recorder: &mut NoBegin,
            },
            None,
            |_, _, _| panic!("no request without key"),
            |_| panic!("no sleep"),
        );
        assert_eq!(result.exit_code, 1);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap(),
            serde_json::json!({"ok":false,"error":"TYPESAFE_API_KEY is not set"})
        );
        assert!(result.end.is_none());
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn revision_and_original_template_are_pinned_to_bytes() {
        let resource = &PLUGIN.manifest().resources[0];
        let mut list = resource
            .files
            .iter()
            .map(|file| (file.path, format!("{:x}", Sha256::digest(file.bytes))))
            .collect::<Vec<_>>();
        list.sort();
        let fingerprint = format!(
            "sha256:{:x}",
            Sha256::digest(serde_json::to_vec(&list).unwrap())
        );
        assert_eq!(
            (resource.revision, fingerprint.as_str()),
            (
                3,
                "sha256:efb2ff8bef91ef8d52772c267ace1ee65b05b8f70e9098c78931b9e2219e5407"
            )
        );
        assert_eq!(
            list.iter()
                .find(|(name, _)| *name == "项目AGENTS模板.md")
                .unwrap()
                .1,
            "22339f4674469d1b8a5041086f4aa3e5a4b5e9d53f356f946dd1a06cc2f73384"
        );
    }
}
