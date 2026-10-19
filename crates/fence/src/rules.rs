use crate::{Result, data, error};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn strings(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|a| a.iter().all(Value::is_string))
}

// Match srt 0.0.80's domain-pattern.js allow-list grammar, including its
// wildcard rules, bracketed IPv6 zone IDs and non-zero, unpadded port suffixes.
fn valid_domain(entry: &str) -> bool {
    let valid_port = |port: &str| {
        !port.starts_with('0')
            && port.bytes().all(|b| b.is_ascii_digit())
            && port.parse::<u16>().is_ok_and(|p| p > 0)
    };
    let host = if let Some(inner) = entry.strip_prefix('[') {
        let Some((host, rest)) = inner.split_once(']') else {
            return false;
        };
        if !rest.is_empty() && !rest.strip_prefix(':').is_some_and(valid_port) {
            return false;
        }
        let host = host.split('%').next().unwrap();
        if host.parse::<std::net::Ipv6Addr>().is_ok() {
            return true;
        }
        host
    } else if let Some((host, port)) = entry.split_once(':') {
        if !valid_port(port) {
            return false;
        }
        host
    } else {
        entry
    };
    if host.contains(['/', ':']) {
        return false;
    }
    if host == "localhost" {
        return true;
    }
    if let Some(domain) = host.strip_prefix("*.") {
        return domain.contains('.') && domain.split('.').all(|part| !part.is_empty());
    }
    !host.contains('*') && host.contains('.') && !host.starts_with('.') && !host.ends_with('.')
}

pub fn validate(value: &Value, source: &str, role: &str, section: &str) -> Result<()> {
    let table = value
        .as_object()
        .ok_or_else(|| data::problem(source, section, "expected table"))?;
    for (key, v) in table {
        let path = if section.is_empty() {
            key.clone()
        } else {
            format!("{section}.{key}")
        };
        let allowed = match key.as_str() {
            "read" | "write" | "domains" => true,
            "description" => section.is_empty() && matches!(role, "profile" | "agent" | "service"),
            "deny_read" => role == "profile",
            "deny_write" | "env" | "unix_sockets" => matches!(role, "profile" | "agent" | "local"),
            "mkdir" => matches!(role, "profile" | "local" | "repo"),
            "args" => role == "agent",
            "services" => matches!(role, "profile" | "agent" | "local"),
            "macos" | "linux" => {
                section.is_empty() && matches!(role, "profile" | "agent" | "local")
            }
            "allow_pty" | "weaker_network_isolation" => section == "macos",
            _ => false,
        };
        if !allowed {
            return Err(data::problem(source, &path, "key is not allowed here"));
        }
        let valid = match key.as_str() {
            "description" => v.is_string(),
            "env" => v.as_object().is_some_and(|m| {
                m.iter()
                    .all(|(k, v)| !k.is_empty() && !k.contains(['=', '\0']) && v.is_string())
            }),
            "services" if role != "agent" => v.as_object().is_some_and(|m| m.values().all(strings)),
            "macos" | "linux" => {
                validate(v, source, role, key)?;
                true
            }
            "allow_pty" | "weaker_network_isolation" => v.is_boolean(),
            _ => strings(v),
        };
        if !valid {
            return Err(data::problem(source, &path, "wrong value type"));
        }
        if matches!(
            key.as_str(),
            "read" | "write" | "deny_read" | "deny_write" | "mkdir" | "unix_sockets"
        ) {
            for entry in v.as_array().unwrap() {
                if entry.as_str().unwrap().contains(['*', '?', '[']) {
                    return Err(data::problem(
                        source,
                        &path,
                        "path wildcards are not supported",
                    ));
                }
            }
        }
    }
    Ok(())
}

// Platform selection is pure, so both rule sets are checked on either host OS.
pub fn select(value: &Value, platform: &str) -> Value {
    let mut out = value.clone();
    out.as_object_mut().unwrap().remove("linux");
    out.as_object_mut().unwrap().remove("macos");
    if let Some(table) = value[platform].as_object() {
        for (k, v) in table {
            if let Some(a) = out[k].as_array_mut()
                && let Some(b) = v.as_array()
            {
                a.extend(b.iter().cloned());
                continue;
            }
            if let Some(a) = out[k].as_object_mut()
                && let Some(b) = v.as_object()
            {
                a.extend(b.clone());
                continue;
            }
            out[k] = v.clone();
        }
    }
    out
}

fn git(cwd: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

struct Context {
    values: BTreeMap<String, Option<String>>,
    worktree: PathBuf,
    cwd: PathBuf,
}
impl Context {
    fn new(cwd: &Path, tmp: Option<&Path>) -> Result<Self> {
        let cwd = cwd
            .canonicalize()
            .ok()
            .filter(|p| p.is_dir())
            .ok_or_else(|| error("bad_cwd", format!("no directory: {}", cwd.display())))?;
        let worktree = git(&cwd, &["rev-parse", "--show-toplevel"])
            .map(PathBuf::from)
            .unwrap_or_else(|| cwd.clone());
        let common = git(
            &cwd,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        );
        let repo = common
            .as_ref()
            .and_then(|s| Path::new(s).parent())
            .unwrap_or(&worktree);
        let values = [
            ("home", env::var("HOME").ok()),
            ("cwd", Some(cwd.to_string_lossy().into_owned())),
            ("worktree", Some(worktree.to_string_lossy().into_owned())),
            ("repo", Some(repo.to_string_lossy().into_owned())),
            ("git_dir", git(&cwd, &["rev-parse", "--absolute-git-dir"])),
            ("git_common", common),
            (
                "branch",
                git(&cwd, &["symbolic-ref", "--short", "-q", "HEAD"]),
            ),
            (
                "tmp",
                Some(
                    tmp.map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "<运行时才有>".into()),
                ),
            ),
            (
                "target",
                env::var("CARGO_TARGET_DIR")
                    .ok()
                    .filter(|s| Path::new(s).is_absolute()),
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect();
        Ok(Self {
            values,
            worktree,
            cwd,
        })
    }
    fn expand(
        &self,
        text: &str,
        source: &str,
        key: &str,
        optional: bool,
    ) -> Result<Option<String>> {
        let mut out = String::new();
        let mut rest = text;
        while let Some(i) = rest.find("${") {
            out.push_str(&rest[..i]);
            rest = &rest[i + 2..];
            let end = rest
                .find('}')
                .ok_or_else(|| data::problem(source, key, "unclosed placeholder"))?;
            let token = &rest[..end];
            let name = token.strip_suffix(":json").unwrap_or(token);
            let value = self.values.get(name).ok_or_else(|| {
                data::problem(source, key, format!("unknown placeholder {token}"))
            })?;
            match value {
                Some(v) => out.push_str(&if token.ends_with(":json") {
                    serde_json::to_string(v).unwrap()
                } else {
                    v.clone()
                }),
                None if optional => return Ok(None),
                None => return Err(data::problem(source, key, format!("{name} has no value"))),
            }
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        if out.contains('\0') {
            return Err(data::problem(source, key, "NUL is not allowed"));
        }
        Ok(Some(out))
    }
}

fn append(list: &mut Vec<String>, value: String) {
    if !list.contains(&value) {
        list.push(value);
    }
}

// Walk each component rather than canonicalizing away the links: every link
// node must be protected against replacement, including links in parent paths.
// Missing suffixes are valid only on the original path, never a link's target.
fn write_target(
    path: &Path,
    links: &mut Vec<PathBuf>,
    allow_missing: bool,
) -> std::io::Result<PathBuf> {
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => {
                resolved.push(name);
                match fs::symlink_metadata(&resolved) {
                    Ok(meta) if meta.is_symlink() => {
                        // Bound traversal even if another process is changing the chain.
                        if links.len() >= 40 {
                            return Err(std::io::Error::from_raw_os_error(libc::ELOOP));
                        }
                        links.push(resolved.clone());
                        let target = fs::read_link(&resolved)?;
                        let target = resolved.parent().unwrap().join(target);
                        resolved = write_target(&target, links, false)?;
                    }
                    Ok(_) => {}
                    Err(e) if allow_missing && e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e),
                }
            }
            other => resolved.push(other.as_os_str()),
        }
    }
    Ok(resolved)
}

fn protect_write(list: &mut Vec<String>, path: &Path, source: &str) -> Result<()> {
    let mut links = Vec::new();
    let target = write_target(path, &mut links, true)
        .map_err(|e| data::problem(source, "deny_write", format!("{}: {e}", path.display())))?;
    append(list, normalize(path).to_string_lossy().into_owned());
    for link in links {
        append(list, link.to_string_lossy().into_owned());
    }
    append(list, target.to_string_lossy().into_owned());
    Ok(())
}

pub struct Rules {
    pub value: Value,
    pub mkdir: Vec<PathBuf>,
}

fn network_settings(
    domains: &[String],
    unix_sockets: &[String],
    platform: &str,
    warnings: &mut Vec<String>,
) -> Value {
    let mut network = json!({"allowedDomains":domains,"deniedDomains":[]});
    if !unix_sockets.is_empty() {
        if platform == "macos" {
            network["allowUnixSockets"] = json!(unix_sockets);
        } else {
            warnings.push("unix_sockets: Linux 不能按路径放行 unix socket，已忽略".into());
        }
    }
    network
}

pub fn resolve(
    home: &Path,
    profile: &str,
    kind: Option<&str>,
    cwd: &Path,
    tmp: Option<&Path>,
    read: &[String],
    write: &[String],
) -> Result<Rules> {
    let platform = platform();
    let context = Context::new(cwd, tmp)?;
    let (p, pm) = data::load(home, "profiles", profile)?;
    let local_path = home.join("local.toml");
    let local = optional_file(&local_path, "local")?;
    let mut layers = vec![(p, pm)];
    if let Some(kind) = kind {
        let (agent, meta) = data::load(home, "agents", kind)?;
        let mut services = select(&agent, platform)["services"].clone();
        for value in [&layers[0].0, &local] {
            let selected = select(value, platform);
            if !selected["services"][kind].is_null() {
                services = selected["services"][kind].clone();
            }
        }
        layers.push((agent, meta));
        // Validate before looking at services, so malformed data reports its actual source.
        for (v, m) in &layers {
            validate(v, &m.to_string(), m["source"].as_str().unwrap(), "")?;
        }
        validate(&local, &local_path.to_string_lossy(), "local", "")?;
        let services = services
            .as_array()
            .filter(|a| !a.is_empty())
            .ok_or_else(|| error("no_service", format!("no model service for {kind}")))?;
        for name in services {
            layers.push(data::load(home, "services", name.as_str().unwrap())?);
        }
        let path = context.worktree.join(".fence.toml");
        let repo = optional_file(&path, "repo")?;
        if fs::symlink_metadata(&path).is_ok() {
            layers.push((repo, json!({"source":"repo","path":path})));
        }
    }
    if local_path.exists() {
        layers.push((local, json!({"source":"local","path":local_path})));
    }
    if !read.is_empty() || !write.is_empty() {
        layers.push((
            json!({"read":read,"write":write}),
            json!({"source":"caller"}),
        ));
    }
    let mut lists: BTreeMap<&str, Vec<String>> = [
        "deny_read",
        "read",
        "write",
        "deny_write",
        "domains",
        "unix_sockets",
        "args",
        "mkdir",
    ]
    .into_iter()
    .map(|k| (k, vec![]))
    .collect();
    let mut environment = Map::new();
    let mut warnings = Vec::new();
    let mut skipped = Vec::new();
    let mut settings = json!({});
    for (value, meta) in &layers {
        let source = meta["path"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| meta.to_string());
        validate(value, &source, meta["source"].as_str().unwrap(), "")?;
        let v = select(value, platform);
        if platform != "macos" && value.get("macos").is_some() {
            warnings.push(format!(
                "{source}: macos settings are not applicable on {platform}"
            ));
        }
        for (&key, list) in &mut lists {
            if let Some(entries) = v[key].as_array() {
                for (index, entry) in entries.iter().enumerate() {
                    let entry = entry.as_str().unwrap();
                    let is_path = !matches!(key, "domains" | "args");
                    let Some(mut expanded) = context.expand(entry, &source, key, is_path)? else {
                        skipped.push(json!({"entry":entry,"reason":"placeholder has no value (git, branch or CARGO_TARGET_DIR not set)"}));
                        continue;
                    };
                    if key == "domains" && !valid_domain(&expanded) {
                        return Err(data::problem(
                            &source,
                            &format!("domains[{index}]"),
                            format!("invalid domain pattern or port: {expanded}"),
                        ));
                    }
                    if is_path && !expanded.starts_with("<运行时才有>") {
                        if !Path::new(&expanded).is_absolute() || expanded.contains(['*', '?', '['])
                        {
                            return Err(data::problem(
                                &source,
                                key,
                                format!("path must be absolute without wildcards: {expanded}"),
                            ));
                        }
                        if key == "deny_write" {
                            protect_write(list, Path::new(&expanded), &source)?;
                            continue;
                        }
                        if key == "unix_sockets" {
                            expanded = write_target(Path::new(&expanded), &mut Vec::new(), true)
                                .map_err(|e| {
                                    data::problem(&source, key, format!("{expanded}: {e}"))
                                })?
                                .to_string_lossy()
                                .into_owned();
                        }
                        expanded = normalize(Path::new(&expanded))
                            .to_string_lossy()
                            .into_owned();
                        if key == "read" && !Path::new(&expanded).exists() {
                            skipped
                                .push(json!({"entry":entry,"reason":"read path does not exist"}));
                            continue;
                        }
                    }
                    if key == "args" {
                        list.push(expanded);
                    } else {
                        append(list, expanded);
                    }
                }
            }
        }
        if let Some(env) = v["env"].as_object() {
            for (k, v) in env {
                let expanded = context
                    .expand(v.as_str().unwrap(), &source, "env", false)?
                    .unwrap();
                if environment.insert(k.clone(), json!(expanded)).is_some() {
                    warnings.push(format!("{source}: env {k} overrides an earlier value"));
                }
            }
        }
        for (key, output) in [
            ("allow_pty", "allowPty"),
            ("weaker_network_isolation", "enableWeakerNetworkIsolation"),
        ] {
            if let Some(value) = v.get(key) {
                settings[output] = value.clone();
            }
        }
    }
    if settings["enableWeakerNetworkIsolation"] == true {
        warnings.push("weaker_network_isolation: 削弱网络隔离（com.apple.trustd.agent）".into());
    }
    for entry in [context.worktree.join(".fence.toml"), home.to_path_buf()] {
        protect_write(
            lists.get_mut("deny_write").unwrap(),
            &entry,
            &entry.to_string_lossy(),
        )?;
    }
    for entry in lists["write"].clone() {
        append(lists.get_mut("read").unwrap(), entry);
    }
    settings["filesystem"] = json!({"denyRead":lists["deny_read"],"allowRead":lists["read"],"allowWrite":lists["write"],"denyWrite":lists["deny_write"]});
    settings["network"] = network_settings(
        &lists["domains"],
        &lists["unix_sockets"],
        platform,
        &mut warnings,
    );
    let mkdir = lists["mkdir"]
        .iter()
        .filter(|s| !s.starts_with("<运行时才有>"))
        .map(PathBuf::from)
        .collect();
    Ok(Rules {
        mkdir,
        value: json!({"ok":true,"profile":profile,"kind":kind,"platform":platform,"cwd":context.cwd,"layers":layers.iter().map(|(_,m)| m).collect::<Vec<_>>(),"settings":settings,"env":environment,"args":lists["args"],"skipped":skipped,"warnings":warnings}),
    })
}

fn optional_file(path: &Path, role: &str) -> Result<Value> {
    match fs::read_to_string(path) {
        Ok(text) => {
            let v = data::parse(&text, &path.to_string_lossy())?;
            validate(&v, &path.to_string_lossy(), role, "")?;
            Ok(v)
        }
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound && fs::symlink_metadata(path).is_err() =>
        {
            Ok(json!({}))
        }
        Err(e) => Err(data::problem(&path.to_string_lossy(), "", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unix_socket_network_settings_ignore_paths_on_linux_with_a_warning() {
        let domains = vec!["example.com".into()];
        let paths = vec!["/private/tmp/fence-test".into()];
        let mut warnings = Vec::new();
        assert_eq!(
            network_settings(&domains, &paths, "linux", &mut warnings),
            json!({"allowedDomains":["example.com"],"deniedDomains":[]})
        );
        assert_eq!(
            warnings,
            ["unix_sockets: Linux 不能按路径放行 unix socket，已忽略"]
        );
        warnings.clear();
        assert_eq!(
            network_settings(&domains, &paths, "macos", &mut warnings),
            json!({"allowedDomains":["example.com"],"deniedDomains":[],"allowUnixSockets":["/private/tmp/fence-test"]})
        );
        assert!(warnings.is_empty());
        assert_eq!(
            network_settings(&domains, &[], "linux", &mut warnings),
            json!({"allowedDomains":["example.com"],"deniedDomains":[]})
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn platform_segments_are_selected_without_leaking_macos_options() {
        let data = json!({"write":["/base"],"macos":{"allow_pty":true,"weaker_network_isolation":true,"read":["/keychain"]},"linux":{"write":["/refs"]}});
        assert_eq!(
            select(&data, "macos"),
            json!({"write":["/base"],"read":["/keychain"],"allow_pty":true,"weaker_network_isolation":true})
        );
        assert_eq!(select(&data, "linux"), json!({"write":["/base","/refs"]}));
        for (_, _, text) in data::BUILTIN {
            let data = data::parse(text, "builtin").unwrap();
            let linux = select(&data, "linux");
            assert!(linux.get("allow_pty").is_none());
            assert!(linux.get("weaker_network_isolation").is_none());
        }
    }
}
