use project::search::SearchQuery;
use text::Rope;
use util::{
    paths::{PathMatcher, PathStyle},
    rel_path::RelPath,
};

#[test]
fn path_matcher_creation_for_valid_paths() {
    for valid_path in [
        "file",
        "Cargo.toml",
        ".DS_Store",
        "~/dir/another_dir/",
        "./dir/file",
        "dir/[a-z].txt",
    ] {
        let path_matcher = PathMatcher::new(&[valid_path.to_owned()], PathStyle::local())
            .unwrap_or_else(|e| panic!("Valid path {valid_path} should be accepted, but got: {e}"));
        assert!(
            path_matcher.is_match(&RelPath::new(valid_path.as_ref(), PathStyle::local()).unwrap()),
            "Path matcher for valid path {valid_path} should match itself"
        )
    }
}

#[test]
fn path_matcher_creation_for_globs() {
    for invalid_glob in ["dir/[].txt", "dir/[a-z.txt", "dir/{file"] {
        match PathMatcher::new(&[invalid_glob.to_owned()], PathStyle::local()) {
            Ok(_) => panic!("Invalid glob {invalid_glob} should not be accepted"),
            Err(_expected) => {}
        }
    }

    for valid_glob in [
        "dir/?ile",
        "dir/*.txt",
        "dir/**/file",
        "dir/[a-z].txt",
        "{dir,file}",
    ] {
        match PathMatcher::new(&[valid_glob.to_owned()], PathStyle::local()) {
            Ok(_expected) => {}
            Err(e) => panic!("Valid glob should be accepted, but got: {e}"),
        }
    }
}

#[test]
fn test_case_sensitive_pattern_items() {
    let case_sensitive = false;
    let search_query = SearchQuery::regex(
        "test\\C",
        false,
        case_sensitive,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .expect("Should be able to create a regex SearchQuery");

    assert_eq!(
        search_query.case_sensitive(),
        true,
        "Case sensitivity should be enabled when \\C pattern item is present in the query."
    );

    let case_sensitive = true;
    let search_query = SearchQuery::regex(
        "test\\c",
        true,
        case_sensitive,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .expect("Should be able to create a regex SearchQuery");

    assert_eq!(
        search_query.case_sensitive(),
        false,
        "Case sensitivity should be disabled when \\c pattern item is present, even if initially set to true."
    );

    let case_sensitive = false;
    let search_query = SearchQuery::regex(
        "test\\c\\C",
        false,
        case_sensitive,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .expect("Should be able to create a regex SearchQuery");

    assert_eq!(
        search_query.case_sensitive(),
        true,
        "Case sensitivity should be enabled when \\C is the last pattern item, even after a \\c."
    );

    let case_sensitive = false;
    let search_query = SearchQuery::regex(
        "tests\\\\C",
        false,
        case_sensitive,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .expect("Should be able to create a regex SearchQuery");

    assert_eq!(
        search_query.case_sensitive(),
        false,
        "Case sensitivity should not be enabled when \\C pattern item is preceded by a backslash."
    );
}

#[gpui::test]
async fn test_multiline_regex(cx: &mut gpui::TestAppContext) {
    let search_query = SearchQuery::regex(
        "^hello$\n",
        false,
        false,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .expect("Should be able to create a regex SearchQuery");

    use language::Buffer;
    let text = Rope::from("hello\nworld\nhello\nworld");
    let snapshot = cx
        .update(|app| Buffer::build_snapshot(text, None, None, app))
        .await;

    let results = search_query.search(&snapshot, None).await;
    assert_eq!(results, vec![0..6, 12..18]);
}

fn regex_query_with_replacement(pattern: &str, replacement: &str) -> SearchQuery {
    SearchQuery::regex(
        pattern,
        false,
        true,
        false,
        false,
        Default::default(),
        Default::default(),
        false,
        None,
    )
    .unwrap()
    .with_replacement(replacement.to_string())
}

#[test]
fn test_lookahead_regex_replacement() {
    // Issue #25905: (\d)(?=(\d{4})+$) with replacement $1,
    // should split "316227766016837933199" into "3,1622,7766,0168,3793,3199"
    let query = regex_query_with_replacement(r"(\d)(?=(\d{4})+$)", "$1,");
    assert!(query.is_regex());

    // replacement_for is called with just the matched text (a single digit).
    // The lookahead context from the original string is lost, so the regex
    // won't re-match inside replacement_for — this demonstrates the bug.
    let result = query.replacement_for("316227766016837933199");
    assert_eq!(
        result.as_deref(),
        Some("3,16227766016837933199"),
        "Lookahead replacement should produce '$1,' substitution for matched digit"
    );
}

#[gpui::test]
async fn test_lookahead_regex_search_finds_matches(cx: &mut gpui::TestAppContext) {
    // Verify the lookahead pattern finds the correct match positions
    let query = regex_query_with_replacement(r"(\d)(?=(\d{4})+$)", "$1,");

    use language::Buffer;
    let text = Rope::from("316227766016837933199");
    let snapshot = cx
        .update(|app| Buffer::build_snapshot(text, None, None, app))
        .await;

    let results = query.search(&snapshot, None).await;
    // 21-char string: matches at positions where remaining digits are a multiple of 4
    // pos 0 (20 remaining), 4 (16), 8 (12), 12 (8), 16 (4)
    assert_eq!(
        results,
        vec![0..1, 4..5, 8..9, 12..13, 16..17],
        "Lookahead regex should find digits at positions 0, 4, 8, 12, 16"
    );
}

#[test]
fn test_capture_group_replacement() {
    // (\w+)EventPayload with replacement $1Schema
    // "FooEventPayload" should become "FooSchema", not empty string
    let query = regex_query_with_replacement(r"(\w+)EventPayload", "$1Schema");

    let result = query.replacement_for("FooEventPayload");
    assert_eq!(
        result.as_deref(),
        Some("FooSchema"),
        "Capture group $1 should be substituted in replacement"
    );
}

#[test]
fn test_capture_group_replacement_various_inputs() {
    let query = regex_query_with_replacement(r"(\w+)EventPayload", "$1Schema");

    let result = query.replacement_for("ClickEventPayload");
    assert_eq!(result.as_deref(), Some("ClickSchema"));

    let result = query.replacement_for("MouseMoveEventPayload");
    assert_eq!(result.as_deref(), Some("MouseMoveSchema"));
}
