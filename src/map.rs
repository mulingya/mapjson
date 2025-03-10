use crate::json_reader::JsonReader;
use crate::json_writer::JsonWriter;
use crate::Value;
use crate::{JsonReaderSettings, JsonWriterSettings};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

/// 可以与Json格式互相转换的`Map`.
#[derive(Clone, PartialEq)]
pub struct Map(HashMap<String, Value>);

impl Map {
    /// 创建一个空的`Map`.
    /// 这个`Map`的初始容量为0, 直到它插入第一个元素.
    ///
    /// # 例子
    ///
    /// ```
    /// use mapjson::{Map, Value};
    ///
    /// let mut map = Map::new();
    /// assert_eq!(map.capacity(), 0);
    /// map.insert("Panda".to_string(), Value::Bool(true));
    /// assert_ne!(map.capacity(), 0);
    /// ```
    pub fn new() -> Self {
        Map(HashMap::new())
    }

    /// 合并两个`Map`, 当Key相同时, 则新值覆盖旧值, 否则插入该键值对.
    ///
    /// # 例子
    ///
    /// ```
    /// use mapjson::{Map, Value};
    ///
    /// let mut map1 = Map::new();
    /// map1.insert("111".to_string(), Value::F64(111.));
    /// map1.insert("222".to_string(), Value::F64(222.));
    /// let mut map2 = Map::new();
    /// map2.insert("222".to_string(), Value::F64(444.));
    /// map2.insert("333".to_string(), Value::F64(333.));
    /// assert_eq!(map1["222"].as_f64().unwrap(), 222.);
    /// assert_eq!(map1.len(), 2);
    ///
    /// map1.merge_from(map2);
    /// assert_eq!(map1["222"].as_f64().unwrap(), 444.);
    /// assert_eq!(map1.len(), 3);
    /// assert_eq!(map1["111"].as_f64().unwrap(), 111.);
    /// assert_eq!(map1["333"].as_f64().unwrap(), 333.);
    /// ```
    pub fn merge_from(&mut self, other: Map) {
        for (k, v) in other.0.into_iter() {
            self.0.insert(k, v);
        }
    }

    /// 将`Map`转换为Json结构, 带有默认设置.
    ///
    /// # 例子
    ///
    /// ```
    /// use mapjson::{Map, Value};
    ///
    /// let mut vec = Vec::new();
    /// vec.push(Value::String("hi".to_string()));
    /// vec.push(Value::String("china".to_string()));
    ///
    /// let mut map1 = Map::new();
    /// map1.insert("a1".to_string(), Value::F64(11.));
    /// map1.insert("b1".to_string(), Value::F64(22.));
    ///
    /// let mut map = Map::new();
    /// map.insert("a".to_string(), Value::Null);
    /// map.insert("b".to_string(), Value::Bool(true));
    /// map.insert("c".to_string(), Value::F64(3.14));
    /// map.insert("d".to_string(), Value::String("hello".to_string()));
    /// map.insert("e".to_string(), Value::Vec(vec));
    /// map.insert("f".to_string(), Value::Object(map1));
    ///
    /// assert_eq!(map.to_json().len(), 81);
    /// ```
    pub fn to_json(&self) -> String {
        self.to_json_with_settings(JsonWriterSettings::default())
    }

    /// 将`Map`转换为Json结构, 自定义格式化设置.
    ///
    /// # 例子
    ///
    /// ```
    /// use mapjson::{JsonWriterSettings, Map, Value};
    ///
    /// let mut vec = Vec::new();
    /// vec.push(Value::String("hi".to_string()));
    /// vec.push(Value::String("china".to_string()));
    ///
    /// let mut map1 = Map::new();
    /// map1.insert("a1".to_string(), Value::F64(11.));
    /// map1.insert("b1".to_string(), Value::F64(22.));
    ///
    /// let mut map = Map::new();
    /// map.insert("a".to_string(), Value::Null);
    /// map.insert("b".to_string(), Value::Bool(true));
    /// map.insert("c".to_string(), Value::F64(3.14));
    /// map.insert("d".to_string(), Value::String("hello".to_string()));
    /// map.insert("e".to_string(), Value::Vec(vec));
    /// map.insert("f".to_string(), Value::Object(map1));
    ///
    /// let settings = JsonWriterSettings {
    ///     indentation: "  ".to_string(),
    /// };
    /// assert_eq!(map.to_json_with_settings(settings).len(), 147);
    /// ```
    pub fn to_json_with_settings(&self, settings: JsonWriterSettings) -> String {
        JsonWriter::new(settings).format(self)
    }

    /// 将Json解析，并赋值给自身, 带有默认设置.
    ///
    /// # 例子
    ///
    /// ```
    /// use mapjson::Map;
    ///
    /// let json = r#"{"a":null,"b":true,"c":3.14,"d":"hello","e":["hi","china"],"f":{"a1":11,"b1":22}}"#;
    ///
    /// let mut map = Map::new();
    /// if let Err(e) = map.merge(json) {
    ///     eprintln!("{}", e);
    /// }
    ///
    /// assert_eq!(map.len(), 6);
    /// ```
    pub fn merge(&mut self, json: &str) -> Result<(), String> {
        self.merge_with_settings(json, JsonReaderSettings::default())
    }

    /// 将Json解析，并赋值给自身, 自定义设置.
    ///
    /// # 例子
    ///
    /// ```
    /// use mapjson::{JsonReaderSettings, Map};
    ///
    /// let json = r#"{"a1":{"a2":{"a3":true}}}"#;
    ///
    /// let mut map = Map::new();
    /// let settings = JsonReaderSettings {
    ///     recursion_limit: 2
    /// };
    ///
    /// if let Err(e) = map.merge_with_settings(json, settings) {
    ///     assert_eq!("The set recursion depth is exceeded: 2", e);
    /// }
    /// ```
    pub fn merge_with_settings(
        &mut self,
        json: &str,
        settings: JsonReaderSettings,
    ) -> Result<(), String> {
        JsonReader::new(settings).parse(self, json)
    }
}

// 通过 Deref 暴露内部方法
impl Deref for Map {
    type Target = HashMap<String, Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// 通过 DerefMut 暴露内部方法
impl DerefMut for Map {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
