use super::{invalid, load, local_bytes, local_path, unknown_cell, version};
use serde_json::{Value, json};
use std::{fs, io::Write};
use toml_edit::{DocumentMut, Item, Table, value};

/// Only cell fields are writable; agent launch templates remain read-only.
#[derive(Default)]
pub struct CellPatch {
    pub agents: Option<Vec<String>>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub cross_review: Option<String>,
    pub rework: Option<String>,
    pub description: Option<String>,
}

/// `None` removes the local cell override, restoring the built-in definition.
pub fn edit(
    name: &str,
    patch: Option<CellPatch>,
    expected_version: Option<&str>,
) -> Result<Value, (u8, Value)> {
    let path = local_path()?;
    let bytes = local_bytes(&path).map_err(|e| invalid(&path, e))?;
    check_version(expected_version, bytes.as_deref())?;
    let text = bytes
        .as_deref()
        .map(std::str::from_utf8)
        .transpose()
        .map_err(|e| invalid(&path, e))?;
    let before = load(&path, text).map_err(|e| invalid(&path, e))?;
    if before["cells"].get(name).is_none() {
        return Err(unknown_cell(name));
    }
    let mut doc: DocumentMut = text.unwrap_or("").parse().map_err(|e| invalid(&path, e))?;
    if doc.to_string() != text.unwrap_or("") {
        return Err((
            1,
            json!({"ok": false, "error": "format_not_preserved",
                   "message": format!("{}: TOML formatting (for example interleaved dotted keys) cannot be preserved; edit manually or use ordinary [cells.NAME] tables", path.display())}),
        ));
    }
    if let Some(patch) = patch {
        apply_patch(&mut doc, name, patch);
    } else if let Some(cells) = doc.get_mut("cells") {
        cells
            .as_table_like_mut()
            .expect("validated cells table")
            .remove(name);
    }
    let candidate = doc.to_string();
    let after = load(&path, Some(&candidate)).map_err(|e| invalid(&path, e))?;
    if candidate == text.unwrap_or("") {
        return Ok(before);
    }
    let prepare = || -> std::io::Result<tempfile::NamedTempFile> {
        let parent = path.parent().expect("dispatch parent");
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        if text.is_some() {
            temporary
                .as_file()
                .set_permissions(fs::metadata(&path)?.permissions())?;
        }
        temporary.write_all(candidate.as_bytes())?;
        temporary.as_file().sync_all()?;
        Ok(temporary)
    };
    let io_error = |e: std::io::Error| {
        (
            1,
            json!({"ok": false, "error": "io_error", "message": format!("{}: {e}", path.display())}),
        )
    };
    let temporary = prepare().map_err(io_error)?;
    if expected_version.is_some() {
        let current = local_bytes(&path).map_err(|e| invalid(&path, e))?;
        check_version(expected_version, current.as_deref())?;
    }
    temporary.persist(&path).map_err(|e| io_error(e.error))?;
    Ok(after)
}

fn check_version(expected: Option<&str>, bytes: Option<&[u8]>) -> Result<(), (u8, Value)> {
    if let Some(expected) = expected {
        let actual = version(bytes);
        if expected != actual {
            return Err((
                3,
                json!({"ok": false, "error": "conflict",
                       "message": "dispatch.toml changed; read the table again",
                       "expected_version": expected, "actual_version": actual}),
            ));
        }
    }
    Ok(())
}

fn apply_patch(doc: &mut DocumentMut, name: &str, patch: CellPatch) {
    let cells = doc.entry("cells").or_insert_with(|| {
        let mut table = Table::new();
        table.set_implicit(true);
        Item::Table(table)
    });
    let cell = cells
        .as_table_like_mut()
        .expect("validated cells table")
        .entry(name)
        .or_insert(Item::Table(Table::new()))
        .as_table_like_mut()
        .expect("validated cell table");
    let mut fields = Vec::new();
    if let Some(agents) = patch.agents {
        fields.push((
            "agents",
            value(agents.into_iter().collect::<toml_edit::Array>()),
        ));
    }
    for (key, new) in [
        ("model", patch.model),
        ("effort", patch.effort),
        ("cross_review", patch.cross_review),
        ("rework", patch.rework),
        ("description", patch.description),
    ] {
        if let Some(new) = new {
            fields.push((key, value(new)));
        }
    }
    for (key, mut new) in fields {
        if let Some(old) = cell.get_mut(key) {
            *new.as_value_mut().expect("new value").decor_mut() =
                old.as_value().expect("validated value").decor().clone();
            *old = new;
        } else {
            cell.insert(key, new);
        }
    }
}
