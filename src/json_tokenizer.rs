use crate::json_token::JsonToken;
use std::str::Chars;

/// 简单但严格的JSON标记器, 严格遵循RFC 7159.
///
/// 这个标记器是有状态的, 并且只返回"有用的"标记-名称, 值等.
///
/// 它不会为名称和值之间的分隔符或值之间的逗号创建标记. 它在令牌流运行时对其进行验证——因此调用者可以假设它生成
/// 的令牌是合适的. 例如, 它永远不会产生"开始对象, 结束数组".
///
/// 实现细节: 基类处理单个令牌推回, 但不是线程安全的.
pub struct JsonTokenizer<'a> {
    buffered_token: Vec<JsonToken>,

    // 返回堆栈深度，纯对象(不是集合).
    // 非正式地, 这是我们拥有的剩余未关闭的"{"字符的数量.
    pub object_depth: usize,
    proxy: JsonTextTokenizer<'a>,
}

impl<'a> JsonTokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        JsonTokenizer {
            buffered_token: Vec::with_capacity(1),
            object_depth: 0,
            proxy: JsonTextTokenizer::new(input),
        }
    }

    pub fn push_back(&mut self, token: JsonToken) -> Result<(), String> {
        if !self.buffered_token.is_empty() {
            return Err(String::from("Can't push back twice"));
        }

        if token == JsonToken::StartObject {
            self.object_depth -= 1;
        } else if token == JsonToken::EndObject {
            self.object_depth += 1;
        }
        self.buffered_token.push(token);

        Ok(())
    }

    // 返回流中的下一个JSON标记. 返回一个EndDocument标记来表示流的结束,
    // 在此点之后Next()不应该再被调用.
    //
    // 此实现提供单令牌缓冲, 如果没有缓冲令牌, 则调用next_impl().
    // 流中的下一个标记. 它永远不会为空.
    pub fn next(&mut self) -> Result<JsonToken, String> {
        let token_to_return: JsonToken;
        if !self.buffered_token.is_empty() {
            token_to_return = self.buffered_token.pop().unwrap();
        } else {
            token_to_return = self.proxy.next_impl()?;
        }

        if token_to_return == JsonToken::StartObject {
            self.object_depth += 1;
        } else if token_to_return == JsonToken::EndObject {
            self.object_depth -= 1;
        }

        Ok(token_to_return)
    }

    // 跳过将要读取的值. 这只能在读取属性名称后立即调用.
    // 如果该值是对象或数组, 则跳过完整的对象/数组.
    // 在找不到对应的key且忽略该key时才会用到该方法.
    #[allow(dead_code)]
    fn skip_value(&mut self) -> Result<(), String> {
        // 我们假设next()确保结束对象和结束数组都是有效的.
        // 我们只关心需要关闭的总嵌套深度.
        let mut depth = 0;

        loop {
            let token = self.next()?;
            match token {
                JsonToken::EndArray | JsonToken::EndObject => {
                    depth -= 1;
                }
                JsonToken::StartArray | JsonToken::StartObject => {
                    depth += 1;
                }
                _ => {}
            }

            if depth == 0 {
                break;
            }
        }

        Ok(())
    }
}

/// Tokenizer, 它完成了解析JSON的所有*真正*工作.
struct JsonTextTokenizer<'a> {
    container_stack: Vec<ContainerType>,
    reader: PushBackReader<'a>,
    state: i32,
}

impl<'a> JsonTextTokenizer<'a> {
    const VALUE_STATES: i32 = State::ARRAY_START
        | State::ARRAY_AFTER_COMMA
        | State::OBJECT_AFTER_COLON
        | State::START_OF_DOCUMENT;

    fn new(input: &'a str) -> Self {
        let mut container_stack = Vec::new();
        container_stack.push(ContainerType::Document);

        let reader = PushBackReader::new(input);
        let state = State::START_OF_DOCUMENT;
        JsonTextTokenizer {
            container_stack,
            reader,
            state,
        }
    }

    // 这个方法本质上只是循环通过字符跳过空白, 验证和改变状态(例如, 从ObjectBeforeColon到ObjectAfterColon),
    // 直到它到达一个真正的令牌(例如, 一个开始对象, 或一个值), 在这一点上它返回令牌. 虽然这个方法很大, 但要进一步分
    // 解它相对来说比较困难...其中大部分是大型switch语句, 它有时返回, 有时不返回.
    fn next_impl(&mut self) -> Result<JsonToken, String> {
        if self.state == State::READER_EXHAUSTED {
            return Err(String::from("Next() called after end of document"));
        }

        loop {
            let next = self.reader.read_char();
            if None == next {
                self.validate_state(
                    State::EXPECTED_END_OF_DOCUMENT,
                    "Unexpected end of document in state: ",
                )?;
                self.state = State::READER_EXHAUSTED;
                return Ok(JsonToken::EndDocument);
            }

            match next {
                // Skip whitespace between tokens
                Some(' ') | Some('\t') | Some('\r') | Some('\n') => continue,
                Some(':') => {
                    self.validate_state(State::OBJECT_BEFORE_COLON, "Invalid state to read a colon: ")?;
                    self.state = State::OBJECT_AFTER_COLON;
                }
                Some(',') => {
                    self.validate_state(State::OBJECT_AFTER_PROPERTY | State::ARRAY_AFTER_VALUE, "Invalid state to read a comma: ")?;
                    self.state = if self.state == State::OBJECT_AFTER_PROPERTY { State::OBJECT_AFTER_COMMA } else { State::ARRAY_AFTER_COMMA }
                }
                Some('"') => {
                    let string_value = self.read_string()?;
                    return if (self.state & (State::OBJECT_START | State::OBJECT_AFTER_COMMA)) != 0 {
                        self.state = State::OBJECT_BEFORE_COLON;
                        Ok(JsonToken::Name(string_value))
                    } else {
                        self.validate_and_modify_state_for_value("Invalid state to read a double quote: ")?;
                        Ok(JsonToken::StringValue(string_value))
                    };
                }
                Some('{') => {
                    self.validate_state(Self::VALUE_STATES, "Invalid state to read an open brace: ")?;
                    self.state = State::OBJECT_START;
                    self.container_stack.push(ContainerType::Object);
                    return Ok(JsonToken::StartObject);
                }
                Some('}') => {
                    self.validate_state(State::OBJECT_AFTER_PROPERTY | State::OBJECT_START, "Invalid state to read a close brace: ")?;
                    self.pop_container();
                    return Ok(JsonToken::EndObject);
                }
                Some('[') => {
                    self.validate_state(Self::VALUE_STATES, "Invalid state to read an open square bracket: ")?;
                    self.state = State::ARRAY_START;
                    self.container_stack.push(ContainerType::Array);
                    return Ok(JsonToken::StartArray);
                }
                Some(']') => {
                    self.validate_state(State::ARRAY_AFTER_VALUE | State::ARRAY_START, "Invalid state to read a close square bracket: ")?;
                    self.pop_container();
                    return Ok(JsonToken::EndArray);
                }
                Some('n') => { // Start of null
                    self.consume_literal("null")?;
                    self.validate_and_modify_state_for_value("Invalid state to read a null literal: ")?;
                    return Ok(JsonToken::Null);
                }
                Some('t') => { // Start of true
                    self.consume_literal("true")?;
                    self.validate_and_modify_state_for_value("Invalid state to read a true literal: ")?;
                    return Ok(JsonToken::True);
                }
                Some('f') => { // Start of false
                    self.consume_literal("false")?;
                    self.validate_and_modify_state_for_value("Invalid state to read a false literal: ")?;
                    return Ok(JsonToken::False);
                }
                Some('-') /* Start of a number*/ | Some('0') | Some('1') | Some('2') | Some('3') | Some('4') | Some('5') | Some('6') | Some('7') | Some('8') | Some('9') => {
                    let number = self.read_number(next.unwrap())?;
                    self.validate_and_modify_state_for_value("Invalid state to read a number token: ")?;
                    return Ok(JsonToken::Number(number));
                }
                _ => return Err(format!("Invalid first character of token: {:?}", next)),
            }
        }
    }

    fn validate_state(&self, valid_state: i32, error_prefix: &str) -> Result<(), String> {
        if valid_state & self.state == 0 {
            Err(format!("{}{:?}", error_prefix, State::name(self.state)))
        } else {
            Ok(())
        }
    }

    // 读取字符串标记. 假设开头 " 已经被读过了.
    fn read_string(&mut self) -> Result<String, String> {
        let mut val = String::new();

        loop {
            let mut c = self
                .reader
                .read_char()
                .ok_or(String::from("Unexpected end of text while reading string"))?;
            if c < ' ' {
                return Err(format!(
                    "Invalid character in string literal: U+{:04X}",
                    c as u32
                ));
            }

            if c == '"' {
                return Ok(val);
            }

            if c == '\\' {
                c = self.read_escaped_character()?;
            }

            val.push(c);
        }
    }

    // 读取转义字符. 假设前面的反斜杠已经被读取.
    fn read_escaped_character(&mut self) -> Result<char, String> {
        let c = self.reader.read_char().ok_or(String::from(
            "Unexpected end of text while reading character escape sequence",
        ))?;
        match c {
            'n' => Ok('\n'),
            '\\' => Ok('\\'),
            'b' => Ok('\x08'), // \b
            'f' => Ok('\x0C'), // \f
            'r' => Ok('\r'),
            't' => Ok('\t'),
            '"' => Ok('"'),
            '/' => Ok('/'),
            'u' => self.read_unicode_escape(),
            _ => Err(format!(
                "Invalid character in character escape sequence: U+{:04X}",
                c as u32
            )),
        }
    }

    // 读取转义的Unicode 4-nybble十六进制序列. 假设前面的\u已经被读取.
    fn read_unicode_escape(&mut self) -> Result<char, String> {
        let mut result = 0;
        for _ in 0..4 {
            let c = self.reader.read_char().ok_or(String::from(
                "Unexpected end of text while reading Unicode escape sequence",
            ))?;
            let nybble = if c >= '0' && c <= '9' {
                c as u32 - '0' as u32
            } else if c >= 'a' && c <= 'f' {
                c as u32 - 'a' as u32 + 10
            } else if c >= 'A' && c <= 'F' {
                c as u32 - 'A' as u32 + 10
            } else {
                return Err(format!(
                    "Invalid character in escape sequence: U+{:04X}",
                    c as u32
                ));
            };

            result = (result << 4) + nybble as i32;
        }

        Ok(result as u8 as char)
    }

    // 消耗一个纯文本字面量, 如果读取的文本与之不匹配, 则抛出异常. 假定文本的第一个字母已经被读取.
    fn consume_literal(&mut self, text: &str) -> Result<(), String> {
        let mut chars = text.chars();
        chars.next(); // Skip the first
        while let Some(c) = chars.next() {
            let next = self.reader.read_char().ok_or(format!(
                "Unexpected end of text while reading literal token {}",
                text
            ))?;
            if next != c {
                return Err(format!(
                    "Unexpected character while reading literal token {}",
                    text
                ));
            }
        }

        Ok(())
    }

    fn read_number(&mut self, initial_character: char) -> Result<String, String> {
        let mut builder = String::new();
        if initial_character == '-' {
            builder.push('-');
        } else {
            self.reader.push_back(initial_character)?;
        }

        // 每个方法返回它读取的不属于该部分的字符,
        // 这样我们就知道下一步该做什么, 包括在最后把字符推回去.
        // "end of text"返回null.
        let mut next_char = self.read_int(&mut builder)?;
        if let Some(val) = next_char {
            if val == '.' {
                next_char = self.read_frac(&mut builder)?;
            }
        }

        if let Some(val) = next_char {
            if val == 'e' || val == 'E' {
                next_char = self.read_exp(&mut builder)?;
            }
        }

        // 如果读取的字符不是数字的一部分, 则将其推回, 以便再次读取以解析下一个标记.
        if let Some(val) = next_char {
            self.reader.push_back(val)?;
        }

        Ok(builder)
    }

    fn read_int(&mut self, builder: &mut String) -> Result<Option<char>, String> {
        let first = self.reader.read_char();
        match first {
            None => Err(String::from("Invalid numeric literal")),
            Some(val) => {
                if val < '0' || val > '9' {
                    return Err(String::from("Invalid numeric literal"));
                }

                builder.push(val);
                let result = self.consume_digits(builder);
                if val == '0' && !result.1 {
                    Err(String::from(
                        "Invalid numeric literal: leading 0 for non-zero value.",
                    ))
                } else {
                    Ok(result.0)
                }
            }
        }
    }

    fn read_frac(&mut self, builder: &mut String) -> Result<Option<char>, String> {
        builder.push('.'); // Already consumed this

        let result = self.consume_digits(builder);
        if result.1 {
            Err(String::from(
                "Invalid numeric literal: fraction with no trailing digits",
            ))
        } else {
            Ok(result.0)
        }
    }

    fn read_exp(&mut self, builder: &mut String) -> Result<Option<char>, String> {
        builder.push('E'); // Already consumed this (or 'e')
        let next = self.reader.read_char();
        match next {
            None => Err(String::from(
                "Invalid numeric literal: exponent with no trailing digits",
            )),
            Some(val) => {
                if val == '-' || val == '+' {
                    builder.push(val);
                } else {
                    self.reader.push_back(val)?;
                }

                let result = self.consume_digits(builder);
                if result.1 {
                    Err(String::from(
                        "Invalid numeric literal: exponent without value",
                    ))
                } else {
                    Ok(result.0)
                }
            }
        }
    }

    fn consume_digits(&mut self, builder: &mut String) -> (Option<char>, bool) {
        let mut count: usize = 0;
        loop {
            let next = self.reader.read_char();

            match next {
                Some(val) => {
                    if val < '0' || val > '9' {
                        return (next, count == 0);
                    } else {
                        count += 1;
                        builder.push(val);
                    }
                }
                None => return (next, count == 0),
            }
        }
    }

    // 验证我们是否处于读取值的有效状态(必要时使用给定的错误前缀), 并将状态更改为适当的状态,
    // 例如将ObjectAfterColon更改为ObjectAfterProperty.
    fn validate_and_modify_state_for_value(&mut self, error_prefix: &str) -> Result<(), String> {
        self.validate_state(Self::VALUE_STATES, error_prefix)?;

        match self.state {
            State::START_OF_DOCUMENT => {
                self.state = State::EXPECTED_END_OF_DOCUMENT;
            }
            State::OBJECT_AFTER_COLON => {
                self.state = State::OBJECT_AFTER_PROPERTY;
            }
            State::ARRAY_START | State::ARRAY_AFTER_COMMA => {
                self.state = State::ARRAY_AFTER_VALUE;
            }
            _ => {
                return Err(String::from(
                    "ValidateAndModifyStateForValue does not handle all value states (and should)",
                ));
            }
        }
        Ok(())
    }

    fn pop_container(&mut self) {
        self.container_stack.pop();
        let parent = self.container_stack.last();
        if let Some(val) = parent {
            self.state = match val {
                ContainerType::Object => State::OBJECT_AFTER_PROPERTY,
                ContainerType::Array => State::ARRAY_AFTER_VALUE,
                ContainerType::Document => State::EXPECTED_END_OF_DOCUMENT,
            };
        }
    }
}

#[derive(Debug)]
enum ContainerType {
    Document,
    Object,
    Array,
}

struct State;

impl State {
    // ^ { "foo": "bar" }
    // 在文档中的值之前. 下一个状态: ObjectStart, ArrayStart, "AfterValue"
    const START_OF_DOCUMENT: i32 = 1 << 0;

    // { "foo": "bar" } ^
    // 在文档中的值之后. 下一个状态: ReaderExhausted
    const EXPECTED_END_OF_DOCUMENT: i32 = 1 << 1;

    // { "foo": "bar" } ^ (已经读到最后了)
    // 终端状态.
    const READER_EXHAUSTED: i32 = 1 << 2;

    // { ^ "foo": "bar" }
    // 在对象的*第一个*属性之前。
    // 下一个状态:
    // "AfterValue" (空对象)
    // ObjectBeforeColon (读一个名字)
    const OBJECT_START: i32 = 1 << 3;

    // { "foo" ^ : "bar", "x": "y" }
    // 下一个状态: ObjectAfterColon
    const OBJECT_BEFORE_COLON: i32 = 1 << 4;

    // { "foo" : ^ "bar", "x": "y" }
    // 在对象中除第一个属性之外的任何属性之前.
    // (等价地: 在对象的任何属性之后)
    // 下一个状态:
    // "AfterValue" (value is simple)
    // ObjectStart (value is object)
    // ArrayStart (value is array)
    const OBJECT_AFTER_COLON: i32 = 1 << 5;

    // { "foo" : "bar" ^ , "x" : "y" }
    // 在属性的末尾，因此期望逗号或对象末尾.
    // 下一个状态: ObjectAfterComma or "AfterValue"
    const OBJECT_AFTER_PROPERTY: i32 = 1 << 6;

    // { "foo":"bar", ^ "x":"y" }
    // 读取前一个属性后面的逗号，因此期望另一个属性.
    // 这类似于ObjectStart, 但右括号在这里无效.
    // 下一个状态: ObjectBeforeColon.
    const OBJECT_AFTER_COMMA: i32 = 1 << 7;

    // [ ^ "foo", "bar" ]
    // 在数组中的*第一个*值之前.
    // 下一个状态:
    // "AfterValue" (读取一个值)
    // "AfterValue" (数组结束；将弹出堆栈)
    const ARRAY_START: i32 = 1 << 8;

    // [ "foo" ^ , "bar" ]
    // 在数组中的任何值之后，因此期望逗号或数组结束.
    // 下一个状态: ArrayAfterComma or "AfterValue"
    const ARRAY_AFTER_VALUE: i32 = 1 << 9;

    // [ "foo", ^ "bar" ]
    // 在数组中的逗号之后, 因此*必须*有另一个值(简单或复杂).
    // 下一个状态: "AfterValue" (simple value), StartObject, StartArray
    const ARRAY_AFTER_COMMA: i32 = 1 << 10;

    fn name(val: i32) -> &'static str {
        match val {
            Self::START_OF_DOCUMENT => "START_OF_DOCUMENT",
            Self::EXPECTED_END_OF_DOCUMENT => "EXPECTED_END_OF_DOCUMENT",
            Self::READER_EXHAUSTED => "READER_EXHAUSTED",
            Self::OBJECT_START => "OBJECT_START",
            Self::OBJECT_BEFORE_COLON => "OBJECT_BEFORE_COLON",
            Self::OBJECT_AFTER_COLON => "OBJECT_AFTER_COLON",
            Self::OBJECT_AFTER_PROPERTY => "OBJECT_AFTER_PROPERTY",
            Self::OBJECT_AFTER_COMMA => "OBJECT_AFTER_COMMA",
            Self::ARRAY_START => "ARRAY_START",
            Self::ARRAY_AFTER_VALUE => "ARRAY_AFTER_VALUE",
            Self::ARRAY_AFTER_COMMA => "ARRAY_AFTER_COMMA",
            _ => "UnKnow State",
        }
    }
}

struct PushBackReader<'a> {
    chars: Chars<'a>,
    next_char: Option<char>,
}

impl<'a> PushBackReader<'a> {
    fn new(input: &'a str) -> Self {
        PushBackReader {
            chars: input.chars(),
            next_char: None,
        }
    }

    // 返回迭代器中的下一个字符, 如果已到达末尾则返回None.
    fn read_char(&mut self) -> Option<char> {
        if self.next_char != None {
            let tmp = self.next_char;
            self.next_char = None;
            return tmp;
        }

        self.chars.next()
    }

    fn push_back(&mut self, c: char) -> Result<(), String> {
        match self.next_char {
            Some(_) => Err(String::from(
                "Cannot push back when already buffering a character",
            )),
            None => {
                self.next_char = Some(c);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::json_token::JsonToken;
    use crate::json_tokenizer::JsonTokenizer;

    #[test]
    fn empty_object_value() {
        assert_tokens("{}", &[JsonToken::StartObject, JsonToken::EndObject]);
    }

    #[test]
    fn empty_array_value() {
        assert_tokens("[]", &[JsonToken::StartArray, JsonToken::EndArray]);
    }

    #[test]
    fn string_value() {
        let case1 = ("foo", "foo");
        let case2 = ("tab\\t", "tab\t");
        let case3 = ("line\\nfeed", "line\nfeed");
        let case4 = ("carriage\\rreturn", "carriage\rreturn");
        let case5 = ("back\\bspace", "back\x08space");
        let case6 = ("form\\ffeed", "form\x0Cfeed");
        let case7 = ("escaped\\/slash", "escaped/slash");
        let case8 = ("escaped\\\\backslash", "escaped\\backslash");
        let case9 = ("escaped\\\"quote", "escaped\"quote");
        let case10 = ("foo {}[] bar", "foo {}[] bar");
        let case11 = ("fooযbar", "foo\u{09af}bar"); // Digits, upper hex, lower hex
        let case12 = ("ab𐀀cd", "ab\u{10000}cd");
        let case13 = ("ab\u{10000}cd", "ab𐀀cd");

        assert_tokens_no_replacement(
            warp_quotes(case1.0).as_str(),
            &[JsonToken::StringValue(String::from(case1.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case2.0).as_str(),
            &[JsonToken::StringValue(String::from(case2.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case3.0).as_str(),
            &[JsonToken::StringValue(String::from(case3.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case4.0).as_str(),
            &[JsonToken::StringValue(String::from(case4.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case5.0).as_str(),
            &[JsonToken::StringValue(String::from(case5.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case6.0).as_str(),
            &[JsonToken::StringValue(String::from(case6.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case7.0).as_str(),
            &[JsonToken::StringValue(String::from(case7.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case8.0).as_str(),
            &[JsonToken::StringValue(String::from(case8.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case9.0).as_str(),
            &[JsonToken::StringValue(String::from(case9.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case10.0).as_str(),
            &[JsonToken::StringValue(String::from(case10.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case11.0).as_str(),
            &[JsonToken::StringValue(String::from(case11.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case12.0).as_str(),
            &[JsonToken::StringValue(String::from(case12.1))],
        );
        assert_tokens_no_replacement(
            warp_quotes(case13.0).as_str(),
            &[JsonToken::StringValue(String::from(case13.1))],
        );
    }

    #[test]
    fn object_depth() {
        let json = "{ \"foo\": { \"x\": 1, \"y\": [ 0 ] } }";
        let mut tokenizer = JsonTokenizer::new(json);

        assert_eq!(tokenizer.object_depth, 0);
        assert_eq!(tokenizer.next().unwrap(), JsonToken::StartObject);
        assert_eq!(tokenizer.object_depth, 1);
        assert_eq!(
            tokenizer.next().unwrap(),
            JsonToken::Name(String::from("foo"))
        );
        assert_eq!(tokenizer.object_depth, 1);
        assert_eq!(tokenizer.next().unwrap(), JsonToken::StartObject);
        assert_eq!(tokenizer.object_depth, 2);
        assert_eq!(
            tokenizer.next().unwrap(),
            JsonToken::Name(String::from("x"))
        );
        assert_eq!(tokenizer.object_depth, 2);
        assert_eq!(
            tokenizer.next().unwrap(),
            JsonToken::Number(String::from("1"))
        );
        assert_eq!(tokenizer.object_depth, 2);
        assert_eq!(
            tokenizer.next().unwrap(),
            JsonToken::Name(String::from("y"))
        );
        assert_eq!(tokenizer.object_depth, 2);
        assert_eq!(tokenizer.next().unwrap(), JsonToken::StartArray);
        assert_eq!(tokenizer.object_depth, 2); // 数组的深度没有改变
        assert_eq!(
            tokenizer.next().unwrap(),
            JsonToken::Number(String::from("0"))
        );
        assert_eq!(tokenizer.object_depth, 2);
        assert_eq!(tokenizer.next().unwrap(), JsonToken::EndArray);
        assert_eq!(tokenizer.object_depth, 2);
        assert_eq!(tokenizer.next().unwrap(), JsonToken::EndObject);
        assert_eq!(tokenizer.object_depth, 1);
        assert_eq!(tokenizer.next().unwrap(), JsonToken::EndObject);
        assert_eq!(tokenizer.object_depth, 0);
        assert_eq!(tokenizer.next().unwrap(), JsonToken::EndDocument);
        assert_eq!(tokenizer.object_depth, 0);
    }

    #[test]
    fn object_depth_with_push_back() {
        let json = "{}";
        let mut tokenizer = JsonTokenizer::new(json);

        assert_eq!(tokenizer.object_depth, 0);
        let token = tokenizer.next().unwrap();
        assert_eq!(tokenizer.object_depth, 1);
        // 当我们推回"开始对象"时, 我们应该有效地回到之前的深度.
        tokenizer.push_back(token).unwrap();
        assert_eq!(tokenizer.object_depth, 0);
        // 再次读取相同的标记, 并返回深度1
        tokenizer.next().unwrap();
        assert_eq!(tokenizer.object_depth, 1);

        // 现在反过来看EndObject也是一样
        let token = tokenizer.next().unwrap();
        assert_eq!(tokenizer.object_depth, 0);
        tokenizer.push_back(token).unwrap();
        assert_eq!(tokenizer.object_depth, 1);
        tokenizer.next().unwrap();
        assert_eq!(tokenizer.object_depth, 0);
    }

    #[test]
    fn invalid_string_value() {
        let case1 = "embedded tab\t";
        let case2 = "embedded CR\r";
        let case3 = "embedded LF\n";
        let case4 = "embedded bell\u{0007}";
        let case5 = "bad escape\\a";
        let case6 = "incomplete escape\\";
        let case7 = "incomplete Unicode escape\\u{000}";
        let case8 = "invalid Unicode escape\\u{000}H";

        assert_error_after(warp_quotes(case1).as_str(), &[]);
        assert_error_after(warp_quotes(case2).as_str(), &[]);
        assert_error_after(warp_quotes(case3).as_str(), &[]);
        assert_error_after(warp_quotes(case4).as_str(), &[]);
        assert_error_after(warp_quotes(case5).as_str(), &[]);
        assert_error_after(warp_quotes(case6).as_str(), &[]);
        assert_error_after(warp_quotes(case7).as_str(), &[]);
        assert_error_after(warp_quotes(case8).as_str(), &[]);
    }

    #[test]
    fn number_value() {
        let case1 = ("0", "0");
        let case2 = ("-0", "0"); // 我们不区分正0和负0
        let case3 = ("1", "1");
        let case4 = ("-1", "-1");
        // 从现在开始, 假设前面的标志没问题...
        let case5 = ("1.125", "1.125");
        let case6 = ("1e5", "100000");
        let case7 = ("1E5", "100000");
        let case8 = ("1e+5", "100000");
        let case9 = ("1E-5", "0.00001");
        let case10 = ("   1   ", "1");

        assert_tokens(
            case1.0.parse::<i32>().unwrap().to_string().as_str(),
            &[JsonToken::Number(String::from(case1.1))],
        );
        assert_tokens(
            case2.0.parse::<i32>().unwrap().to_string().as_str(),
            &[JsonToken::Number(String::from(case2.1))],
        );
        assert_tokens(
            case3.0.parse::<i32>().unwrap().to_string().as_str(),
            &[JsonToken::Number(String::from(case3.1))],
        );
        assert_tokens(
            case4.0.parse::<i32>().unwrap().to_string().as_str(),
            &[JsonToken::Number(String::from(case4.1))],
        );
        assert_tokens(
            case5.0.parse::<f32>().unwrap().to_string().as_str(),
            &[JsonToken::Number(String::from(case5.1))],
        );
        assert_tokens(
            (case6.0.parse::<f32>().unwrap() as i32)
                .to_string()
                .as_str(),
            &[JsonToken::Number(String::from(case6.1))],
        );
        assert_tokens(
            (case7.0.parse::<f32>().unwrap() as i32)
                .to_string()
                .as_str(),
            &[JsonToken::Number(String::from(case7.1))],
        );
        assert_tokens(
            (case8.0.parse::<f32>().unwrap() as i32)
                .to_string()
                .as_str(),
            &[JsonToken::Number(String::from(case8.1))],
        );
        assert_tokens(
            case9.0.parse::<f32>().unwrap().to_string().as_str(),
            &[JsonToken::Number(String::from(case9.1))],
        );
        assert_tokens(
            case10.0.trim().parse::<i32>().unwrap().to_string().as_str(),
            &[JsonToken::Number(String::from(case10.1))],
        );
    }

    #[test]
    fn invalid_number_value() {
        let case1 = "00";
        let case2 = ".5";
        let case3 = "1.";
        let case4 = "1e";
        let case5 = "1e-";
        let case6 = "--";
        let case7 = "--1";
        let case8 = "-1.7977e308";
        let case9 = "1.7977e308";

        assert_error_after(case1, &[]);
        assert_error_after(case2, &[]);
        assert_error_after(case3, &[]);
        assert_error_after(case4, &[]);
        assert_error_after(case5, &[]);
        assert_error_after(case6, &[]);
        assert_error_after(case7, &[]);
        {
            assert_ok_after(case8, &[]);
            assert_eq!(case8.parse::<f64>().unwrap(), f64::NEG_INFINITY);
        }
        {
            assert_ok_after(case9, &[]);
            assert_eq!(case9.parse::<f64>().unwrap(), f64::INFINITY);
        }
    }

    #[test]
    fn invalid_literals() {
        let case1 = "nul";
        let case2 = "nothing";
        let case3 = "truth";
        let case4 = "fALSEhood";

        assert_error_after(case1, &[]);
        assert_error_after(case2, &[]);
        assert_error_after(case3, &[]);
        assert_error_after(case4, &[]);
    }

    #[test]
    fn null_value() {
        assert_tokens("null", &[JsonToken::Null]);
    }

    #[test]
    fn true_value() {
        assert_tokens("true", &[JsonToken::True]);
    }

    #[test]
    fn false_value() {
        assert_tokens("false", &[JsonToken::False]);
    }

    #[test]
    fn simple_object() {
        assert_tokens(
            "{'x': 'y'}",
            &[
                JsonToken::StartObject,                    //
                JsonToken::Name(String::from("x")),        //
                JsonToken::StringValue(String::from("y")), //
                JsonToken::EndObject,                      //
            ],
        );
    }

    #[test]
    fn invalid_structure() {
        let case1 = ("[10, 20", 3);
        let case2 = ("[10,", 2);
        let case3 = ("[10:20]", 2);
        let case4 = ("[", 1);
        let case5 = ("[,", 1);
        let case6 = ("{", 1);
        let case7 = ("{,", 1);
        let case8 = ("{[", 1);
        let case9 = ("{{", 1);
        let case10 = ("{0", 1);
        let case11 = ("{null", 1);
        let case12 = ("{false", 1);
        let case13 = ("{true", 1);
        let case14 = ("}", 0);
        let case15 = ("]", 0);
        let case16 = (",", 0);
        let case17 = ("'foo' 'bar'", 1);
        let case18 = (":", 0);
        let case19 = ("'foo", 0); // 不完整的字符串
        let case20 = ("{ 'foo' }", 2);
        let case21 = ("{ x:1", 1); // 属性名必须加引号
        let case22 = ("{]", 1);
        let case23 = ("[}", 1);
        let case24 = ("[1,", 2);
        let case25 = ("{'x':0]", 3);
        let case26 = ("{ 'foo': }", 2);
        let case27 = ("{ 'foo':'bar', }", 3);

        let assert_structure = |json: &str, expected_valid_tokens: i32| {
            let json = json.replace("\'", "\"");
            let mut tokenizer = JsonTokenizer::new(json.as_str());
            for _ in 0..expected_valid_tokens {
                assert!(
                    matches!(tokenizer.next(), Ok(_)),
                    "Expected an Ok, but got an Err"
                );
            }

            assert!(
                matches!(tokenizer.next(), Err(_)),
                "Expected an Err, but got an Ok"
            );
        };

        assert_structure(case1.0, case1.1);
        assert_structure(case2.0, case2.1);
        assert_structure(case3.0, case3.1);
        assert_structure(case4.0, case4.1);
        assert_structure(case5.0, case5.1);
        assert_structure(case6.0, case6.1);
        assert_structure(case7.0, case7.1);
        assert_structure(case8.0, case8.1);
        assert_structure(case9.0, case9.1);
        assert_structure(case10.0, case10.1);
        assert_structure(case11.0, case11.1);
        assert_structure(case12.0, case12.1);
        assert_structure(case13.0, case13.1);
        assert_structure(case14.0, case14.1);
        assert_structure(case15.0, case15.1);
        assert_structure(case16.0, case16.1);
        assert_structure(case17.0, case17.1);
        assert_structure(case18.0, case18.1);
        assert_structure(case19.0, case19.1);
        assert_structure(case20.0, case20.1);
        assert_structure(case21.0, case21.1);
        assert_structure(case22.0, case22.1);
        assert_structure(case23.0, case23.1);
        assert_structure(case24.0, case24.1);
        assert_structure(case25.0, case25.1);
        assert_structure(case26.0, case26.1);
        assert_structure(case27.0, case27.1);
    }

    #[test]
    fn array_mixed_type() {
        assert_tokens(
            "[1, 'foo', null, false, true, [2], {'x':'y' }]",
            &[
                JsonToken::StartArray,
                JsonToken::Number(String::from("1")),
                JsonToken::StringValue(String::from("foo")),
                JsonToken::Null,
                JsonToken::False,
                JsonToken::True,
                JsonToken::StartArray,
                JsonToken::Number(String::from("2")),
                JsonToken::EndArray,
                JsonToken::StartObject,
                JsonToken::Name(String::from("x")),
                JsonToken::StringValue(String::from("y")),
                JsonToken::EndObject,
                JsonToken::EndArray,
            ],
        );
    }

    #[test]
    fn object_mixed_type() {
        assert_tokens(
            "{'a': 1, 'b': 'bar', 'c': null, 'd': false, 'e': true, 'f': [2], 'g': {'x':'y' }}",
            &[
                JsonToken::StartObject,
                JsonToken::Name(String::from("a")),
                JsonToken::Number(String::from("1")),
                JsonToken::Name(String::from("b")),
                JsonToken::StringValue(String::from("bar")),
                JsonToken::Name(String::from("c")),
                JsonToken::Null,
                JsonToken::Name(String::from("d")),
                JsonToken::False,
                JsonToken::Name(String::from("e")),
                JsonToken::True,
                JsonToken::Name(String::from("f")),
                JsonToken::StartArray,
                JsonToken::Number(String::from("2")),
                JsonToken::EndArray,
                JsonToken::Name(String::from("g")),
                JsonToken::StartObject,
                JsonToken::Name(String::from("x")),
                JsonToken::StringValue(String::from("y")),
                JsonToken::EndObject,
                JsonToken::EndObject,
            ],
        );
    }

    #[test]
    fn next_after_end_document_error() {
        let mut tokenizer = JsonTokenizer::new("null");
        assert_eq!(tokenizer.next().unwrap(), JsonToken::Null);
        assert_eq!(tokenizer.next().unwrap(), JsonToken::EndDocument);
        assert!(
            matches!(tokenizer.next(), Err(_)),
            "Expected an Err, but got an Ok"
        );
    }

    #[test]
    fn can_push_back_end_document() {
        let mut tokenizer = JsonTokenizer::new("null");
        assert_eq!(tokenizer.next().unwrap(), JsonToken::Null);
        assert_eq!(tokenizer.next().unwrap(), JsonToken::EndDocument);
        tokenizer.push_back(JsonToken::EndDocument).unwrap();
        assert_eq!(tokenizer.next().unwrap(), JsonToken::EndDocument);
        assert!(
            matches!(tokenizer.next(), Err(_)),
            "Expected an Err, but got an Ok"
        );
    }

    #[test]
    fn skip_value() {
        let case1 = "{ 'skip': 0, 'next': 1";
        let case2 = "{ 'skip': [0, 1, 2], 'next': 1";
        let case3 = "{ 'skip': 'x', 'next': 1";
        let case4 = "{ 'skip': ['x', 'y'], 'next': 1";
        let case5 = "{ 'skip': {'a': 0}, 'next': 1";
        let case6 = "{ 'skip': {'a': [0, {'b':[]}]}, 'next': 1";

        let assert_skip = |json: &str| {
            let json = json.replace("\'", "\"");
            let mut tokenizer = JsonTokenizer::new(json.as_str());
            assert_eq!(tokenizer.next().unwrap(), JsonToken::StartObject);

            if let JsonToken::Name(val) = tokenizer.next().unwrap() {
                assert_eq!(val.as_str(), "skip");
            }
            tokenizer.skip_value().unwrap();
            if let JsonToken::Name(val) = tokenizer.next().unwrap() {
                assert_eq!(val.as_str(), "next");
            }
        };

        assert_skip(case1);
        assert_skip(case2);
        assert_skip(case3);
        assert_skip(case4);
        assert_skip(case5);
        assert_skip(case6);
    }

    fn warp_quotes(s: &str) -> String {
        let mut builder = String::new();
        builder.push('\"');
        builder.push_str(s);
        builder.push('\"');

        builder
    }

    // 断言指定的JSON被标记为给定的标记序列.
    // 所有的撇号首先被转换成双引号, 允许任何不需要检查实际撇号处理的测试在JSON中使用撇号,
    // 避免混乱的字符串文字转义. "end document"令牌没有在预期令牌列表中指定, 而是隐式的.
    fn assert_tokens(json: &str, expected_tokens: &[JsonToken]) {
        let json = json.replace("\'", "\"");
        assert_tokens_no_replacement(json.as_str(), expected_tokens);
    }

    // 断言指定的JSON被标记为给定的标记序列.
    // 与AssertTokens(&str, JsonToken[])不同, 这不会对指定的JSON执行任何字符替换, 并且应该在文本包含
    // 预计将*用作*撇号的撇号时使用. "end document"令牌没有在预期令牌列表中指定, 而是隐式的.
    fn assert_tokens_no_replacement(json: &str, expected_tokens: &[JsonToken]) {
        let mut tokenizer = JsonTokenizer::new(json);
        for expected_token in expected_tokens {
            let actual_token = tokenizer.next().unwrap();
            if actual_token == JsonToken::EndDocument {
                panic!(
                    "Expected {:?} but reached end of token stream",
                    expected_token
                );
            }

            assert_eq!(actual_token, *expected_token);
        }

        let final_token = tokenizer.next().unwrap();
        if final_token != JsonToken::EndDocument {
            panic!(
                "Expected token stream to be exhausted; received {:?}",
                final_token
            );
        }
    }

    fn assert_error_after(json: &str, expected_tokens: &[JsonToken]) {
        let mut tokenizer = JsonTokenizer::new(json);
        for expected_token in expected_tokens {
            let actual_token = tokenizer.next().unwrap();
            if actual_token == JsonToken::EndDocument {
                panic!(
                    "Expected {:?} but reached end of token stream",
                    expected_token
                );
            }

            assert_eq!(actual_token, *expected_token);
        }

        assert!(
            matches!(tokenizer.next(), Err(_)),
            "Expected an Err, but got an Ok"
        );
    }

    fn assert_ok_after(json: &str, expected_tokens: &[JsonToken]) {
        let mut tokenizer = JsonTokenizer::new(json);
        for expected_token in expected_tokens {
            let actual_token = tokenizer.next().unwrap();
            if actual_token == JsonToken::EndDocument {
                panic!(
                    "Expected {:?} but reached end of token stream",
                    expected_token
                );
            }

            assert_eq!(actual_token, *expected_token);
        }

        assert!(
            matches!(tokenizer.next(), Ok(_)),
            "Expected an Ok, but got an Err"
        );
    }
}
