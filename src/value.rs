use crate::Map;

/// `Map`的指定值类型.
#[derive(Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    F64(f64),
    I64(i64),
    String(String),
    Vec(Vec<Value>),
    Object(Map),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::F64(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match *self {
            Value::I64(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match *self {
            Value::String(ref s) => Some(s),
            _ => None,
        }
    }

    pub fn as_vec(&self) -> Option<&[Value]> {
        match *self {
            Value::Vec(ref v) => Some(v),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Map> {
        match *self {
            Value::Object(ref o) => Some(o),
            _ => None,
        }
    }
}
