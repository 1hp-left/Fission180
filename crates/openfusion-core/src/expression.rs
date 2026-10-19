//! Independent dimensional expression evaluator. Document lengths are millimetres.
use std::collections::{BTreeMap, BTreeSet};

pub type Parameters = BTreeMap<String, String>;
#[derive(Debug, Clone, PartialEq)]
pub enum InputState {
    Incomplete,
    Valid(Evaluated),
    Error(String),
}
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluated {
    pub millimetres: f64,
    pub dependencies: BTreeSet<String>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct DraftInput {
    pub text: String,
    pub state: InputState,
    pub last_valid: Option<Evaluated>,
}
impl DraftInput {
    pub fn new(text: String, parameters: &Parameters) -> Self {
        let mut input = Self {
            text: String::new(),
            state: InputState::Incomplete,
            last_valid: None,
        };
        input.update(text, parameters);
        input
    }
    pub fn update(&mut self, text: String, parameters: &Parameters) {
        self.state = evaluate(&text, parameters);
        if let InputState::Valid(value) = &self.state {
            self.last_valid = Some(value.clone());
        }
        self.text = text;
    }
}
#[derive(Debug)]
enum Failure {
    Incomplete,
    Invalid(String),
}
type Parsed<T> = std::result::Result<T, Failure>;
fn bad<T>(message: &str) -> Parsed<T> {
    Err(Failure::Invalid(message.into()))
}
#[derive(Clone, Copy)]
struct Quantity {
    value: f64,
    length_power: i32,
}
struct Parser<'a> {
    text: &'a str,
    offset: usize,
    parameters: &'a Parameters,
    stack: Vec<String>,
    dependencies: BTreeSet<String>,
    depth: usize,
}
impl<'a> Parser<'a> {
    fn space(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }
    fn peek(&mut self) -> Option<u8> {
        self.space();
        self.text.as_bytes().get(self.offset).copied()
    }
    fn expression(&mut self) -> Parsed<Quantity> {
        self.depth += 1;
        if self.depth > 64 {
            return bad("Expression nesting limit exceeded");
        }
        let result = self.sum();
        self.depth -= 1;
        result
    }
    fn sum(&mut self) -> Parsed<Quantity> {
        let mut a = self.product()?;
        while let Some(op @ (b'+' | b'-')) = self.peek() {
            self.offset += 1;
            let mut b = self.product()?;
            // Bare input numbers use document millimetres in a length sum.
            if a.length_power == 0 && b.length_power == 1 {
                a.length_power = 1;
            }
            if b.length_power == 0 && a.length_power == 1 {
                b.length_power = 1;
            }
            if a.length_power != b.length_power {
                return bad("Cannot add quantities with different dimensions");
            }
            a.value += if op == b'+' { b.value } else { -b.value };
        }
        Ok(a)
    }
    fn product(&mut self) -> Parsed<Quantity> {
        let mut a = self.atom()?;
        while let Some(op @ (b'*' | b'/')) = self.peek() {
            self.offset += 1;
            let b = self.atom()?;
            if op == b'*' {
                a.value *= b.value;
                a.length_power += b.length_power;
            } else {
                if b.value == 0. {
                    return bad("Division by zero");
                }
                a.value /= b.value;
                a.length_power -= b.length_power;
            }
        }
        Ok(a)
    }
    fn atom(&mut self) -> Parsed<Quantity> {
        let Some(c) = self.peek() else {
            return Err(Failure::Incomplete);
        };
        if c == b'+' || c == b'-' {
            self.offset += 1;
            self.depth += 1;
            if self.depth > 64 {
                return bad("Expression nesting limit exceeded");
            }
            let mut value = self.atom()?;
            self.depth -= 1;
            if c == b'-' {
                value.value = -value.value;
            }
            return Ok(value);
        }
        let mut value = if c == b'(' {
            self.offset += 1;
            let value = self.expression()?;
            match self.peek() {
                Some(b')') => self.offset += 1,
                None => return Err(Failure::Incomplete),
                _ => return bad("Expected closing parenthesis"),
            }
            value
        } else if c.is_ascii_digit() || c == b'.' {
            let start = self.offset;
            while self
                .text
                .as_bytes()
                .get(self.offset)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
            {
                self.offset += 1;
            }
            if self
                .text
                .as_bytes()
                .get(self.offset)
                .is_some_and(|c| *c == b'e' || *c == b'E')
            {
                self.offset += 1;
                if self
                    .text
                    .as_bytes()
                    .get(self.offset)
                    .is_some_and(|c| *c == b'+' || *c == b'-')
                {
                    self.offset += 1;
                }
                let exponent_start = self.offset;
                while self
                    .text
                    .as_bytes()
                    .get(self.offset)
                    .is_some_and(u8::is_ascii_digit)
                {
                    self.offset += 1;
                }
                if exponent_start == self.offset {
                    return if self.offset == self.text.len() {
                        Err(Failure::Incomplete)
                    } else {
                        bad("Invalid exponent")
                    };
                }
            }
            let value = self.text[start..self.offset]
                .parse::<f64>()
                .map_err(|_| Failure::Invalid("Invalid number".into()))?;
            Quantity {
                value,
                length_power: 0,
            }
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let name = self.identifier();
            if self.stack.contains(&name) {
                return bad("Cyclic parameter dependency");
            }
            if self.stack.len() >= 32 {
                return bad("Parameter dependency limit exceeded");
            }
            let expression = self
                .parameters
                .get(&name)
                .ok_or_else(|| Failure::Invalid(format!("Unknown parameter: {name}")))?;
            let mut stack = self.stack.clone();
            stack.push(name.clone());
            let mut nested = Parser {
                text: expression,
                offset: 0,
                parameters: self.parameters,
                stack,
                dependencies: BTreeSet::new(),
                depth: 0,
            };
            let mut q = nested.expression()?;
            if nested.peek().is_some() {
                return bad("Unexpected parameter input");
            }
            if q.length_power == 0 {
                q.length_power = 1;
            }
            self.dependencies.insert(name);
            self.dependencies.extend(nested.dependencies);
            q
        } else {
            return bad("Expected a number, parameter or parenthesis");
        };
        if self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
            let unit = self.identifier();
            let scale = match unit.as_str() {
                "mm" => 1.,
                "cm" => 10.,
                "m" => 1000.,
                "in" => 25.4,
                "ft" => 304.8,
                _ => return bad("Unknown length unit"),
            };
            value.value *= scale;
            value.length_power += 1;
        }
        if !value.value.is_finite() {
            return bad("Expression is not finite");
        }
        Ok(value)
    }
    fn identifier(&mut self) -> String {
        self.space();
        let start = self.offset;
        while self
            .text
            .as_bytes()
            .get(self.offset)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            self.offset += 1;
        }
        self.text[start..self.offset].into()
    }
}
pub fn evaluate(text: &str, parameters: &Parameters) -> InputState {
    if text.len() > 4096 {
        return InputState::Error("Expression exceeds 4096 bytes".into());
    }
    let mut parser = Parser {
        text,
        offset: 0,
        parameters,
        stack: vec![],
        dependencies: BTreeSet::new(),
        depth: 0,
    };
    let result = (|| {
        let value = parser.expression()?;
        if parser.peek().is_some() {
            return bad("Unexpected input");
        }
        if !(0..=1).contains(&value.length_power) {
            return bad("Expected a length expression");
        }
        if !value.value.is_finite() || value.value.abs() > 1_000_000. {
            return bad("Length must be finite and within ±1,000,000 mm");
        }
        Ok(Evaluated {
            millimetres: value.value,
            dependencies: parser.dependencies,
        })
    })();
    match result {
        Ok(value) => InputState::Valid(value),
        Err(Failure::Incomplete) => InputState::Incomplete,
        Err(Failure::Invalid(e)) => InputState::Error(e),
    }
}
pub fn length(text: &str, parameters: &Parameters) -> crate::Result<f64> {
    match evaluate(text, parameters) {
        InputState::Valid(v) => Ok(v.millimetres),
        InputState::Incomplete => crate::invalid("Incomplete expression"),
        InputState::Error(e) => Err(crate::Error::Invalid(e)),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_parameters_dimensions_and_invalid_drafts() {
        let p = BTreeMap::from([
            ("width".into(), "1 in".into()),
            ("half".into(), "width / 2".into()),
        ]);
        for (s, expected) in [
            ("2 * 3 mm", 6.),
            ("1 in", 25.4),
            ("half + 2 mm", 14.7),
            ("(1 cm + 5 mm) / 3", 5.),
            ("-2e1 mm", -20.),
        ] {
            assert!((length(s, &p).unwrap() - expected).abs() < 1e-10, "{s}");
        }
        let mut draft = DraftInput::new("2 * 3 mm".into(), &p);
        draft.update("5 +".into(), &p);
        assert_eq!(draft.state, InputState::Incomplete);
        assert_eq!(draft.last_valid.as_ref().unwrap().millimetres, 6.);
        for s in ["1 / 0", "NaN", "2 mm * 3 mm", "1,5", "1e999", "1 + )"] {
            assert!(matches!(evaluate(s, &p), InputState::Error(_)), "{s}");
        }
        let cyclic = BTreeMap::from([("a".into(), "b".into()), ("b".into(), "a".into())]);
        assert!(length("a", &cyclic).is_err());
        let InputState::Valid(v) = evaluate("half", &p) else {
            panic!()
        };
        assert_eq!(
            v.dependencies,
            BTreeSet::from(["half".into(), "width".into()])
        );
    }
}
