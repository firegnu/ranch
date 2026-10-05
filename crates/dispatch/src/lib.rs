// From Saddle `plugins/dispatch/src/lib.rs` at commit `c21674a`: the built-in plugin manifest
// is replaced by `route` and the skill files for the `ranch dispatch` commands; no telemetry.
//! Routing and the corral-dispatch skill behind `ranch dispatch`.
mod json;
mod route;
#[cfg(test)]
mod route_tests;
mod rules;
mod skills;
mod transport;

pub use route::Completion;
pub use skills::install as install_skills;

/// Reads the task summary from `stdin` and asks the router. Never writes stdout itself.
pub fn route(stdin: &mut dyn std::io::Read) -> Completion {
    route::run(
        stdin,
        std::env::var("TYPESAFE_API_KEY").ok().as_deref(),
        transport::post,
        std::thread::sleep,
    )
}

/// The corral-dispatch skill, installed as `<agent skills>/corral-dispatch/<path>`.
pub(crate) const SKILL: &[(&str, &[u8])] = &[
    (
        "SKILL.md",
        include_bytes!("../resources/corral-dispatch/SKILL.md"),
    ),
    (
        "项目AGENTS模板.md",
        include_bytes!("../resources/corral-dispatch/项目AGENTS模板.md"),
    ),
    (
        "README.md",
        include_bytes!("../resources/corral-dispatch/README.md"),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_key_is_a_business_failure_without_request() {
        let result = route::run(
            &mut &b"synthetic"[..],
            None,
            |_, _, _| panic!("no request without key"),
            |_| panic!("no sleep"),
        );
        assert_eq!(result.exit_code, 1);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap(),
            serde_json::json!({"ok":false,"error":"TYPESAFE_API_KEY is not set"})
        );
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn original_template_is_pinned_to_bytes() {
        let template = SKILL
            .iter()
            .find(|(name, _)| *name == "项目AGENTS模板.md")
            .unwrap()
            .1;
        assert_eq!(
            format!("{:x}", Sha256::digest(template)),
            "22339f4674469d1b8a5041086f4aa3e5a4b5e9d53f356f946dd1a06cc2f73384"
        );
    }
}
