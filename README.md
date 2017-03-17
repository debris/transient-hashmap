# transient-hashmap
Simple rust HashMap with transient entries.

[![Build Status][travis-image]][travis-url]

[travis-image]: https://travis-ci.org/debris/transient-hashmap.svg?branch=master
[travis-url]: https://travis-ci.org/debris/transient-hashmap

[Documentation](http://debris.github.io/transient-hashmap/transient_hashmap/index.html)

## Example

```rust
use transient_hashmap::TransientHashMap;

let entry_lifetime_seconds = 0;
let mut map = TransientHashMap::new(entry_lifetime_seconds);
map.insert(10, "Hello World");

// Clear old entries
map.prune();

// Item is not there
assert_eq!(map.contains_key(10), false);
```

`Cargo.toml`

```
[dependencies]
transient-hashmap = "0.4"
```

