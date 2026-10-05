// From Saddle `plugins/dispatch/src/route_tests.rs` at commit `c21674a`, telemetry capture removed.
use super::{route::*, rules};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    const RESPONSE: &str = r#"{ "extra": {"z":1,"a":2,"z":3}, "answers": {
        "tier":{"probabilities":{"2":0.7996,"0":0.8004,"2":0.8001,"1":0.0625},"confidence":0.7999,"score":0.1875},
        "cross_security_privacy":{"noul":0.7996},"cross_data_model":{"noul":0.0005},
        "visible":{"noul":0.2675},"doc_only":{"noul":0.1}},
        "model":{"id":"synthetic"},"usage":{"tokens":18446744073709551615}}
    "#;

    #[test]
    fn request_constants_and_python_order_rounding_are_preserved() {
        let mut sent = vec![];
        let result = run(
            &mut "\u{1c}\u{85}\u{2003}合成任务\u{1f}\r\n".as_bytes(),
            Some("synthetic-key-03c"),
            |body, key, _| {
                assert_eq!(key, "synthetic-key-03c");
                sent.push(body.to_vec());
                Ok(Response {
                    status: 200,
                    body: RESPONSE.as_bytes().to_vec(),
                })
            },
            |_| panic!("no sleep on success"),
        );
        assert_eq!(
            result.exit_code,
            0,
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        assert_eq!(sent.len(), 1);
        assert_eq!(
            serde_json::from_slice::<Value>(&sent[0]).unwrap(),
            serde_json::from_str::<Value>(include_str!("../tests/fixtures/request.json")).unwrap()
        );
        let request = String::from_utf8(sent[0].clone()).unwrap();
        assert!(request.starts_with("{\"state\":{\"task_summary\":\"合成任务\"},\"model\":\"jev-1.13.0\",\"questions\":{\"tier\":"));
        assert!(
            request.find("cross_data_model").unwrap()
                < request.find("cross_security_privacy").unwrap()
        );
        let value: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            value,
            json!({"ok":true,"model":{"id":"synthetic"},"tier":{"verdict":null,"level":"重","score":0.188,"probabilities":{"重":0.8,"轻":0.8,"常规":0.062},"confidence":0.8},"cross_review":{"verdict":"要","security_privacy":0.8,"data_model":0.001},"impact":{"verdict":"碰要害","visible":0.268},"usage":{"tokens":18446744073709551615u64}})
        );
        let stdout = String::from_utf8(result.stdout.clone()).unwrap();
        assert!(stdout.find("security_privacy").unwrap() < stdout.find("data_model").unwrap());
        assert!(!stdout.contains("synthetic-key-03c"));
    }
}

#[cfg(test)]
mod retry_tests {
    use super::*;
    #[test]
    fn retryable_statuses_retry_once_and_use_only_the_final_response() {
        for status in [429, 529] {
            let mut n = 0;
            let mut sleeps = vec![];
            let result = run(
                &mut &b"test"[..],
                Some("synthetic"),
                |_, _, _| {
                    n += 1;
                    Ok(Response {
                        status: if n == 1 { status } else { 200 },
                        body: if n == 1 {
                            b"previous body must not survive".to_vec()
                        } else {
                            b"{ \"extra\": 3 }".to_vec()
                        },
                    })
                },
                |d| sleeps.push(d),
            );
            assert_eq!(n, 2, "retry status {status}");
            assert_eq!(sleeps, vec![std::time::Duration::from_secs(1)]);
            // The final body is valid JSON with the wrong shape; the first one is not JSON at all.
            assert_eq!(result.exit_code, 1);
            let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
            assert!(
                value["error"]
                    .as_str()
                    .unwrap()
                    .starts_with("invalid response: ")
            );
        }
    }
}

#[cfg(test)]
mod boundaries {
    use super::*;
    use serde_json::{Value, json};
    fn call(body: &[u8]) -> Completion {
        run(
            &mut &b"synthetic"[..],
            Some("synthetic"),
            |_, _, _| {
                Ok(Response {
                    status: 200,
                    body: body.to_vec(),
                })
            },
            |_| panic!("no sleep"),
        )
    }
    fn response() -> Value {
        json!({"answers":{"tier":{"probabilities":{"0":1},"score":0,"confidence":0.8},"cross_x":{"noul":0},"visible":{"noul":0},"doc_only":{"noul":0}}})
    }
    #[test]
    fn rounding_and_thresholds_use_the_documented_binary_float_values() {
        // Independent Python 3 round(x,3) values, including exact binary ties.
        for (input, expected) in [
            (0.0625, 0.062),
            (0.1875, 0.188),
            (0.3125, 0.312),
            (0.4375, 0.438),
            (0.5625, 0.562),
            (0.6875, 0.688),
            (0.8125, 0.812),
            (0.9375, 0.938),
            (0.0005, 0.001),
            (0.2675, 0.268),
        ] {
            let mut value = response();
            value["answers"]["tier"]["score"] = json!(input);
            let result = call(&serde_json::to_vec(&value).unwrap());
            assert_eq!(result.exit_code, 0);
            let v: Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(v["tier"]["score"], expected);
        }
        for (cross, visible, doc, confidence, cv, impact, tier) in [
            (
                0.2004,
                0.2004,
                0.,
                0.8,
                json!("不要"),
                json!("改行为"),
                json!("轻"),
            ),
            (
                0.2006,
                0.,
                0.,
                0.7999,
                Value::Null,
                Value::Null,
                Value::Null,
            ),
            (
                0.7994,
                0.,
                0.7996,
                0.8,
                Value::Null,
                json!("看得见"),
                json!("轻"),
            ),
            (
                0.7996,
                1.,
                0.,
                0.8,
                json!("要"),
                json!("碰要害"),
                json!("轻"),
            ),
            (0., 0.2006, 0., 0.8, json!("不要"), Value::Null, json!("轻")),
        ] {
            let mut v = response();
            v["answers"]["cross_x"]["noul"] = json!(cross);
            v["answers"]["visible"]["noul"] = json!(visible);
            v["answers"]["doc_only"]["noul"] = json!(doc);
            v["answers"]["tier"]["confidence"] = json!(confidence);
            let result = call(&serde_json::to_vec(&v).unwrap());
            assert_eq!(result.exit_code, 0);
            let value: Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(value["cross_review"]["verdict"], cv);
            assert_eq!(value["impact"]["verdict"], impact);
            assert_eq!(value["tier"]["verdict"], tier);
            assert!(value["model"].is_null());
            assert!(value["usage"].is_null());
        }
    }
    #[test]
    fn invalid_shapes_fail_with_the_shape_error() {
        for body in [
            b"null".as_slice(),
            b"{\"unused\":3}",
            br#"{"answers":{"tier":{"probabilities":{"01":1},"score":0,"confidence":1}}}"#,
            br#"{"answers":{"tier":{"probabilities":{"0":true},"score":0,"confidence":1}}}"#,
        ] {
            let result = call(body);
            assert_eq!(result.exit_code, 1);
            let value: Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(value["ok"], false);
            assert!(
                value["error"]
                    .as_str()
                    .unwrap()
                    .starts_with("invalid response: ")
            );
        }
    }
    #[test]
    fn missing_empty_key_and_invalid_utf8_never_request() {
        for (key, bytes) in [
            (None, b"synthetic".as_slice()),
            (Some(""), b"synthetic"),
            (Some("synthetic"), b"\xff"),
        ] {
            let result = run(
                &mut &bytes[..],
                key,
                |_, _, _| panic!("no request"),
                |_| panic!("no sleep"),
            );
            assert_eq!(result.exit_code, 1);
        }
    }
    #[test]
    fn network_retries_stop_after_two_attempts_without_sleeping() {
        for retry in [true, false] {
            let mut n = 0;
            let result = run(
                &mut &b""[..],
                Some("synthetic"),
                |_, _, _| {
                    n += 1;
                    Err(Failure {
                        message: "network: synthetic",
                        retry,
                    })
                },
                |_| panic!("no delay for transport errors"),
            );
            assert_eq!(n, if retry { 2 } else { 1 });
            assert_eq!(result.exit_code, 1);
        }
    }
    #[test]
    fn rules_fingerprint_is_pinned_to_original_constants() {
        assert_eq!(
            rules::fingerprint(),
            "sha256:8d4ec37d101d7d6c69d48df828d488b91b68fe751a80c783b6f34911484388b0"
        );
    }
}
