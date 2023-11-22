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
        let nodes = NodeParser {
            input: self.pattern.chars().collect(),
            index: 0,
        }
        .parse();

        println!("nodes: {:?}", nodes);

        let characters = self.input.chars().collect::<Vec<char>>();

        (0..characters.len()).any(|index| Grep::is_pattern_match(&nodes, &characters, index))
    }

    fn is_pattern_match(nodes: &Vec<NodeType>, characters: &Vec<char>, index: usize) -> bool {
        let mut current = index;
        let mut node_index = 0;
        loop {
            if node_index >= nodes.len() {
                return true;
            }
            if current >= characters.len() {
                return false;
            }

            match nodes[node_index] {
                NodeType::Literal(c) => {
                    if characters[current] != c {
                        return false;
                    }
                }
                NodeType::AnyDigit => {
                    if !characters[current].is_digit(10) {
                        return false;
                    }
                }
                NodeType::AnyWord => {
                    if !characters[current].is_alphanumeric() {
                        return false;
                    }
                }
                NodeType::AnyWhitespace => {
                    if !characters[current].is_whitespace() {
                        return false;
                    }
                }
                NodeType::AnyChar => {}
                NodeType::AnyCharIn(ref chars) => {
                    if !chars.contains(&characters[current]) {
                        return false;
                    }
                }
                NodeType::AnyCharNotIn(ref chars) => {
                    if chars.contains(&characters[current]) {
                        return false;
                    }
                }
            }
            current += 1;
            node_index += 1;
        }
    }
}

#[derive(Debug)]
enum NodeType {
    Literal(char),
    AnyDigit,
    AnyWord,
    AnyWhitespace,
    AnyChar,
    AnyCharIn(Vec<char>),
    AnyCharNotIn(Vec<char>),
}

struct NodeParser {
    input: Vec<char>,
    index: usize,
}

impl NodeParser {
    fn parse(mut self) -> Vec<NodeType> {
        let mut result = Vec::new();
        loop {
            if self.index >= self.input.len() {
                break;
            }

            let c = self.input[self.index];
            match c {
                '\\' => {
                    self.index += 1;
                    let c = self.input[self.index];
                    match c {
                        'd' => result.push(NodeType::AnyDigit),
                        'w' => result.push(NodeType::AnyWord),
                        's' => result.push(NodeType::AnyWhitespace),
                        'D' => result.push(NodeType::AnyCharNotIn("0123456789".chars().collect())),
                        'W' => result.push(NodeType::AnyCharNotIn(
                            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_"
                                .chars()
                                .collect(),
                        )),
                        'S' => result.push(NodeType::AnyCharNotIn(" \t\r\n".chars().collect())),
                        _ => result.push(NodeType::Literal(c)),
                    }
                }
                '.' => result.push(NodeType::AnyChar),
                '[' => {
                    self.index += 1;
                    let mut chars = Vec::new();
                    let mut is_not = false;
                    if self.input[self.index] == '^' {
                        is_not = true;
                        self.index += 1;
                    }
                    loop {
                        let c = self.input[self.index];
                        if c == ']' {
                            break;
                        }
                        chars.push(c);
                        self.index += 1;
                    }
                    if is_not {
                        result.push(NodeType::AnyCharNotIn(chars));
                    } else {
                        result.push(NodeType::AnyCharIn(chars));
                    }
                }
                _ => result.push(NodeType::Literal(c)),
            }
            self.index += 1;
        }

        result
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn test_grep(pattern: &str, input: &str, expected: bool) {
        let grep = Grep {
            pattern: pattern.to_string(),
            input: input.to_string(),
        };
        assert_eq!(grep.is_match(), expected, "pattern: {}, input: {}", pattern, input);
    }

    #[test]
    fn grep_literal_pattern() {
        test_grep("abc", "abc", true);
        test_grep("abc", "abcd", true);
        test_grep("abc", "ab", false);
        test_grep("abc", "abce", true);
        test_grep("abc", "uvwxyzabde", false);
    }

    #[test]
    fn grep_digit_pattern() {
        test_grep(r"\d", "1", true);
        test_grep(r"\d", "a", false);
        test_grep(r"\d", " ", false);
    }

    #[test]
    fn grep_sample_pattern() {
        test_grep(r"\d", "apple123" , true);
        test_grep(r"\w","alpha-num3ric",true);
        test_grep("[abc]", "apple", true);
        test_grep("[^abc]", "apple", true);
        test_grep(r"\d apple", "1 apple", true);
        test_grep(r"\d apple", "x apple", false);
    }

}
