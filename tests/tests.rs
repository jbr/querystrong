use querystrong::*;

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
fn list_access() -> Result<()> {
    let q = QueryStrong::parse("a[]=1&a[]=2&a[]=3")?;
    assert_eq!(q["a"][1], "2");
    assert_eq!(q["a[1]"], "2");

    let q = QueryStrong::parse("a[1]=2")?;
    assert_eq!(q["a"][0], Value::Empty);
    assert_eq!(q["a"][1], "2");
    assert_eq!(q["a[1]"], "2");

    let q = QueryStrong::parse("a[2]=3&a[1]=2")?;
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
        format!("{:?}", q)
    );

    assert_eq!(
        "one[two][three][]=1&one[two][three][]=2&one[two][three][]=3",
        q.to_string()
    );
}

#[test]
fn parse_k_v() {
    let q: QueryStrong = "a=b".parse().unwrap();
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
fn parse_list_with_no_values_and_duplicates() -> Result<()> {
    //at the top level this is a map
    let q = QueryStrong::parse("a&b&a")?;
    assert_eq!(q["a"], ());
    assert_eq!(q.to_string(), "a&b");

    //below the top level, this is a list until there's a value
    let mut q = QueryStrong::parse("top[nested]=a&top[nested]=b&top[nested]=a").unwrap();
    assert_eq!(
        q.to_string(),
        "top[nested][]=a&top[nested][]=b&top[nested][]=a"
    );

    q.append("top[nested][c]", "d")?; // we now transform the list into a map
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
    assert_eq!(
        format!("{:?}", q),
        r#"{"a": {"x": (), "y": (), "z": "map"}}"#
    );
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
