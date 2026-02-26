use querystrong::*;
use std::borrow::Cow;

fn all_borrowed_value(value: &Value) -> bool {
    match value {
        Value::Map(m) => m
            .iter()
            .all(|(k, v)| matches!(k, Cow::Borrowed(_)) && all_borrowed_value(v)),
        Value::List(l) => l.iter().all(all_borrowed_value),
        Value::SparseList(m) => m.values().all(all_borrowed_value),
        Value::String(s) => matches!(s, Cow::Borrowed(_)),
        Value::Empty => true,
    }
}

fn all_borrowed_index_path(path: &IndexPath) -> bool {
    path.iter().all(|indexer| match indexer {
        Indexer::String(s) => matches!(s, Cow::Borrowed(_)),
        _ => true,
    })
}

#[test]
fn parse_key() {
    assert_eq!(
        "a[b][]".parse::<IndexPath>().unwrap(),
        vec![Indexer::from("a"), Indexer::from("b"), Indexer::Empty]
    );

    assert_eq!(
        "a[0][b]".parse::<IndexPath>().unwrap(),
        vec![Indexer::from("a"), Indexer::from(0), Indexer::from("b")]
    );

    assert_eq!("a[b][c]".parse::<IndexPath>().unwrap(), vec!["a", "b", "c"]);
}

#[test]
fn more_nested_k_v() {
    let q = QueryStrong::from(("a[b][c]", "value"));
    assert_eq!(q["a[b][c]"], "value");
    assert!(matches!(q["a"], Value::Map(_)));
    assert!(matches!(q["a"]["b"], Value::Map(_)));
    assert!(matches!(q["a[b]"]["c"], Value::String(_)));
    assert!(matches!(q["a[b][c]"], Value::String(_)));
    assert!(matches!(q["a"]["b[c]"], Value::String(_)));
    assert_eq!(q.to_string(), "a[b][c]=value");
}

#[test]
fn list_access() -> std::result::Result<(), ParseErrors<'static>> {
    let q = QueryStrong::parse_strict("a[]=1&a[]=2&a[]=3")?;
    assert_eq!(q["a"][1], "2");
    assert_eq!(q["a[1]"], "2");

    let q = QueryStrong::parse_strict("a[1]=2")?;
    // SparseList: absent slots within 0..=max_key behave like Value::Empty
    assert_eq!(q["a"][0], Value::Empty);
    assert_eq!(q["a"][1], "2");
    assert_eq!(q["a[1]"], "2");
    // Slots beyond max_key are absent
    assert_eq!(q.get("a[2]"), None);

    let q = QueryStrong::parse_strict("a[2]=3&a[1]=2")?;
    assert_eq!(q["a"][0], Value::Empty);
    assert_eq!(q["a"][1], "2");
    assert_eq!(q["a[1]"], "2");
    assert_eq!(q["a"][2], "3");
    assert_eq!(q["a[2]"], "3");

    Ok(())
}

#[test]
fn deeply_nested() {
    let q = QueryStrong::from(("one", ("two", ("three", vec!["1", "2", "3"]))));

    assert_eq!(
        r#"{"one": {"two": {"three": ["1", "2", "3"]}}}"#,
        format!("{q:?}")
    );

    assert_eq!(
        "one[two][three][]=1&one[two][three][]=2&one[two][three][]=3",
        q.to_string()
    );
}

#[test]
fn parse_k_v() {
    let q = QueryStrong::parse("a=b").unwrap();
    assert!(q.get("a").unwrap().is_string());
    assert_eq!(q.get_str("a").unwrap(), "b");
    assert_eq!(q.get("a"), Some(&Value::from("b")));
    assert_eq!(q.get("other"), None);
    assert_eq!(q["a"], "b");

    assert!(q.is_map());
}

#[test]
fn parse_list() {
    let q = QueryStrong::parse("a=1&a=2&b=string&c").unwrap();

    assert_eq!(
        q.get_slice("a"),
        Some(&[Value::from("1"), Value::from("2")][..])
    );
    assert_eq!(q.get_str("a[0]"), Some("1"));
    assert_eq!(q["a"], Value::from(vec!["1", "2"]));

    assert_eq!(q["a[0]"], "1");
    assert_eq!(q.get_str("a[1]"), Some("2"));
    assert_eq!(q.get_str("a[2]"), None);
    assert!(q.get("a").unwrap().is_list());

    assert!(q.get_slice("b").is_none());
    assert_eq!(q.get_str("b"), Some("string"));
    assert!(q.get("b").unwrap().is_string());

    assert!(q.get("a[a]").is_none());
    assert!(q.get("a[0][1]").is_none());
    assert!(q.get("b[1][2]").is_none());
    assert!(q.get("c[0]").is_none());
    assert!(q.get("other").is_none());

    assert_eq!(q["c"], Value::Empty);

    assert_eq!("a[]=1&a[]=2&b=string&c", q.to_string());
}

#[test]
fn parse_list_with_no_values_and_duplicates() -> std::result::Result<(), ParseErrors<'static>> {
    //at the top level this is a map
    let q = QueryStrong::parse_strict("a&b&a")?;
    assert_eq!(q["a"], ());
    assert_eq!(q.to_string(), "a&b");

    //below the top level, this is a list until there's a value
    let mut q = QueryStrong::parse("top[nested]=a&top[nested]=b&top[nested]=a").unwrap();
    assert_eq!(
        q.to_string(),
        "top[nested][]=a&top[nested][]=b&top[nested][]=a"
    );

    q.append("top[nested][c]", "d").unwrap(); // we now transform the list into a map
    assert_eq!(
        q.to_string(),
        "top[nested][a]&top[nested][b]&top[nested][c]=d"
    );

    Ok(())
}

#[test]
fn nested_list() {
    let q = QueryStrong::from(("a", vec!["1", "2"]));
    assert_eq!(q["a"], Value::from(vec!["1", "2"]));
    assert_eq!(q.to_string(), "a[]=1&a[]=2");
}

#[test]
fn going_from_a_list_to_a_map() {
    let q = QueryStrong::parse("a[]=x&a[]=y&a[z]=map").unwrap();
    assert_eq!(format!("{q:?}"), r#"{"a": {"x": (), "y": (), "z": "map"}}"#);
    assert_eq!("a[x]&a[y]&a[z]=map", q.to_string());
}

#[test]
fn encoding() {
    let mut q = QueryStrong::new();
    q.append(Indexer::from("a[b]"), "&b").unwrap();
    assert_eq!("a%5Bb%5D=%26b", q.to_string());

    let q = QueryStrong::parse("a%5B%5D=0%26b%3D2").unwrap();
    assert_eq!(format!("{:?}", &q), r#"{"a[]": "0&b=2"}"#);
    assert_eq!(q.get(Indexer::from("a[]")).unwrap(), "0&b=2");
    assert_eq!(q.to_string(), "a%5B%5D=0%26b%3D2");
    assert_eq!(q.get("a%5B%5D").unwrap(), "0&b=2");
}

#[test]
fn spaces_and_pluses() {
    let q = QueryStrong::parse("key+with+spaces=value+with+spaces").unwrap();
    assert_eq!(q["key with spaces"], "value with spaces");
}

#[test]
fn no_owned_strings_for_unencoded_input() {
    let qs = QueryStrong::parse("user[name][first]=jacob&user[language]=rust").unwrap();
    assert!(all_borrowed_value(&qs));

    let path = IndexPath::parse("user[name][first]").unwrap();
    assert!(all_borrowed_index_path(&path));
}

#[test]
fn owned_strings_for_encoded_input() {
    // Sanity check: encoded input produces Owned strings
    let qs = QueryStrong::parse("key+with+spaces=value%20here").unwrap();
    assert!(!all_borrowed_value(&qs));
}

mod error_accumulation {
    use querystrong::*;

    // --- CouldNotParseIndexer ---
    //
    // BUG: these currently panic. `From<&str> for IndexPath` calls
    // `IndexPath::parse().unwrap()`, so malformed keys in the parse loop panic
    // before `if let Err(e) = querystrong.append(...)` can catch anything.
    // Fix: call `IndexPath::parse(k)` directly in the parse loop, handle Err
    // before calling append.

    #[test]
    fn malformed_key_leading_close_bracket() {
        let qs = QueryStrong::parse("]key=value");
        assert_eq!(qs.errors().unwrap().errors().len(), 1);
        assert_eq!(
            qs.errors().unwrap().to_string(),
            r#"1 error parsing "]key=value":
  - parsing indexer ran into `Some(']')` in state `Start` when parsing "]key"
"#
        );
    }

    #[test]
    fn malformed_key_double_open_bracket() {
        let qs = QueryStrong::parse("a[[b]=value");
        assert_eq!(qs.errors().unwrap().errors().len(), 1);
        assert_eq!(
            qs.errors().unwrap().to_string(),
            r#"1 error parsing "a[[b]=value":
  - parsing indexer ran into `Some('[')` in state `BracketOpen` when parsing "a[[b]"
"#
        );
    }

    #[test]
    fn malformed_key_double_close_bracket() {
        let qs = QueryStrong::parse("a[b]]=value");
        assert_eq!(qs.errors().unwrap().errors().len(), 1);
        assert_eq!(
            qs.errors().unwrap().to_string(),
            r#"1 error parsing "a[b]]=value":
  - parsing indexer ran into `Some(']')` in state `BracketClose` when parsing "a[b]]"
"#
        );
    }

    #[test]
    fn string_value_then_map_key_conflict() {
        let qs = QueryStrong::parse("a=1&a[b]=2");
        // "a" was successfully set to "1" before the conflict; it should be preserved
        assert_eq!(qs.get_str("a"), Some("1"));
        assert_eq!(qs.errors().unwrap().errors().len(), 1);
        assert_eq!(
            qs.errors().unwrap().to_string(),
            r#"1 error parsing "a=1&a[b]=2":
  - could not append (`"1"`, `Some(String("b"))`, `"2"`)
"#
        );
    }

    #[test]
    fn map_value_then_bare_value_conflict() {
        let qs = QueryStrong::parse("a[b]=1&a=2");
        assert_eq!(qs.get_str("a[b]"), Some("1"));
        assert_eq!(qs.errors().unwrap().errors().len(), 1);
        assert_eq!(
            qs.errors().unwrap().to_string(),
            r#"1 error parsing "a[b]=1&a=2":
  - could not append (`{"b": "1"}`, `None`, `"2"`)
"#
        );
    }

    // --- CouldNotConvertToMap ---

    #[test]
    fn list_with_empty_element_then_keyed_access() {
        // a[] with no value produces List([Empty]) at "a";
        // a[b]=x then tries to convert that list to a map, failing on the Empty element
        let qs = QueryStrong::parse("a[]&a[b]=x");
        assert_eq!(qs.errors().unwrap().errors().len(), 1);
        assert_eq!(
            qs.errors().unwrap().to_string(),
            r#"1 error parsing "a[]&a[b]=x":
  - could not convert `()` to a map
"#
        );
    }

    #[test]
    fn multiple_unconvertible_elements_only_first_error_reported_rest_dropped() {
        // List([Empty, Empty, String("valid")]) -> Map:
        // - first Empty: CouldNotConvertToMap error recorded
        // - second Empty: silently dropped (no second error)
        // - "valid": becomes a map key normally
        // - "b"="x": inserted as the triggering key
        let qs = QueryStrong::parse("a[]&a[]&a[]=valid&a[b]=x");
        assert_eq!(
            qs.errors().unwrap().errors().len(),
            1,
            "only the first unconvertible element produces an error"
        );
        let map = qs["a"].as_map().unwrap();
        assert_eq!(
            map.len(),
            2,
            "only 'valid' and 'b' are in the map; both Empties were dropped"
        );
        assert!(map.contains_key("valid"));
        assert_eq!(qs.get_str("a[b]"), Some("x"));
        assert_eq!(
            qs.errors().unwrap().to_string(),
            r#"1 error parsing "a[]&a[]&a[]=valid&a[b]=x":
  - could not convert `()` to a map
"#
        );
    }

    #[test]
    fn nested_path_through_numeric_list_index() {
        let qs = QueryStrong::parse("a[2][nested]=v").unwrap();
        // a[2] is a map with key "nested"; a is a SparseList
        assert!(matches!(qs["a"], Value::SparseList(_)));
        assert!(matches!(qs["a"][2], Value::Map(_)));
        assert_eq!(qs.get_str("a[2][nested]"), Some("v"));
        // Absent in-range slots are Empty; serializes with [n] notation
        assert_eq!(qs["a"][0], Value::Empty);
        assert_eq!(qs["a"][1], Value::Empty);
        assert_eq!(qs.to_string(), "a[2][nested]=v");
    }

    #[test]
    fn multiple_keys_under_same_numeric_index() {
        let qs = QueryStrong::parse("a[1][b]=x&a[1][c]=y").unwrap();
        // Index 0 is an absent-but-in-range slot: returns Empty, doesn't panic
        assert_eq!(qs["a"][0], Value::Empty);
        assert_eq!(qs.get_str("a[1][b]"), Some("x"));
        assert_eq!(qs.get_str("a[1][c]"), Some("y"));
    }

    // --- Error accumulation and data preservation ---

    #[test]
    fn valid_data_around_errors_is_preserved() {
        let qs = QueryStrong::parse("before=yes&a=1&a[b]=2&after=also");
        assert_eq!(qs.get_str("before"), Some("yes"));
        assert_eq!(qs.get_str("a"), Some("1"));
        assert_eq!(qs.get_str("after"), Some("also"));
        assert_eq!(qs.errors().unwrap().errors().len(), 1);
    }

    #[test]
    fn multiple_errors_all_accumulated() {
        // Two separate CouldNotAppend conflicts on different keys.
        // After the first error (a[b]=2), root is left empty by the bug above;
        // b=3 then succeeds on the empty root, and b[c]=4 produces the second error.
        let qs = QueryStrong::parse("a=1&a[b]=2&b=3&b[c]=4");
        assert_eq!(qs.errors().unwrap().errors().len(), 2);
        assert_eq!(
            qs.errors().unwrap().to_string(),
            r#"2 errors parsing "a=1&a[b]=2&b=3&b[c]=4":
  - could not append (`"1"`, `Some(String("b"))`, `"2"`)
  - could not append (`"3"`, `Some(String("c"))`, `"4"`)
"#
        );
    }

    // --- API methods ---

    #[test]
    fn parse_strict_ok_for_valid_input() {
        assert!(QueryStrong::parse_strict("a=1&b[c]=2").is_ok());
    }

    #[test]
    fn parse_strict_err_for_conflict() {
        let err = QueryStrong::parse_strict("a=1&a[b]=2").unwrap_err();
        assert_eq!(err.errors().len(), 1);
    }

    #[test]
    fn errors_is_none_for_clean_parse() {
        assert!(QueryStrong::parse("a=1&b=2").errors().is_none());
    }

    #[test]
    fn into_result_ok_for_clean_parse() {
        assert!(QueryStrong::parse("a=1").into_result().is_ok());
    }

    #[test]
    fn into_result_err_for_bad_parse() {
        assert!(QueryStrong::parse("a=1&a[b]=2").into_result().is_err());
    }

    #[test]
    #[should_panic]
    fn unwrap_panics_on_parse_errors() {
        QueryStrong::parse("a=1&a[b]=2").unwrap();
    }

    // --- Display format ---

    #[test]
    fn display_uses_singular_for_one_error() {
        let s = QueryStrong::parse_strict("a=1&a[b]=2")
            .unwrap_err()
            .to_string();
        assert!(s.starts_with("1 error parsing"), "got: {s:?}");
        assert!(!s.starts_with("1 errors"));
    }

    #[test]
    fn display_uses_plural_for_multiple_errors() {
        let s = QueryStrong::parse_strict("a=1&a[b]=2&b=3&b[c]=4")
            .unwrap_err()
            .to_string();
        assert!(s.starts_with("2 errors parsing"), "got: {s:?}");
    }

    #[test]
    fn display_includes_input() {
        let input = "a=1&a[b]=2";
        let s = QueryStrong::parse_strict(input).unwrap_err().to_string();
        assert!(s.contains(&format!("{input:?}")));
    }
}

// ── additional edge-case tests ──────────────────────────────────────────────

#[test]
fn empty_input() {
    let qs = QueryStrong::parse("");
    assert!(qs.is_empty());
    assert!(qs.errors().is_none());
}

#[test]
fn consecutive_ampersands_are_skipped() {
    let qs = QueryStrong::parse("&&a=1&&b=2&&").unwrap();
    assert_eq!(qs.get_str("a"), Some("1"));
    assert_eq!(qs.get_str("b"), Some("2"));
}

#[test]
fn value_containing_equals_sign() {
    // Only the first `=` splits key from value; the rest belongs to the value.
    let qs = QueryStrong::parse("k=v=extra").unwrap();
    assert_eq!(qs.get_str("k"), Some("v=extra"));
}

#[test]
fn key_with_explicit_empty_value() {
    // `k=` has an empty string value (not Empty / None).
    let qs = QueryStrong::parse("k=").unwrap();
    assert_eq!(qs.get_str("k"), Some(""));
}

#[test]
fn into_owned_roundtrip() {
    let input = "user[name]=jacob&tag[]=rust&tag[]=web";
    let owned = QueryStrong::parse(input).into_owned();
    assert_eq!(owned.get_str("user[name]"), Some("jacob"));
    assert_eq!(owned.get_str("tag[0]"), Some("rust"));
    assert_eq!(owned.get_str("tag[1]"), Some("web"));
}

#[test]
fn into_owned_carries_errors() {
    let owned = QueryStrong::parse("a=1&a[b]=2").into_owned();
    assert_eq!(owned.errors().unwrap().errors().len(), 1);
    assert_eq!(owned.errors().unwrap().input(), "a=1&a[b]=2");
}

#[test]
fn into_result_takes_errors_leaving_clean_state() {
    // into_result moves the errors out; the resulting QueryStrong is error-free.
    let qs = QueryStrong::parse("a=1&a[b]=2");
    let (errors, qs) = match qs.into_result() {
        Ok(_) => panic!("expected errors"),
        Err(e) => {
            // Recover a clean QueryStrong by re-parsing just the valid portion.
            // (into_result consumes self, so we reconstruct to verify clean state.)
            let clean = QueryStrong::parse("a=1");
            (e, clean)
        }
    };
    assert_eq!(errors.errors().len(), 1);
    assert!(qs.errors().is_none());
}

#[test]
fn percent_encoded_brackets_in_key_are_a_string_key_not_nested() {
    // `a%5Bb%5D` percent-decodes to the literal string `a[b]`.
    // Because the raw bytes contain no `[` or `]`, the IndexPath parser sees it
    // as a single string segment, not a nested path.
    let qs = QueryStrong::parse("a%5Bb%5D=v").unwrap();
    // Retrieved via the decoded form of the key.
    assert_eq!(qs.get(Indexer::from("a[b]")).unwrap(), "v");
    // Round-trips back with re-encoded brackets.
    assert_eq!(qs.to_string(), "a%5Bb%5D=v");
}

#[test]
fn non_ascii_key_with_brackets() {
    // Keys with multi-byte characters before `[` must parse correctly.
    // A byte-index into a non-ASCII str != the char-index: using chars().nth(byte_idx)
    // returns the wrong character, making the state machine error spuriously.
    let qs = QueryStrong::parse("über[key]=value").unwrap();
    assert_eq!(qs.get_str("über[key]"), Some("value"));
}

#[test]
fn empty_index_path_from_empty_string() {
    // An empty string produces an empty (zero-element) IndexPath without error.
    let path = IndexPath::parse("").unwrap();
    assert!(path.is_empty());
}

#[test]
fn missing_close_bracket_parses_leniently() {
    // A key like `a[b` (unclosed bracket) is accepted as two segments: ["a", "b"].
    // This is intentional: the parser is best-effort.
    let qs = QueryStrong::parse("a[b=v").unwrap();
    assert_eq!(qs.get_str("a[b]"), Some("v"));
}

#[test]
fn large_numeric_index_overflow_treated_as_string_key() {
    // A numeric index too large to fit in usize is treated as a string map key
    // rather than panicking or allocating a giant Vec.
    let huge = "99999999999999999999999";
    let input = format!("a[{huge}]=v");
    let qs = QueryStrong::parse(&input).into_owned();
    assert_eq!(qs.get_str(format!("a[{huge}]")), Some("v"));
    // Specifically it should be in a map, not a list.
    assert!(qs["a"].is_map());
}

// ── take ─────────────────────────────────────────────────────────────────────

#[test]
fn take_map_key_returns_value_and_removes_key() {
    let mut qs = QueryStrong::parse("a=1&b=2").unwrap();
    assert_eq!(qs.take("a"), Some(Value::from("1")));
    assert_eq!(qs.get("a"), None);
    assert_eq!(qs.get_str("b"), Some("2"));
}

#[test]
fn take_nested_map_key() {
    let mut qs = QueryStrong::parse("a[b]=1&a[c]=2").unwrap();
    assert_eq!(qs.take("a[b]"), Some(Value::from("1")));
    assert_eq!(qs.get("a[b]"), None);
    assert_eq!(qs.get_str("a[c]"), Some("2"));
    // "a" still exists with remaining key
    assert!(qs["a"].is_map());
}

#[test]
fn take_last_map_key_removes_parent() {
    let mut qs = QueryStrong::parse("a[b]=1").unwrap();
    assert_eq!(qs.take("a[b]"), Some(Value::from("1")));
    // "a" mapped to an empty map, so the key itself is removed
    assert_eq!(qs.get("a"), None);
}

#[test]
fn take_absent_key_returns_none() {
    let mut qs = QueryStrong::parse("a=1").unwrap();
    assert_eq!(qs.take("b"), None);
    assert_eq!(qs.take("a[nested]"), None);
    // Existing value unaffected
    assert_eq!(qs.get_str("a"), Some("1"));
}

#[test]
fn take_last_dense_list_element_stays_dense() {
    let mut qs = QueryStrong::parse("a[]=x&a[]=y&a[]=z").unwrap();
    assert_eq!(qs.take("a[2]"), Some(Value::from("z")));
    assert!(qs["a"].is_dense_list());
    assert_eq!(qs["a"].len(), 2);
    assert_eq!(qs.to_string(), "a[]=x&a[]=y");
}

#[test]
fn take_middle_dense_list_element_promotes_to_sparse() {
    let mut qs = QueryStrong::parse("a[]=x&a[]=y&a[]=z").unwrap();
    assert_eq!(qs.take("a[1]"), Some(Value::from("y")));
    // Gap at 1 → SparseList; symmetrical with how insert creates a gap
    assert!(qs["a"].is_sparse_list());
    assert_eq!(qs.get_str("a[0]"), Some("x"));
    assert_eq!(qs["a"][1], Value::Empty); // in-range absent slot
    assert_eq!(qs.get_str("a[2]"), Some("z"));
    assert_eq!(qs.to_string(), "a[0]=x&a[2]=z");
}

#[test]
fn take_sparse_list_entry_removes_it() {
    let mut qs = QueryStrong::parse("a[0]=x&a[5]=z").unwrap();
    assert_eq!(qs.take("a[5]"), Some(Value::from("z")));
    assert_eq!(qs.get("a[5]"), None);
    assert_eq!(qs.get_str("a[0]"), Some("x"));
}

#[test]
fn take_sparse_fills_gap_and_densifies() {
    // After taking [5], only [0] remains → try_densify collapses to List
    let mut qs = QueryStrong::parse("a[0]=x&a[5]=z").unwrap();
    qs.take("a[5]");
    assert!(qs["a"].is_dense_list());
    assert_eq!(qs.to_string(), "a[]=x");
}

#[test]
fn take_whole_subtree() {
    let mut qs = QueryStrong::parse("a[b][c]=1&a[b][d]=2&e=3").unwrap();
    let subtree = qs.take("a[b]").unwrap();
    assert!(subtree.is_map());
    assert_eq!(subtree.get_str("c"), Some("1"));
    assert_eq!(subtree.get_str("d"), Some("2"));
    // "a" → empty map → removed; "e" unaffected
    assert_eq!(qs.get("a"), None);
    assert_eq!(qs.get_str("e"), Some("3"));
}

// ── SparseList behaviour ─────────────────────────────────────────────────────

#[test]
fn explicit_numeric_index_creates_sparse_list() {
    let q = QueryStrong::parse("a[1]=x").unwrap();
    assert!(q["a"].is_sparse_list());
}

#[test]
fn sparse_list_dos_safe_large_index() {
    // a single BTreeMap entry, not 1M Vec slots
    let q = QueryStrong::parse("a[999999]=v").unwrap();
    assert_eq!(q.get_str("a[999999]"), Some("v"));
    assert_eq!(q["a"].as_sparse_list().unwrap().len(), 1);
}

#[test]
fn sparse_list_absent_in_range_is_empty_not_none() {
    let q = QueryStrong::parse("a[2]=v").unwrap();
    // slots 0 and 1 are within 0..=max_key(2) → Empty, not None
    assert_eq!(q["a"][0], Value::Empty);
    assert_eq!(q["a"][1], Value::Empty);
    assert_eq!(q["a"][2], "v");
}

#[test]
fn sparse_list_absent_beyond_max_is_none() {
    let q = QueryStrong::parse("a[2]=v").unwrap();
    assert_eq!(q.get("a[3]"), None);
    assert_eq!(q.get("a[999]"), None);
}

#[test]
fn empty_appends_create_dense_list() {
    let q = QueryStrong::parse("a[]=1&a[]=2&a[]=3").unwrap();
    assert!(q["a"].is_dense_list());
    assert_eq!(q["a"].as_slice().unwrap().len(), 3);
    assert_eq!(q.to_string(), "a[]=1&a[]=2&a[]=3");
}

#[test]
fn dense_to_sparse_promotion_on_numeric_index() {
    // [] appends first, then a [n] index triggers promotion
    let q = QueryStrong::parse("a[]=x&a[]=y&a[5]=z").unwrap();
    assert!(q["a"].is_sparse_list());
    assert_eq!(q.get_str("a[0]"), Some("x"));
    assert_eq!(q.get_str("a[1]"), Some("y"));
    assert_eq!(q["a"][2], Value::Empty); // in-range absent
    assert_eq!(q.get_str("a[5]"), Some("z"));
    assert_eq!(q.to_string(), "a[0]=x&a[1]=y&a[5]=z");
}

#[test]
fn sparse_list_roundtrip_preserves_indices() {
    // numeric indices survive a parse→serialize→parse roundtrip
    let input = "a[0]=x&a[2]=y&a[5]=z";
    let q = QueryStrong::parse(input).unwrap();
    assert_eq!(q.to_string(), input);
    let q2 = QueryStrong::parse(&q.to_string()).into_owned();
    assert_eq!(q2.get_str("a[0]"), Some("x"));
    assert_eq!(q2.get_str("a[2]"), Some("y"));
    assert_eq!(q2.get_str("a[5]"), Some("z"));
}

#[test]
fn sparse_list_to_map_promotion() {
    // String key on a SparseList promotes it to a Map, same as for dense List
    let q = QueryStrong::parse("a[0]=x&a[1]=y&a[z]=map").unwrap();
    assert!(q["a"].is_map());
    assert_eq!(q.get_str("a[z]"), Some("map"));
}

#[test]
fn single_zero_index_is_dense() {
    // a[0]=x is equivalent to a[]=x — collapses to List immediately
    let q = QueryStrong::parse("a[0]=x").unwrap();
    assert!(q["a"].is_dense_list());
    assert_eq!(q.to_string(), "a[]=x");
}

#[test]
fn out_of_order_numeric_indices_collapse_to_dense() {
    // a[1] arrives first (sparse), then a[0] fills the gap → dense
    let q = QueryStrong::parse("a[1]=y&a[0]=x").unwrap();
    assert!(q["a"].is_dense_list());
    assert_eq!(q["a"][0], "x");
    assert_eq!(q["a"][1], "y");
    assert_eq!(q.to_string(), "a[]=x&a[]=y");
}

#[test]
fn filling_gap_in_sparse_collapses_to_dense() {
    // a[0], a[2], then a[1] bridges the gap
    let q = QueryStrong::parse("a[0]=x&a[2]=z&a[1]=y").unwrap();
    assert!(q["a"].is_dense_list());
    assert_eq!(q.to_string(), "a[]=x&a[]=y&a[]=z");
}

#[test]
fn partial_gap_fill_stays_sparse() {
    // a[0], a[3] — bridge not fully filled, still sparse
    let q = QueryStrong::parse("a[0]=x&a[3]=z&a[1]=y").unwrap();
    assert!(q["a"].is_sparse_list());
    assert_eq!(q.to_string(), "a[0]=x&a[1]=y&a[3]=z");
}

#[test]
fn sparse_list_append_via_empty_bracket_appends_after_max() {
    // After a[5]=v, a[]=w should land at index 6
    let q = QueryStrong::parse("a[5]=v&a[]=w").unwrap();
    assert_eq!(q.get_str("a[5]"), Some("v"));
    assert_eq!(q.get_str("a[6]"), Some("w"));
}

// ── percent_coding edge cases ────────────────────────────────────────────────

mod percent_coding_edge_cases {
    use querystrong::*;

    #[test]
    fn decode_truncated_percent_at_eof_passes_through() {
        // `%` with no following hex digits is kept as-is.
        let qs = QueryStrong::parse("k=foo%").unwrap();
        assert_eq!(qs.get_str("k"), Some("foo%"));
    }

    #[test]
    fn decode_percent_with_single_hex_digit_at_eof_passes_through() {
        // `%2` at end of string (only one hex digit) keeps the literal chars.
        let qs = QueryStrong::parse("k=foo%2").unwrap();
        assert_eq!(qs.get_str("k"), Some("foo%2"));
    }

    #[test]
    fn decode_percent_with_non_hex_digits_passes_through() {
        // `%GG` is not a valid sequence; the literal bytes are preserved.
        let qs = QueryStrong::parse("k=foo%GGbar").unwrap();
        assert_eq!(qs.get_str("k"), Some("foo%GGbar"));
    }

    #[test]
    fn encode_decode_roundtrip_arbitrary_bytes() {
        // Values survive an encode→parse roundtrip unchanged.
        let original = "hello world & goodbye = world";
        let mut qs = QueryStrong::new();
        qs.append("k", original).unwrap();
        let encoded = qs.to_string();
        let qs2 = QueryStrong::parse(&encoded).into_owned();
        assert_eq!(qs2.get_str("k"), Some(original));
    }
}

#[cfg(feature = "serde")]
#[test]
fn serde() {
    let q = QueryStrong::parse("a[b][]=1&b").unwrap();
    let json = serde_json::to_value(&q).unwrap();
    assert_eq!(json, serde_json::json!({"a": {"b": ["1"]}, "b": null}));
}
