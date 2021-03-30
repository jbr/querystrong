

use querystrong::*;

#[test]
fn parse_key() {
    assert_eq!(
        "a[b][]".parse::<IndexPath>().unwrap(),
        vec![Indexer::from("a"), Indexer::from("b"), Indexer::Empty]
    );

    assert_eq!(
        "a[0][b]".parse::<IndexPath>().unwrap(),
        vec![
            Indexer::String("a".into()),
            Indexer::Number(0),
            Indexer::String("b".into())
        ]
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
fn list_access() {
    let q = QueryStrong::parse("a[]=1&a[]=2&a[]=3").unwrap();
    assert_eq!(q["a"][1], "2");
    assert_eq!(q["a[1]"], "2");

    let q = QueryStrong::parse("a[1]=2").unwrap();
    assert_eq!(q["a"][0], Value::Empty);
    assert_eq!(q["a"][1], "2");
    assert_eq!(q["a[1]"], "2");

    let q = QueryStrong::parse("a[2]=3&a[1]=2").unwrap();
    assert_eq!(q["a"][0], Value::Empty);
    assert_eq!(q["a"][1], "2");
    assert_eq!(q["a[1]"], "2");
    assert_eq!(q["a"][2], "3");
    assert_eq!(q["a[2]"], "3");
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
    assert_eq!(q["a"], "b");
}

#[test]
fn parse_list() {
    let q: QueryStrong = "a=1&a=2&b".parse().unwrap();
    assert_eq!(q["a"], Value::from(vec!["1", "2"]));
    assert_eq!(q["b"], Value::Empty);
    assert_eq!("a[]=1&a[]=2&b", q.to_string());
}

#[test]
fn parse_list_with_no_values_and_duplicates() {
    let q = "a&b&a".parse::<QueryStrong>().unwrap();
    assert_eq!(q["a"], ());
    assert_eq!(q["b"], ());

    assert_eq!(q.to_string(), "a&b");
}

#[test]
fn nested_list() {
    let q = QueryStrong::from(("a", vec!["1", "2"]));
    assert_eq!(q["a"], Value::from(vec!["1", "2"]));
    assert_eq!(q.to_string(), "a[]=1&a[]=2");
}
