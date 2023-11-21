use std::env;
use std::io;
use std::process;
use std::process::ExitCode;

// Usage: echo <input_text> | your_grep.sh -E <pattern>
fn main() -> ExitCode {
    if env::args().nth(1).unwrap() != "-E" {
        println!("Expected first argument to be '-E'");
        process::exit(1);
    }

    let pattern = env::args().nth(2).unwrap();
    let mut input_line = String::new();

    io::stdin().read_line(&mut input_line).unwrap();

    let grep = Grep {
        pattern,
        input: input_line,
    };
    if grep.is_match() {
        ExitCode::from(0)
    } else {
        ExitCode::from(1)
    }
}

struct Grep {
    pattern: String,
    input: String,
}

impl Grep {
    fn is_match(&self) -> bool {
        if self.pattern.chars().count() == 1 {
            self.input.contains(&self.pattern)
        } else if self.pattern == "\\d" {
            self.input.chars().any(|c| c.is_digit(10))
        } else if self.pattern == "\\w" {
            self.input.chars().any(|c| c.is_alphanumeric())
        } else if self.pattern == "\\s" {
            self.input.chars().any(|c| c.is_whitespace())
        } else if self.pattern.starts_with("[") {
            self.pattern
                .chars()
                .skip(1)
                .take_while(|c| *c != ']')
                .any(|c| self.input.contains(c))
        }
        else {
            panic!("Unhandled pattern: {}", self.pattern)
        }
    }
}
