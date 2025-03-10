# MapJson
![license](https://img.shields.io/badge/license-MIT-blue)
![license](https://img.shields.io/badge/release-v0.1.0-green)

一个基于标准库设计的Map与Json的转换器，没有任何外部依赖。

## 快速开始
``` rust
use mapjson::{Map, Value};

let mut vec = Vec::new();
vec.push(Value::F64(11.));
vec.push(Value::F64(22.));

let mut map = Map::new();
map.insert("a".to_string(), Value::Vec(vec));
map.insert("b".to_string(), Value::String("hi".to_string()));
let json = map.to_json();
println!("{}", json.as_str());

let mut obj = Map::new();
obj.merge(json.as_str()).unwrap();

println!("{}", obj.len());
```