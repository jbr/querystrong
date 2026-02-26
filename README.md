# querystrong

QueryStrong parses query strings like `user[name][first]=jacob&user[language]=rust`
into a nested value tree that can be traversed, mutated, and serialized back to a string.

```rust
use querystrong::QueryStrong;

let mut qs = QueryStrong::parse("user[name][first]=jacob&user[language]=rust");
assert_eq!(qs["user[name][first]"], "jacob");
assert_eq!(qs.get_str("user[language]"), Some("rust"));
assert!(qs["user"].is_map());

qs.append("user[name][last]", "rothstein").unwrap();
qs.append("user[language]", "english").unwrap();
assert_eq!(
    qs.to_string(),
    "user[language][]=rust&user[language][]=english&\
     user[name][first]=jacob&user[name][last]=rothstein"
);
```

For full documentation, see [the api docs][docs]

* [API Docs][docs] [![docs.rs docs][docs-badge]][docs]
* [Releases][releases] [![crates.io version][version-badge]][crate]
* [Contributing][contributing]
* [CI ![CI][ci-badge]][ci]
* [API docs for main][main-docs]

[ci]: https://github.com/jbr/querystrong/actions?query=workflow%3ACI
[ci-badge]: https://github.com/jbr/querystrong/workflows/CI/badge.svg
[releases]: https://github.com/jbr/querystrong/releases
[docs]: https://docs.rs/querystrong
[contributing]: https://github.com/jbr/querystrong/blob/main/.github/CONTRIBUTING.md
[crate]: https://crates.io/crates/querystrong
[docs-badge]: https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square
[version-badge]: https://img.shields.io/crates/v/querystrong.svg?style=flat-square
[main-docs]: https://jbr.github.io/querystrong/querystrong/

## Safety
This crate uses `#![deny(unsafe_code)]`.

## License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br/>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
