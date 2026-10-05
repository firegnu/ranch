use super::{json::Json, rules};
use saddle_core_plugin::{Begin, Call, Captured, Completion, End, Missing, RouteBegin, RouteEnd};

pub(super) struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}
pub(super) struct Failure {
    pub message: &'static str,
    pub retry: bool,
    pub missing: Missing,
}

pub(super) const MAX_RESPONSE: usize = 16 * 1024 * 1024;

pub(super) fn run(
    call: Call<'_>,
    key: Option<&str>,
    mut post: impl FnMut(&[u8], &str, bool) -> Result<Response, Failure>,
    mut sleep: impl FnMut(std::time::Duration),
) -> Completion {
    let Some(key) = key.filter(|v| !v.is_empty()) else {
        return failure("TYPESAFE_API_KEY is not set", None);
    };
    let mut input = String::new();
    if call.stdin.read_to_string(&mut input).is_err() {
        return failure("stdin must be readable UTF-8", None);
    }
    // Python str.isspace includes these four controls in addition to Unicode whitespace.
    let summary =
        input.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c));
    let request = rules::request(summary);
    call.recorder.begin(Begin::Route(RouteBegin {
        router_model: rules::MODEL.into(),
        router_version: Some(rules::VERSION.into()),
        rules_version: Some(rules::fingerprint()),
        summary: summary.as_bytes().to_vec(),
        request: request.clone(),
    }));
    let mut attempt = 0;
    let body = loop {
        let first = attempt == 0;
        attempt += 1;
        match post(&request, key, first) {
            Ok(reply) if first && matches!(reply.status, 429 | 529) => {
                sleep(std::time::Duration::from_secs(1))
            }
            Ok(reply) if !(200..300).contains(&reply.status) => {
                let prefix = &reply.body[..reply.body.len().min(300)];
                return failure(
                    &format!("HTTP {}: {}", reply.status, String::from_utf8_lossy(prefix)),
                    Some(Captured::Missing(Missing::NotAvailable)),
                );
            }
            Ok(reply) => break reply.body,
            Err(error) if first && error.retry => (),
            Err(error) => return failure(error.message, Some(Captured::Missing(error.missing))),
        }
    };
    if body.len() > MAX_RESPONSE {
        return failure(
            "response exceeds 16 MiB",
            Some(Captured::Missing(Missing::TooLarge)),
        );
    }
    let parsed: Json = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => {
            return failure(
                "invalid response JSON",
                Some(Captured::Missing(Missing::Unrecognized)),
            );
        }
    };
    let response = serde_json::to_vec(&parsed).expect("parsed JSON");
    if response.len() > MAX_RESPONSE {
        return failure(
            "parsed response exceeds 16 MiB",
            Some(Captured::Missing(Missing::TooLarge)),
        );
    }
    let captured = Captured::Bytes(response);
    match shape(&parsed) {
        Ok(value) => complete(0, value, Some(captured)),
        Err(error) => failure(&format!("invalid response: {error}"), Some(captured)),
    }
}

fn failure(error: &str, response: Option<Captured>) -> Completion {
    complete(
        1,
        Json::object([("ok", Json::Bool(false)), ("error", error.into())]),
        response,
    )
}
fn complete(exit_code: u8, value: Json, response: Option<Captured>) -> Completion {
    let mut stdout = serde_json::to_vec(&value).expect("business JSON");
    stdout.push(b'\n');
    let end = response.map(|response| {
        End::Route(RouteEnd {
            response,
            suggestion: Captured::Bytes(stdout.clone()),
        })
    });
    Completion {
        exit_code,
        stdout,
        end,
    }
}
// Decimal formatting rounds the binary float, ties to even, before converting back.
fn rounded(value: f64) -> f64 {
    format!("{value:.3}")
        .parse()
        .expect("finite formatted number")
}
fn shape(resp: &Json) -> Result<Json, &'static str> {
    let answers = resp.field("answers")?;
    let tier = answers.field("tier")?;
    let mut probs = vec![];
    let mut maximum = None;
    for (key, value) in tier.field("probabilities")?.entries()? {
        let index = match key.as_str() {
            "0" => 0,
            "1" => 1,
            "2" => 2,
            _ => return Err("invalid probability key"),
        };
        let value = rounded(value.number()?);
        let name = rules::TIERS[index];
        if maximum.is_none_or(|(_, best)| value > best) {
            maximum = Some((name, value));
        }
        probs.push((name, Json::from(value)));
    }
    let level = maximum.ok_or("empty probabilities")?.0;
    let confidence = tier.field("confidence")?.number()?;
    let tier = Json::object([
        (
            "verdict",
            if confidence >= rules::CONFIDENT {
                level.into()
            } else {
                Json::Null
            },
        ),
        ("level", level.into()),
        ("score", rounded(tier.field("score")?.number()?).into()),
        ("probabilities", Json::object(probs)),
        ("confidence", rounded(confidence).into()),
    ]);
    let mut cross = vec![];
    let mut top: Option<f64> = None;
    for (key, value) in answers.entries()? {
        if let Some(key) = key.strip_prefix("cross_") {
            let value = rounded(value.field("noul")?.number()?);
            top = Some(top.map_or(value, |best| best.max(value)));
            cross.push((key, Json::from(value)));
        }
    }
    let top = top.ok_or("no cross answers")?;
    let verdict = if top >= rules::CROSS_YES {
        "要".into()
    } else if top <= rules::CROSS_NO {
        "不要".into()
    } else {
        Json::Null
    };
    // Python {verdict: ..., **cross} also lets a cross_verdict answer replace that value.
    let cross = Json::object(std::iter::once(("verdict", verdict)).chain(cross));
    let cv = cross.field("verdict")?;
    let yes = matches!(cv,Json::String(v) if v=="要");
    let no = matches!(cv,Json::String(v) if v=="不要");
    let visible = rounded(
        answers
            .field("visible")?
            .field("noul")?
            .number()?
            .max(answers.field("doc_only")?.field("noul")?.number()?),
    );
    let impact = if yes {
        "碰要害".into()
    } else if visible >= rules::VISIBLE_YES {
        "看得见".into()
    } else if visible <= rules::VISIBLE_NO && no {
        "改行为".into()
    } else {
        Json::Null
    };
    Ok(Json::object([
        ("ok", Json::Bool(true)),
        ("model", resp.field("model").cloned().unwrap_or(Json::Null)),
        ("tier", tier),
        ("cross_review", cross),
        (
            "impact",
            Json::object([("verdict", impact), ("visible", visible.into())]),
        ),
        ("usage", resp.field("usage").cloned().unwrap_or(Json::Null)),
    ]))
}
