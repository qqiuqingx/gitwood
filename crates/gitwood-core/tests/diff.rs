mod support;
use gitwood_core::diff::*;
use std::path::Path;
use support::Repo;

#[test]
fn content_classification_preserves_bytes_and_reports_special_states() {
    let limits = ReadLimits::default();
    assert_eq!(
        classify(vec![], limits),
        Content::Text {
            text: "".into(),
            byte_len: 0,
            empty: true,
            missing_final_newline: false
        }
    );
    assert!(matches!(
        classify(b"last".to_vec(), limits),
        Content::Text {
            missing_final_newline: true,
            ..
        }
    ));
    assert!(matches!(
        classify(b"last\r\n".to_vec(), limits),
        Content::Text {
            missing_final_newline: false,
            byte_len: 6,
            ..
        }
    ));
    assert_eq!(
        classify(vec![0xff], limits),
        Content::Undecodable { byte_len: 1 }
    );
    let mut late_nul = vec![b'x'; 9000];
    late_nul.push(0);
    assert_eq!(
        classify(late_nul, limits),
        Content::Binary { byte_len: 9001 }
    );
    assert_eq!(
        classify(vec![0; 5], ReadLimits { max_bytes: 4 }),
        Content::TooLarge {
            byte_len: 5,
            limit: 4
        }
    );
}
fn range(start: usize, end: usize) -> LineRange {
    LineRange { start, end }
}
#[test]
fn insert_delete_modify_and_alignment_include_empty_anchors() {
    for (old, new, kind, l, r) in [
        (
            "a\nb\n",
            "a\nx\nb\n",
            ChangeKind::Insert,
            range(1, 1),
            range(1, 2),
        ),
        (
            "a\nx\nb\n",
            "a\nb\n",
            ChangeKind::Delete,
            range(1, 2),
            range(1, 1),
        ),
        (
            "a\nx\nb\n",
            "a\ny\nz\nb\n",
            ChangeKind::Modify,
            range(1, 2),
            range(1, 3),
        ),
        ("", "x", ChangeKind::Insert, range(0, 0), range(0, 1)),
        ("x", "", ChangeKind::Delete, range(0, 1), range(0, 0)),
    ] {
        let result = compare(old, new, CompareOptions::default());
        assert_eq!(result.blocks.len(), 1);
        assert_eq!(
            (
                result.blocks[0].kind,
                result.blocks[0].left,
                result.blocks[0].right
            ),
            (kind, l, r)
        );
        assert!(result
            .alignment
            .iter()
            .any(|span| span.left == l && span.right == r));
        let last = result.alignment.last().unwrap();
        assert_eq!(
            (last.left.end, last.right.end),
            (result.left_lines, result.right_lines)
        );
    }
}
#[test]
fn unicode_word_ranges_are_utf16_and_not_byte_offsets() {
    let result = compare(
        "🙂 old value\n",
        "🙂 new value\n",
        CompareOptions::default(),
    );
    let block = &result.blocks[0];
    assert_eq!(
        block.left_inline,
        [InlineRange {
            line: 0,
            start: 3,
            end: 6
        }]
    );
    assert_eq!(
        block.right_inline,
        [InlineRange {
            line: 0,
            start: 3,
            end: 6
        }]
    );
    let multiline = compare("old\nvalue\n", "new\nother\n", CompareOptions::default());
    assert!(multiline.blocks[0]
        .right_inline
        .iter()
        .any(|r| r.line == 1 && r.end == 5));
}
#[test]
fn folding_retains_four_lines_of_context_and_can_be_disabled_by_context() {
    let old: String = (0..30).map(|i| format!("line{i}\n")).collect();
    let new = old.replace("line15\n", "changed\n");
    let result = compare(&old, &new, CompareOptions::default());
    assert_eq!(
        result.folds,
        [
            FoldRange {
                left: range(0, 11),
                right: range(0, 11)
            },
            FoldRange {
                left: range(20, 30),
                right: range(20, 30)
            }
        ]
    );
    let result = compare(
        &old,
        &new,
        CompareOptions {
            context_lines: usize::MAX,
            ..CompareOptions::default()
        },
    );
    assert!(result.folds.is_empty());
    assert!(compare("a\n", "a\n", CompareOptions::default())
        .folds
        .is_empty());
}
#[test]
fn mixed_line_endings_and_missing_final_newline_are_real_differences() {
    let result = compare("a\r\nb\nc", "a\nb\r\nc\n", CompareOptions::default());
    assert_eq!((result.left_lines, result.right_lines), (3, 3));
    assert!(!result.blocks.is_empty());
    for block in &result.blocks {
        for inline in block.left_inline.iter().chain(&block.right_inline) {
            assert!(inline.end <= 1);
        }
    }
    assert_eq!(
        compare("a\n", "a", CompareOptions::default()).blocks[0].kind,
        ChangeKind::Modify
    );
}
#[tokio::test]
async fn reads_head_commit_index_worktree_and_checks_size_before_loading() {
    let repo = Repo::new().await;
    let name = "-中文 spaced.txt";
    repo.put(name, "head\r\n");
    repo.git("add", &["--", name]).await;
    repo.git("commit", &["-qm", "base"]).await;
    let oid = String::from_utf8(repo.git("rev-parse", &["HEAD"]).await).unwrap();
    repo.put(name, "index\n");
    repo.git("add", &["--", name]).await;
    repo.put(name, "worktree");
    for (source, expected) in [
        (
            ContentSource::Commit {
                revision: "HEAD",
                path: Path::new(name),
            },
            "head\r\n",
        ),
        (
            ContentSource::Commit {
                revision: oid.trim(),
                path: Path::new(name),
            },
            "head\r\n",
        ),
        (
            ContentSource::Index {
                path: Path::new(name),
            },
            "index\n",
        ),
        (
            ContentSource::Worktree {
                path: Path::new(name),
            },
            "worktree",
        ),
    ] {
        let actual = read_content(&repo.runner, &repo.options, source, ReadLimits::default())
            .await
            .unwrap();
        assert_eq!(
            actual,
            classify(expected.as_bytes().to_vec(), ReadLimits::default())
        );
        assert!(matches!(
            read_content(
                &repo.runner,
                &repo.options,
                source,
                ReadLimits { max_bytes: 3 }
            )
            .await
            .unwrap(),
            Content::TooLarge { limit: 3, .. }
        ));
    }
    assert!(read_content(
        &repo.runner,
        &repo.options,
        ContentSource::Worktree {
            path: Path::new("../outside")
        },
        ReadLimits::default()
    )
    .await
    .is_err());
    assert!(read_content(
        &repo.runner,
        &repo.options,
        ContentSource::Commit {
            revision: "--output=x",
            path: Path::new(name)
        },
        ReadLimits::default()
    )
    .await
    .is_err());
    // Sparse file: metadata returns TooLarge without allocating its contents.
    let file = std::fs::File::create(repo.path("huge")).unwrap();
    file.set_len(DEFAULT_MAX_BYTES + 1).unwrap();
    assert!(matches!(
        read_content(
            &repo.runner,
            &repo.options,
            ContentSource::Worktree {
                path: Path::new("huge")
            },
            ReadLimits::default()
        )
        .await
        .unwrap(),
        Content::TooLarge { .. }
    ));
}
#[cfg(unix)]
#[tokio::test]
async fn worktree_symlink_cannot_read_outside_the_root() {
    let repo = Repo::new().await;
    let outside = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), repo.path("link")).unwrap();
    assert!(matches!(
        read_content(
            &repo.runner,
            &repo.options,
            ContentSource::Worktree {
                path: Path::new("link")
            },
            ReadLimits::default()
        )
        .await,
        Err(DiffError::InvalidSource(_))
    ));
}
#[test]
fn measure_twenty_thousand_lines() {
    let old: String = (0..20_000)
        .map(|i| format!("let value_{i} = {i};\n"))
        .collect();
    let new = old.replace(" = 15000;", " = 42;");
    let start = std::time::Instant::now();
    let result = compare(&old, &new, CompareOptions::default());
    eprintln!(
        "diff 20000 lines, {} / {} bytes, {:?}, {} block(s)",
        old.len(),
        new.len(),
        start.elapsed(),
        result.blocks.len()
    );
    assert_eq!(result.blocks.len(), 1);
}

#[test]
fn serde_contract_uses_the_same_tags_and_camel_case_as_typescript() {
    let content = classify(b"x".to_vec(), ReadLimits::default());
    let value = serde_json::to_value(&content).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"kind":"text","text":"x","byteLen":1,"empty":false,"missingFinalNewline":true})
    );
    let result = compare("old\r\n", "new\n", CompareOptions::default());
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["blocks"][0]["kind"], "modify");
    assert_eq!(
        value["blocks"][0]["leftInline"][0],
        serde_json::json!({"line":0,"start":0,"end":3})
    );
    assert_eq!(value["leftLines"], 1);
    assert_eq!(serde_json::from_value::<DiffResult>(value).unwrap(), result);
}
