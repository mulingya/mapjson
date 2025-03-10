use mapjson::{JsonReaderSettings, JsonWriterSettings, Map, Value};

#[test]
fn standard_format() {
    let mut vec = Vec::new();
    vec.push(Value::String("hi".to_string()));
    vec.push(Value::String("china".to_string()));

    let mut map1 = Map::new();
    map1.insert("a1".to_string(), Value::F64(11.));
    map1.insert("b1".to_string(), Value::F64(22.));

    let mut map = Map::new();
    map.insert("a".to_string(), Value::Null);
    map.insert("b".to_string(), Value::Bool(true));
    map.insert("c".to_string(), Value::F64(3.14));
    map.insert("d".to_string(), Value::String("hello".to_string()));
    map.insert("e".to_string(), Value::Vec(vec));
    map.insert("f".to_string(), Value::Object(map1));

    let json = map.to_json();
    assert_ne!(json, "");
    assert_ne!(json, "{}");
    assert_eq!(json.len(), 81);
    let settings = JsonWriterSettings {
        indentation: "  ".to_string(),
    };
    let json = map.to_json_with_settings(settings);
    assert_ne!(json, "");
    assert_ne!(json, "{}");
    assert_eq!(json.len(), 147);
}

#[test]
fn default_values_when_omitted() {
    let map = Map::new();
    assert_eq!(map.to_json(), "{}");
    let settings = JsonWriterSettings {
        indentation: "  ".to_string(),
    };
    assert_eq!(map.to_json_with_settings(settings), "{}");
}

#[test]
fn nested_format() {
    let mut map1 = Map::new();
    map1.insert("a".to_string(), Value::Null);
    map1.insert("b".to_string(), Value::Bool(false));

    let mut map2 = Map::new();
    map2.insert("c".to_string(), Value::F64(6.18));
    map2.insert("d".to_string(), Value::String("hello".to_string()));

    let mut vec = Vec::new();
    vec.push(Value::Object(map1));
    vec.push(Value::Object(map2));

    let mut map = Map::new();
    map.insert("x".to_string(), Value::Bool(true));
    map.insert("y".to_string(), Value::Vec(vec));

    let settings = JsonWriterSettings {
        indentation: "  ".to_string(),
    };
    let json = map.to_json_with_settings(settings);
    assert_eq!(json.len(), 136);
}

#[test]
fn parse() {
    let json =
        r#"{"a":null,"b":true,"c":3.14,"d":"hello","e":["hi","china"],"f":{"a1":11,"b1":22}}"#;
    let mut map = Map::new();
    if let Err(e) = map.merge(json) {
        panic!("{}", e);
    }
    assert_eq!(map.len(), 6);
}

#[test]
fn nested_parse() {
    let json = r#"{"a1":{"a2":{"a3":true}}}"#;
    let mut map = Map::new();

    if let Err(e) = map.merge(json) {
        panic!("{}", e);
    }
    assert_eq!(map.len(), 1);

    let map = map.get("a1").unwrap();
    let map = map.as_object().unwrap();
    assert_eq!(map.len(), 1);

    let map = map.get("a2").unwrap();
    let map = map.as_object().unwrap();
    assert_eq!(map.len(), 1);

    let map = map.get("a3").unwrap();
    let b = map.as_bool().unwrap();
    assert_eq!(b, true);
}

#[test]
fn recursion_limit() {
    let json = r#"{"a1":{"a2":{"a3":true}}}"#;
    let mut map = Map::new();
    let settings = JsonReaderSettings { recursion_limit: 2 };

    if let Err(e) = map.merge_with_settings(json, settings) {
        assert_eq!("The set recursion depth is exceeded: 2", e);
    }
}
