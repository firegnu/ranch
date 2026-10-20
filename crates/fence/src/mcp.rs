use crate::{Result, data, rules::Context};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

fn domain(value: &Value, source: &str) -> Result<String> {
    let url = value["url"].as_str().unwrap_or("");
    let uri = url.parse::<http::Uri>().ok();
    let uri = uri
        .as_ref()
        .filter(|uri| {
            matches!(uri.scheme_str(), Some("http" | "https"))
                && uri.authority().is_some_and(|a| {
                    !a.as_str().contains(['@', '*']) && crate::rules::valid_domain(a.as_str())
                })
                && !url.contains(['#', '$'])
        })
        .ok_or_else(|| {
            data::problem(
                source,
                "url",
                "expected an absolute HTTP(S) URL without credentials or fragments",
            )
        })?;
    let host = uri.host().unwrap();
    let port = uri.port_u16().filter(|port| {
        !matches!(
            (uri.scheme_str(), port),
            (Some("https"), 443) | (Some("http"), 80)
        )
    });
    Ok(match port {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    })
}

pub fn validate_server(value: &Value, source: &str) -> Result<()> {
    let table = value
        .as_object()
        .ok_or_else(|| data::problem(source, "", "expected table"))?;
    for (key, value) in table {
        let valid = match key.as_str() {
            "url" | "description" => value.is_string(),
            "auth" => value.as_object().is_some_and(|auth| {
                auth.keys()
                    .all(|k| matches!(k.as_str(), "header" | "prefix"))
                    && auth
                        .get("header")
                        .and_then(Value::as_str)
                        .is_some_and(|s| s.parse::<http::header::HeaderName>().is_ok())
                    && auth.get("prefix").is_none_or(Value::is_string)
            }),
            _ => false,
        };
        if !valid {
            return Err(data::problem(source, key, "invalid MCP server field"));
        }
    }
    domain(value, source)?;
    Ok(())
}

pub fn validate_template(value: &Value, source: &str) -> Result<()> {
    let table = value
        .as_object()
        .ok_or_else(|| data::problem(source, "mcp", "expected table"))?;
    for (key, value) in table {
        let valid = match key.as_str() {
            "headers_via_env" => value.is_boolean(),
            "args" => value
                .as_array()
                .is_some_and(|a| a.iter().all(Value::is_string)),
            "files" => value.as_object().is_some_and(|files| {
                files.iter().all(|(path, content)| {
                    !path.is_empty()
                        && Path::new(path)
                            .components()
                            .all(|c| matches!(c, Component::Normal(_)))
                        && !path.contains(['$', '\\'])
                        && content.is_string()
                })
            }),
            _ => false,
        };
        if !valid {
            return Err(data::problem(
                source,
                &format!("mcp.{key}"),
                "invalid MCP template field",
            ));
        }
    }
    Ok(())
}

pub struct Resolved {
    pub args: Vec<String>,
    pub files: Vec<(PathBuf, String)>,
    pub domains: Vec<String>,
    pub layers: Vec<Value>,
    pub key_paths: Vec<PathBuf>,
    pub secret_env: BTreeMap<String, String>,
}

pub fn resolve(
    home: &Path,
    selected: &Value,
    template: Option<&Value>,
    context: &Context,
    running: bool,
) -> Result<Resolved> {
    let mut result = Resolved {
        args: Vec::new(),
        files: Vec::new(),
        domains: Vec::new(),
        layers: Vec::new(),
        key_paths: vec![home.join("keys")],
        secret_env: BTreeMap::new(),
    };
    // Protect all sources independently of which servers will receive credentials.
    match fs::read_dir(home.join("keys")) {
        Ok(entries) => {
            let mut paths = entries
                .map(|entry| entry.map(|e| e.path()))
                .collect::<std::io::Result<Vec<_>>>()
                .map_err(|e| data::problem("mcp keys", "keys", e))?;
            paths.sort();
            result.key_paths.extend(paths);
        }
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                && fs::symlink_metadata(home.join("keys")).is_err() => {}
        Err(e) => return Err(data::problem("mcp keys", "keys", e)),
    }
    let mut seen = Vec::new();
    for name in selected.as_array().unwrap() {
        let name = name.as_str().unwrap();
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err(data::problem(
                "mcp",
                name,
                "expected letters, digits, '_' or '-'",
            ));
        }
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let (server, meta) = data::load(home, "mcp", name)?;
        let source = meta.to_string();
        result.domains.push(domain(&server, &source)?);
        result.layers.push(meta);
        let key_path = home.join("keys").join(name);
        result.key_paths.push(key_path.clone());
        let Some(template) = template else { continue };
        let mut headers = json!({});
        if running {
            match fs::read_to_string(&key_path) {
                Ok(key) => {
                    let key = key.strip_suffix('\n').unwrap_or(&key);
                    let key = key.strip_suffix('\r').unwrap_or(key);
                    let header = server["auth"]["header"].as_str().ok_or_else(|| {
                        data::problem(
                            &source,
                            "auth",
                            "key exists but auth.header is not configured",
                        )
                    })?;
                    let value = format!("{}{key}", server["auth"]["prefix"].as_str().unwrap_or(""));
                    // Clients also expand $VAR and $$, not just ${VAR}.
                    if key.is_empty()
                        || !value.is_ascii()
                        || value.starts_with('!')
                        || value.contains('$')
                        || value.parse::<http::HeaderValue>().is_err()
                    {
                        return Err(data::problem(
                            &source,
                            "auth",
                            "key must be a nonempty literal HTTP header value, without client interpolation",
                        ));
                    }
                    let header = if header.eq_ignore_ascii_case("authorization") {
                        "Authorization"
                    } else {
                        header
                    };
                    headers[header] = json!(value);
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::NotFound
                        && fs::symlink_metadata(&key_path).is_err() => {}
                Err(e) => return Err(data::problem(&key_path.to_string_lossy(), "key", e)),
            }
        }
        let mut env_headers = json!({});
        if template["headers_via_env"] == true {
            for (index, (header, value)) in headers.as_object().unwrap().iter().enumerate() {
                let variable = format!(
                    "FENCE_MCP_{}_{}_{index}",
                    context.values["run_nonce"].as_ref().unwrap(),
                    seen.len()
                );
                env_headers[header] = json!(variable);
                result
                    .secret_env
                    .insert(variable, value.as_str().unwrap().to_owned());
            }
        }
        let mut expanded = context.clone();
        let dir = format!("{}/mcp/{name}", context.values["tmp"].as_ref().unwrap());
        for (key, value) in [
            ("mcp_name", name.to_owned()),
            ("mcp_url", server["url"].as_str().unwrap().to_owned()),
            ("mcp_dir", dir.clone()),
            ("mcp_headers_json", headers.to_string()),
            ("mcp_headers_toml", inline_headers(&headers)),
            ("mcp_env_headers_toml", inline_headers(&env_headers)),
        ] {
            expanded.values.insert(key.into(), Some(value));
        }
        if let Some(args) = template["args"].as_array() {
            for arg in args {
                result.args.push(
                    expanded
                        .expand(arg.as_str().unwrap(), &source, "mcp.args", false)?
                        .unwrap(),
                );
            }
        }
        if let Some(files) = template["files"].as_object() {
            for (path, content) in files {
                result.files.push((
                    Path::new(&dir).join(path),
                    expanded
                        .expand(content.as_str().unwrap(), &source, "mcp.files", false)?
                        .unwrap(),
                ));
            }
        }
    }
    Ok(result)
}

fn inline_headers(headers: &Value) -> String {
    format!(
        "{{{}}}",
        headers
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| format!("{}={value}", json!(key)))
            .collect::<Vec<_>>()
            .join(",")
    )
}
