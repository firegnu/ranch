use pony::core::split::split;
#[test]
fn markdown_chunks_preserve_every_character_and_balance_fences() {
    let inputs = [
        "第一段。\n\n".repeat(70),
        format!(
            "开头\n\n```rust\n{}\n```\n结束\n",
            "let 中文 = 1;\n".repeat(40)
        ),
        format!("~~~json\n{}\n~~~\n", "汉🙂".repeat(200)),
        format!("````text\n{}\n````", "abc\n".repeat(50)),
        "字🙂".repeat(300),
    ];
    for input in inputs {
        let parts = split(&input, 100).unwrap();
        assert!(parts.len() > 1);
        assert_eq!(
            parts.iter().map(|p| p.body.as_str()).collect::<String>(),
            input
        );
        for (i, p) in parts.iter().enumerate() {
            assert!(
                p.render(i, parts.len()).chars().count() <= 100,
                "{}",
                p.render(i, parts.len())
            );
            assert!(
                p.render(i, parts.len())
                    .ends_with(&format!("({}/{})", i + 1, parts.len()))
            );
        }
        if input.contains("```rust") {
            assert!(parts.iter().skip(1).any(|p| p.opening == "```rust\n"));
            assert!(parts.iter().any(|p| p.closing.contains("```")));
        }
        if input.contains("~~~json") {
            assert!(parts.iter().skip(1).any(|p| p.opening == "~~~json\n"));
        }
    }
    assert_eq!(split("原文\n", 100).unwrap()[0].render(0, 1), "原文\n");
    assert_eq!(split("", 100).unwrap()[0].render(0, 1), "（空回复）");
}
#[test]
fn blank_lines_take_priority_and_a_fitting_table_stays_together() {
    let input = format!(
        "{}\n\n{}\n{}\n",
        "a".repeat(20),
        "b".repeat(30),
        "c".repeat(30)
    );
    let parts = split(&input, 75).unwrap();
    assert_eq!(parts[0].body, format!("{}\n\n", "a".repeat(20)));
    let table = "| a | b |\n|---|---|\n| 1 | 2 |\n";
    let input = format!("{}\n{table}结尾", "a".repeat(50));
    let parts = split(&input, 80).unwrap();
    assert!(parts.iter().any(|p| p.body.contains(table)));
}
