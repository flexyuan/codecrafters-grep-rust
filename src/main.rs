use std::collections::HashMap;
use std::env;
use std::io;
use std::process;
use std::process::ExitCode;
use std::vec;

// Usage: echo <input_text> | your_grep.sh -E <pattern>

const SPECIAL_MARKER: char = '\x01';

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
        let chars: Vec<char> = self.pattern.chars().collect();
        let pattern_parser = PatternParser::new(&chars);
        let pattern = pattern_parser.parse();
        println!("pattern: {:?}", pattern);

        let modified_input = format!("{}{}{}", SPECIAL_MARKER, self.input, SPECIAL_MARKER);
        let modified_pattern = Pattern::Sequence(vec![
            Pattern::KleeneStar(Box::new(Pattern::AnyChar)),
            pattern,
            Pattern::KleeneStar(Box::new(Pattern::AnyChar)),
        ]);
        println!("modified_pattern: {:?}", modified_pattern);
        let mut nfa_builder = NfaBuilder::new();
        let nfa = nfa_builder.of(modified_pattern);
        println!("nfa: {:?}", nfa);
        let nfa_runner = NfaRunner::new(nfa);
        nfa_runner.run(&modified_input)
    }
}

#[derive(Debug)]
enum Pattern {
    Start,
    End,
    Literal(char),
    AnyDigit,
    AnyChar,
    AnyCharIn(Vec<char>),
    AnyCharNotIn(Vec<char>),
    OneOrMore(Box<Pattern>),
    KleeneStar(Box<Pattern>),
    Sequence(Vec<Pattern>),
    Or(Box<Pattern>, Box<Pattern>),
}

struct PatternParser<'a> {
    input: &'a [char],
    index: usize,
    patterns: Vec<Pattern>,
}

impl<'a> PatternParser<'a> {
    fn new(input: &'a [char]) -> PatternParser {
        PatternParser {
            input,
            index: 0,
            patterns: Vec::new(),
        }
    }

    fn parse(self) -> Pattern {
        let mut parser: PatternParser<'_> = self;
        parser.internal_parse()
    }

    fn internal_parse(&mut self) -> Pattern {
        while let Some(next) = self.next_pattern() {
            self.patterns.push(next);
        }
        if self.patterns.len() == 0 {
            return Pattern::AnyChar;
        } else if self.patterns.len() == 1 {
            self.patterns.pop().unwrap()
        } else {
            Pattern::Sequence(self.patterns.drain(..).collect())
        }
    }

    fn next_pattern(&mut self) -> Option<Pattern> {
        if self.index >= self.input.len() {
            return None;
        }
        let current = self.input[self.index];
        let next = match current {
            '\\' => {
                self.index += 1;
                let c = self.input[self.index];
                match c {
                    'd' => Pattern::AnyDigit,
                    'w' => Pattern::AnyCharIn(
                        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_"
                            .chars()
                            .collect(),
                    ),
                    's' => Pattern::AnyCharIn(" \t\r\n".chars().collect()),
                    'D' => Pattern::AnyCharNotIn("0123456789".chars().collect()),
                    'W' => Pattern::AnyCharNotIn(
                        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_"
                            .chars()
                            .collect(),
                    ),
                    'S' => Pattern::AnyCharNotIn(" \t\r\n".chars().collect()),
                    _ => Pattern::Literal(c),
                }
            }
            '.' => Pattern::AnyChar,
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
                    Pattern::AnyCharNotIn(chars)
                } else {
                    Pattern::AnyCharIn(chars)
                }
            }
            '(' => {
                self.index += 1;
                let next_close = self.next_index(b')' as char).expect("Expected ')'");
                let index = self.index;
                let pipe_index = self.next_index(b'|' as char);
                self.index = next_close;
                if let Some(pipe_index) = pipe_index {
                    let left = PatternParser::new(&self.input[index..pipe_index]).internal_parse();
                    let right = PatternParser::new(&self.input[pipe_index + 1..next_close])
                        .internal_parse();
                    Pattern::Or(Box::new(left), Box::new(right))
                } else {
                    PatternParser::new(&self.input[index..next_close]).internal_parse()
                }
            }
            '|' => {
                let left = self.patterns.pop().expect("Expected left pattern before |");
                self.index += 1;
                let right = PatternParser::new(&self.input[self.index..])
                    .next_pattern()
                    .expect("Expected right pattern after |");
                self.index -= 1;
                Pattern::Or(Box::new(left), Box::new(right))
            }
            '*' => {
                let left = self.patterns.pop().expect("Expected left pattern before *");
                Pattern::KleeneStar(Box::new(left))
            }
            '+' => {
                let left = self.patterns.pop().expect("Expected left pattern before +");
                Pattern::OneOrMore(Box::new(left))
            }
            '?' => {
                let left = self.patterns.pop().expect("Expected left pattern before ?");
                Pattern::Or(
                    Box::new(Pattern::Sequence(vec![])),
                    Box::new(Pattern::OneOrMore(Box::new(left))),
                )
            }
            '^' => Pattern::Start,
            '$' => Pattern::End,
            _ => Pattern::Literal(current),
        };
        self.index += 1;
        Some(next)
    }

    fn next_index(&self, c: char) -> Option<usize> {
        for i in self.index..self.input.len() {
            if self.input[i] == c {
                return Some(i);
            }
        }
        None
    }
}

type StateId = usize;

struct Nfa {
    start: StateId,
    end: Vec<StateId>,
    states: HashMap<StateId, NfaState>,
}

impl std::fmt::Debug for Nfa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_ids = self.states.keys().collect::<Vec<&StateId>>();
        let mut state_str = String::new();
        for state_id in state_ids {
            let state = self.states.get(state_id).unwrap();
            for (input, next_state) in state.transition.iter() {
                let input_str = match input {
                    StateInput::Literal(c) => format!("{}", c),
                    StateInput::AnyDigit => r"\d".to_string(),
                    StateInput::AnyChar => ".".to_string(),
                    StateInput::AnyCharIn(chars) => {
                        format!("[{}]", chars.iter().collect::<String>())
                    }
                    StateInput::AnyCharNotIn(chars) => {
                        format!("[^{}]", chars.iter().collect::<String>())
                    }
                    StateInput::Epsilon => "ε".to_string(),
                };
                state_str.push_str(&format!("{} -> {} -> {}", state_id, input_str, next_state));
                state_str.push('\n');
            }
        }
        write!(
            f,
            "Nfa {{ start: {}, end: {:?}, states: {} }}",
            self.start, self.end, state_str
        )
    }
}

#[derive(Debug)]
struct NfaState {
    id: StateId,
    transition: Vec<(StateInput, usize)>,
}

#[derive(Debug)]
enum StateInput {
    Literal(char),
    AnyDigit,
    AnyChar,
    AnyCharIn(Vec<char>),
    AnyCharNotIn(Vec<char>),
    Epsilon,
}

struct NfaBuilder {
    id_: usize,
}

impl NfaBuilder {
    fn new() -> NfaBuilder {
        NfaBuilder { id_: 0 }
    }

    fn of(&mut self, pattern: Pattern) -> Nfa {
        match pattern {
            Pattern::Literal(c) => self.literal(c),
            Pattern::AnyDigit => self.any_digit(),
            Pattern::AnyChar => self.any_char(),
            Pattern::AnyCharIn(chars) => self.any_char_in(chars),
            Pattern::AnyCharNotIn(chars) => self.any_char_not_in(chars),
            Pattern::OneOrMore(pattern) => {
                let inner = self.of(*pattern);
                self.one_or_more(inner)
            }
            Pattern::KleeneStar(pattern) => {
                let inner = self.of(*pattern);
                self.kleene_star(inner)
            }
            Pattern::Sequence(patterns) => self.sequence(patterns),
            Pattern::Or(left, right) => {
                let left = self.of(*left);
                let right = self.of(*right);
                self.or(left, right)
            }
            Pattern::Start => self.literal(SPECIAL_MARKER),
            Pattern::End => self.literal(SPECIAL_MARKER),
        }
    }

    fn next_id(&mut self) -> usize {
        self.id_ += 1;
        self.id_
    }

    fn literal(&mut self, c: char) -> Nfa {
        let end = NfaState {
            id: self.next_id(),
            transition: vec![],
        };
        let start = NfaState {
            id: self.next_id(),
            transition: vec![(StateInput::Literal(c), end.id)],
        };
        Nfa {
            start: start.id,

            end: vec![end.id],
            states: [(start.id, start), (end.id, end)].into_iter().collect(),
        }
    }

    fn any_digit(&mut self) -> Nfa {
        let end = NfaState {
            id: self.next_id(),
            transition: vec![],
        };
        let start = NfaState {
            id: self.next_id(),
            transition: vec![(StateInput::AnyDigit, end.id)],
        };
        Nfa {
            start: start.id,
            end: vec![end.id],
            states: vec![(start.id, start), (end.id, end)].into_iter().collect(),
        }
    }

    fn any_char(&mut self) -> Nfa {
        let end = NfaState {
            id: self.next_id(),
            transition: vec![],
        };

        let start = NfaState {
            id: self.next_id(),
            transition: vec![(StateInput::AnyChar, end.id)],
        };
        Nfa {
            start: start.id,
            end: vec![end.id],
            states: [(start.id, start), (end.id, end)].into_iter().collect(),
        }
    }

    fn any_char_in(&mut self, chars: Vec<char>) -> Nfa {
        let end = NfaState {
            id: self.next_id(),
            transition: vec![],
        };
        let start = NfaState {
            id: self.next_id(),
            transition: vec![((StateInput::AnyCharIn(chars), end.id))],
        };
        Nfa {
            start: start.id,
            end: vec![end.id],
            states: vec![(start.id, start), (end.id, end)].into_iter().collect(),
        }
    }

    // create new start state and epsilon transition from the new start state to the left and right nfa start states
    // All end states of the left and right nfa will be connected to the new end state
    fn or(&mut self, left: Nfa, right: Nfa) -> Nfa {
        let mut left = left;
        let mut right = right;
        let end = NfaState {
            id: self.next_id(),
            transition: vec![],
        };
        let start = NfaState {
            id: self.next_id(),
            transition: vec![
                (StateInput::Epsilon, left.start),
                (StateInput::Epsilon, right.start),
            ],
        };

        let start_id = start.id;
        let end_id = end.id;

        for end_index in left.end.iter() {
            let end_state = left.states.get_mut(end_index).unwrap();
            end_state.transition.push((StateInput::Epsilon, end.id));
        }
        for end_index in right.end.iter() {
            let end_state = right.states.get_mut(end_index).unwrap();
            end_state.transition.push((StateInput::Epsilon, end.id));
        }

        left.start = start_id;
        left.end = vec![end_id];

        left.states.extend(right.states);
        left.states.insert_nfa_state(start);
        left.states.insert_nfa_state(end);

        left
    }

    fn kleene_star(&mut self, nfa: Nfa) -> Nfa {
        let mut nfa = nfa;
        let start = nfa.start;
        for end_index in nfa.end.iter() {
            let end_state = nfa.states.get_mut(end_index).unwrap();
            end_state.transition.push((StateInput::Epsilon, start));
        }
        let start_state = nfa.states.get_mut(&start).unwrap();

        for end_index in nfa.end.iter() {
            start_state
                .transition
                .push((StateInput::Epsilon, *end_index));
        }

        nfa
    }

    fn any_char_not_in(&mut self, chars: Vec<char>) -> Nfa {
        let end = NfaState {
            id: self.next_id(),
            transition: vec![],
        };
        let start = NfaState {
            id: self.next_id(),
            transition: vec![(StateInput::AnyCharNotIn(chars), end.id)],
        };
        Nfa {
            start: start.id,
            end: vec![end.id],
            states: vec![(start.id, start), (end.id, end)].into_iter().collect(),
        }
    }

    // we will return the same nfa but will connect all end states to the start state
    fn one_or_more(&mut self, nfa: Nfa) -> Nfa {
        let mut nfa = nfa;
        let start = nfa.start;
        for end_index in nfa.end.iter() {
            let end_state = nfa.states.get_mut(end_index).unwrap();
            end_state.transition.push((StateInput::Epsilon, start));
        }
        nfa
    }

    fn sequence(&mut self, patterns: Vec<Pattern>) -> Nfa {
        let mut states = HashMap::new();
        let mut prev_end: Vec<usize> = vec![];
        let mut start: Option<usize> = None;
        if patterns.len() == 0 {
            let end = NfaState {
                id: self.next_id(),
                transition: vec![],
            };
            let start = NfaState {
                id: self.next_id(),
                transition: vec![(StateInput::Epsilon, end.id)],
            };
            let start_id = start.id;
            let end_id = end.id;
            states.insert_nfa_state(start);
            states.insert_nfa_state(end);
            return Nfa {
                start: start_id,
                end: vec![end_id],
                states,
            };
        }
        for pattern in patterns {
            let next_nfa = self.of(pattern);
            if start.is_none() {
                start = Some(next_nfa.start);
            }
            let next_start = next_nfa.start;
            states.extend(next_nfa.states);
            for end_index in prev_end.iter() {
                let end_state = states.get_mut(end_index).unwrap();
                end_state.transition.push((StateInput::Epsilon, next_start));
            }
            prev_end = next_nfa.end;
        }
        let end = prev_end;
        Nfa {
            start: start.unwrap(),
            end,
            states,
        }
    }
}

trait InsertNfaState {
    fn insert_nfa_state(&mut self, state: NfaState);
}

impl InsertNfaState for HashMap<StateId, NfaState> {
    fn insert_nfa_state(&mut self, state: NfaState) {
        if self.contains_key(&state.id) {
            panic!("State already exists: {:?}", state);
        }
        self.insert(state.id, state);
    }
}

struct NfaRunner {
    nfa: Nfa,
    current_states: Vec<StateId>,
}

impl NfaRunner {
    fn new(nfa: Nfa) -> NfaRunner {
        let start = nfa.start;
        let mut current_states = vec![start];
        NfaRunner::closure(&nfa.states, &mut current_states);
        NfaRunner {
            nfa,
            current_states,
        }
    }

    fn run(self, input: &str) -> bool {
        let mut runner = self;
        for c in input.chars() {
            runner.next(c);
        }
        runner.is_match()
    }

    fn next(&mut self, c: char) {
        let states = &self.nfa.states;
        let mut new_states = vec![];
        for state_index in &self.current_states {
            let state = states.get(&state_index).unwrap();
            for (input, next_state) in state.transition.iter() {
                match input {
                    StateInput::Literal(literal) => {
                        if literal == &c {
                            new_states.push(*next_state);
                        }
                    }
                    StateInput::AnyDigit => {
                        if c.is_digit(10) {
                            new_states.push(*next_state);
                        }
                    }
                    StateInput::AnyChar => {
                        new_states.push(*next_state);
                    }
                    StateInput::AnyCharIn(chars) => {
                        if chars.contains(&c) {
                            new_states.push(*next_state);
                        }
                    }
                    StateInput::AnyCharNotIn(chars) => {
                        if !chars.contains(&c) {
                            new_states.push(*next_state);
                        }
                    }
                    StateInput::Epsilon => {
                        // ignore eplison transitions, we will handle them later
                    }
                }
            }
        }
        NfaRunner::closure(&self.nfa.states, &mut new_states);
        self.current_states = new_states;
    }

    fn is_match(&self) -> bool {
        for state_id in self.current_states.iter() {
            if self.nfa.end.contains(state_id) {
                return true;
            }
        }
        false
    }

    fn closure(states: &HashMap<StateId, NfaState>, current: &mut Vec<usize>) {
        let mut new_states = current.clone();
        while new_states.len() > 0 {
            let mut epsilon_transitons = vec![];
            for current_state in new_states.iter() {
                let state = states.get(&current_state).unwrap();
                for (input, next_state) in state.transition.iter() {
                    if let StateInput::Epsilon = input {
                        epsilon_transitons.push(*next_state);
                    }
                }
            }
            new_states = NfaRunner::diff(&epsilon_transitons, &current);
            current.extend(epsilon_transitons);
        }
    }

    fn diff(a: &[usize], b: &[usize]) -> Vec<usize> {
        let mut diff = vec![];
        for a_item in a {
            if !b.contains(a_item) {
                diff.push(*a_item);
            }
        }
        diff
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
        assert_eq!(
            grep.is_match(),
            expected,
            "pattern: {}, input: {}",
            pattern,
            input
        );
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
        test_grep(r"\d", "apple123", true);
        test_grep(r"\w", "alpha-num3ric", true);
        test_grep("[abc]", "apple", true);
        test_grep("[^abc]", "apple", true);
        test_grep(r"\d apple", "1 apple", true);
        test_grep(r"\d apple", "x apple", false);
        test_grep("^log", "log", true);
        test_grep("^log", "1log", false);
        test_grep("dog$", "dog", true);
        test_grep("dog$", "dog1", false);
        test_grep("^dog$", "dog", true);
        test_grep("ca+ts", "caaaats", true);
    }
}
