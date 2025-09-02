use crate::{Map, Value};

/// 将`Map`转换成Json的转换器.
pub struct JsonWriter {
    settings: JsonWriterSettings,
}

impl JsonWriter {
    const NAME_VALUE_SEPARATOR: &'static str = ":";
    const VALUE_SEPARATOR: &'static str = ",";
    const MULTILINE_VALUE_SEPARATOR: &'static str = ",";
    const STRUCT_OPEN_BRACKET: char = '{';
    const STRUCT_CLOSE_BRACKET: char = '}';
    const ARRAY_BRACKET_OPEN: char = '[';
    const ARRAY_BRACKET_CLOSE: char = ']';

    pub fn new(settings: JsonWriterSettings) -> Self {
        JsonWriter { settings }
    }

    pub fn format(&self, obj: &Map) -> String {
        let mut writer = String::new();
        self.write_struct(&mut writer, obj, 0);

        writer
    }

    fn write_struct(&self, writer: &mut String, obj: &Map, indentation_level: usize) {
        self.write_bracket_open(writer, Self::STRUCT_OPEN_BRACKET);
        let written_entries = self.write_struct_entries(writer, obj, false, indentation_level + 1);
        self.write_bracket_close(
            writer,
            Self::STRUCT_CLOSE_BRACKET,
            written_entries,
            indentation_level,
        );
    }

    fn write_struct_entries(
        &self,
        writer: &mut String,
        obj: &Map,
        assume_first_entry_written: bool,
        indentation_level: usize,
    ) -> bool {
        let mut first = !assume_first_entry_written;
        for (key, val) in obj.iter() {
            self.maybe_write_value_separator(writer, first);
            self.maybe_write_value_whitespace(writer, indentation_level);

            self.write_string(writer, key);

            self.write_name_value_separator(writer);

            self.write_value(writer, val, indentation_level);

            first = false;
        }

        !first
    }

    fn maybe_write_value_separator(&self, writer: &mut String, first: bool) {
        if first {
            return;
        }

        if self.settings.indentation == "" {
            writer.push_str(Self::VALUE_SEPARATOR);
        } else {
            writer.push_str(Self::MULTILINE_VALUE_SEPARATOR);
        }
    }

    fn write_name_value_separator(&self, writer: &mut String) {
        writer.push_str(Self::NAME_VALUE_SEPARATOR);

        if self.settings.indentation != INDENTATION_DEFAULT {
            writer.push(' ');
        }
    }

    fn write_null(&self, writer: &mut String) {
        writer.push_str("null");
    }

    fn write_bool(&self, writer: &mut String, val: bool) {
        let result = if val { "true" } else { "false" };
        writer.push_str(result);
    }

    fn write_f64(&self, writer: &mut String, val: f64) {
        writer.push_str(val.to_string().as_str());
    }

    fn write_i64(&self, writer: &mut String, val: i64) {
        writer.push_str(val.to_string().as_str());
    }

    fn write_value(&self, writer: &mut String, value: &Value, indentation_level: usize) {
        match *value {
            Value::Null => self.write_null(writer),
            Value::Bool(val) => self.write_bool(writer, val),
            Value::F64(val) => self.write_f64(writer, val),
            Value::I64(val) => self.write_i64(writer, val),
            Value::String(ref val) => self.write_string(writer, val),
            Value::Vec(ref val) => self.write_vec(writer, val, indentation_level),
            Value::Object(ref val) => self.write_struct(writer, val, indentation_level),
        }
    }

    fn write_vec(&self, writer: &mut String, vec: &Vec<Value>, indentation_level: usize) {
        self.write_bracket_open(writer, Self::ARRAY_BRACKET_OPEN);
        let mut first = true;
        for val in vec {
            self.maybe_write_value_separator(writer, first);
            self.maybe_write_value_whitespace(writer, indentation_level + 1);
            self.write_value(writer, val, indentation_level + 1);
            first = false;
        }

        self.write_bracket_close(writer, Self::ARRAY_BRACKET_CLOSE, !first, indentation_level);
    }

    // 将字符串(包括前导和尾双引号)写入构建器, 并根据需要进行转义.
    fn write_string(&self, writer: &mut String, text: &str) {
        writer.push('"');
        for c in text.chars() {
            match c {
                '"' => writer.push_str("\\\""),
                '\\' => writer.push_str("\\\\"),
                '\x08' => writer.push_str("\\b"),
                '\x0C' => writer.push_str("\\f"),
                '\n' => writer.push_str("\\n"),
                '\r' => writer.push_str("\\r"),
                '\t' => writer.push_str("\\t"),
                '/' => writer.push_str("\\/"),
                c if c.is_control() => {
                    writer.push_str(format!("\\u{:04x}", c as u32).as_str());
                }
                _ => writer.push(c),
            }
        }
        writer.push('"');
    }

    fn write_bracket_open(&self, writer: &mut String, open_char: char) {
        writer.push(open_char);
        if self.settings.indentation == INDENTATION_DEFAULT {
            writer.push_str("");
        }
    }

    fn write_bracket_close(
        &self,
        writer: &mut String,
        close_char: char,
        has_entries: bool,
        indentation_level: usize,
    ) {
        if has_entries {
            if self.settings.indentation != INDENTATION_DEFAULT {
                self.write_line(writer);
                self.write_indentation(writer, indentation_level);
            } else {
                writer.push_str("");
            }
        }

        writer.push(close_char);
    }

    fn maybe_write_value_whitespace(&self, writer: &mut String, indentation_level: usize) {
        if self.settings.indentation != INDENTATION_DEFAULT {
            self.write_line(writer);
            self.write_indentation(writer, indentation_level);
        }
    }

    fn write_indentation(&self, writer: &mut String, indentation_level: usize) {
        for _ in 0..indentation_level {
            writer.push_str(self.settings.indentation.as_str());
        }
    }

    fn write_line(&self, writer: &mut String) {
        if cfg!(target_os = "windows") {
            writer.push_str("\r\n");
        } else {
            writer.push_str("\n");
        }
    }
}

pub struct JsonWriterSettings {
    pub indentation: String,
}

const INDENTATION_DEFAULT: &str = "";

impl Default for JsonWriterSettings {
    fn default() -> Self {
        JsonWriterSettings {
            indentation: INDENTATION_DEFAULT.to_string(),
        }
    }
}
