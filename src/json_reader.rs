use crate::json_token::JsonToken;
use crate::json_tokenizer::JsonTokenizer;
use crate::{Map, Value};

/// 将Json转换成`Map`的转换器.
pub struct JsonReader {
    settings: JsonReaderSettings,
}

impl JsonReader {
    pub fn new(settings: JsonReaderSettings) -> Self {
        JsonReader { settings }
    }

    pub fn parse(&self, obj: &mut Map, json: &str) -> Result<(), String> {
        let mut tokenizer = JsonTokenizer::new(json);

        self.parse_object(obj, &mut tokenizer)?;

        let last_token = tokenizer.next()?;
        if last_token != JsonToken::EndDocument {
            Err("Expected end of JSON after object".to_string())
        } else {
            Ok(())
        }
    }

    fn parse_object(&self, obj: &mut Map, tokenizer: &mut JsonTokenizer) -> Result<(), String> {
        let mut token = tokenizer.next()?;
        if token != JsonToken::StartObject {
            return Err("Expected an object".to_string());
        }

        if tokenizer.object_depth > self.settings.recursion_limit {
            return Err(format!(
                "The set recursion depth is exceeded: {}",
                self.settings.recursion_limit
            ));
        }

        loop {
            token = tokenizer.next()?;
            if token == JsonToken::EndObject {
                return Ok(());
            }

            match token {
                JsonToken::Name(name) => {
                    let val = self.parse_value_type(obj, name.as_str(), tokenizer)?;
                    obj.insert(name, val);
                }
                _ => return Err(format!("Unexpected token type {:?}", token)),
            }
        }
    }

    fn parse_value_type(
        &self,
        obj: &mut Map,
        name: &str,
        tokenizer: &mut JsonTokenizer,
    ) -> Result<Value, String> {
        let token = tokenizer.next()?;
        if token == JsonToken::StartArray {
            let vec = self.parse_array(obj, name, tokenizer)?;
            Ok(vec)
        } else if token == JsonToken::StartObject {
            let mut nested_obj = Map::new();

            tokenizer.push_back(token)?;
            self.parse_object(&mut nested_obj, tokenizer)?;

            Ok(Value::Object(nested_obj))
        } else {
            let val = self.parse_single_value(&token)?;
            Ok(val)
        }
    }

    fn parse_array(
        &self,
        obj: &mut Map,
        name: &str,
        tokenizer: &mut JsonTokenizer,
    ) -> Result<Value, String> {
        let mut vec = Vec::<Value>::new();
        loop {
            let token = tokenizer.next()?;
            if token == JsonToken::EndArray {
                return Ok(Value::Vec(vec));
            }

            tokenizer.push_back(token)?;
            let val = self.parse_value_type(obj, name, tokenizer)?;
            vec.push(val);
        }
    }

    fn parse_single_value(&self, token: &JsonToken) -> Result<Value, String> {
        match token {
            JsonToken::Null => Ok(Value::Null),
            JsonToken::False => Ok(Value::Bool(false)),
            JsonToken::True => Ok(Value::Bool(true)),
            JsonToken::StringValue(s) => Ok(Value::String(s.to_string())),
            JsonToken::Number(num) => {
                let s = num.as_str();
                if s.parse::<i64>().is_ok() {
                    Ok(Value::I64(num.parse::<i64>().unwrap()))
                } else if s.parse::<f64>().is_ok() {
                    let value = self.safe_parse_f64(num.as_str())?;
                    Ok(Value::F64(value))
                } else {
                    Err(format!("Invalid number: {}", num))
                }
            }
            _ => Err(format!(
                "An error Token occurred while parsing single value: {:?}",
                token
            )),
        }
    }

    // 将字符串安全地转换成f64类型，如果转换失败则抛出错误信息.
    fn safe_parse_f64(&self, s: &str) -> Result<f64, String> {
        s.parse::<f64>()
            .map_err(|e| format!("Parse error: {}", e))
            .and_then(|num| {
                if num.is_nan() || num.is_infinite() {
                    Err("Reject special value".to_string())
                } else {
                    Ok(num)
                }
            })
    }
}

pub struct JsonReaderSettings {
    pub recursion_limit: usize, // 要分析的消息的最大深度.
}

impl Default for JsonReaderSettings {
    fn default() -> Self {
        JsonReaderSettings {
            recursion_limit: 100,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::json_reader::JsonReader;
    use crate::{JsonReaderSettings, Map};

    #[test]
    fn all_types_round_trip() {
        let json =
            r#"{"a":null,"b":false,"c":618,"d":"hello","e":[3.14,6.18],"f":{"a1":11,"b1":22}}"#;

        let map = parse_to_map(json);
        assert_eq!(map.len(), 6);

        assert!(map.get("a").unwrap().is_null());
        assert_eq!(map.get("b").unwrap().as_bool().unwrap(), false);
        assert_eq!(map.get("c").unwrap().as_i64().unwrap(), 618);
        assert_eq!(map.get("d").unwrap().as_string().unwrap(), "hello");
        let vec = map.get("e").unwrap().as_vec().unwrap();
        assert_eq!(vec.len(), 2);
        assert_eq!(vec[0].as_f64().unwrap(), 3.14);
        assert_eq!(vec[1].as_f64().unwrap(), 6.18);
        let obj = map.get("f").unwrap().as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert_eq!(obj.get("a1").unwrap().as_i64().unwrap(), 11);
        assert_eq!(obj.get("b1").unwrap().as_i64().unwrap(), 22);
    }

    #[test]
    fn nested_parse() {
        let json = r#"{"a1":[{"b1":true},{"b2":false}],"a2":"hi"}"#;

        let map = parse_to_map(json);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("a2").unwrap().as_string().unwrap(), "hi");

        let vec = map.get("a1").unwrap().as_vec().unwrap();
        assert_eq!(vec.len(), 2);
        assert_eq!(
            vec[0]
                .as_object()
                .unwrap()
                .get("b1")
                .unwrap()
                .as_bool()
                .unwrap(),
            true
        );
        assert_eq!(
            vec[1]
                .as_object()
                .unwrap()
                .get("b2")
                .unwrap()
                .as_bool()
                .unwrap(),
            false
        );
    }

    #[test]
    fn string_to_i64_valid() {
        let case1 = ("0", 0);
        let case2 = ("-0", 0);
        let case3 = ("1", 1);
        let case4 = ("-1", -1);
        let case5 = ("9223372036854775807", 9223372036854775807);
        let case6 = ("-9223372036854775808", -9223372036854775808);

        assert_string_to_i64_valid(case1.0, case1.1);
        assert_string_to_i64_valid(case2.0, case2.1);
        assert_string_to_i64_valid(case3.0, case3.1);
        assert_string_to_i64_valid(case4.0, case4.1);
        assert_string_to_i64_valid(case5.0, case5.1);
        assert_string_to_i64_valid(case6.0, case6.1);
    }

    #[test]
    fn string_to_f64_valid() {
        let case1 = ("1.0000000000000000000000001", 1.);
        let case2 = ("1e1", 10.);
        let case3 = ("1e01", 10.); // 指数中允许使用前导小数
        let case4 = ("1E1", 10.);
        let case5 = ("-1e1", -10.);
        let case6 = ("1.5e1", 15.);
        let case7 = ("-1.5e1", -15.);
        let case8 = ("15e-1", 1.5);
        let case9 = ("-15e-1", -1.5);
        let case10 = ("1.79769e308", 1.79769e308);
        let case11 = ("-1.79769e308", -1.79769e308);

        assert_string_to_f64_valid(case1.0, case1.1);
        assert_string_to_f64_valid(case2.0, case2.1);
        assert_string_to_f64_valid(case3.0, case3.1);
        assert_string_to_f64_valid(case4.0, case4.1);
        assert_string_to_f64_valid(case5.0, case5.1);
        assert_string_to_f64_valid(case6.0, case6.1);
        assert_string_to_f64_valid(case7.0, case7.1);
        assert_string_to_f64_valid(case8.0, case8.1);
        assert_string_to_f64_valid(case9.0, case9.1);
        assert_string_to_f64_valid(case10.0, case10.1);
        assert_string_to_f64_valid(case11.0, case11.1);
    }

    #[test]
    fn string_to_number_invalid() {
        let case1 = "+0";
        let case2 = "00";
        let case3 = "-00";
        let case4 = "+1";
        let case5 = "1e10111111";
        let case6 = "1.7977e308";
        let case7 = "-1.7977e308";
        let case8 = "1e309";

        assert_string_to_number_invalid(case1);
        assert_string_to_number_invalid(case2);
        assert_string_to_number_invalid(case3);
        assert_string_to_number_invalid(case4);
        assert_string_to_number_invalid(case5);
        assert_string_to_number_invalid(case6);
        assert_string_to_number_invalid(case7);
        assert_string_to_number_invalid(case8);
    }

    #[test]
    fn parse_number() {
        let json = r#"{"a1": 3.14, "a2": 789}"#;
        let map = parse_to_map(json);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("a1").unwrap().as_f64().unwrap(), 3.14f64);
        assert_eq!(map.get("a2").unwrap().as_i64().unwrap(), 789i64);
    }

    fn assert_string_to_f64_valid(left: &str, right: f64) {
        let json = format!("{{\"key_f64\":{}}}", left);
        let map = parse_to_map(json.as_str());
        assert_eq!(map.get("key_f64").unwrap().as_f64().unwrap(), right);
    }

    fn assert_string_to_i64_valid(left: &str, right: i64) {
        let json = format!("{{\"key_i64\":{}}}", left);
        let map = parse_to_map(json.as_str());
        assert_eq!(map.get("key_i64").unwrap().as_i64().unwrap(), right);
    }

    fn parse_to_map(json: &str) -> Map {
        let mut map = Map::new();
        JsonReader::new(JsonReaderSettings::default())
            .parse(&mut map, json)
            .unwrap();

        map
    }

    fn assert_string_to_number_invalid(s: &str) {
        let json = format!("{{\"key_number\":{}}}", s);
        let result = parse_to_map_err(json.as_str());

        assert!(matches!(result, Err(_)));
    }

    fn parse_to_map_err(json: &str) -> Result<(), String> {
        let mut map = Map::new();
        JsonReader::new(JsonReaderSettings::default()).parse(&mut map, json)
    }
}
