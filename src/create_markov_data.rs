use std::collections::HashMap;
use std::{fs, mem};
use indexmap::IndexMap;
use lindera_core::mode::Mode;
use lindera_dictionary::{DictionaryConfig, DictionaryKind};
use lindera_tokenizer::tokenizer::{Tokenizer, TokenizerConfig};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize};
use tokenizer::Tokenize;
use tokenizer_generator::tokenizer;
use crate::markov_lib::{FunctionParamValue, MarkovData, MarkovToken};

mod markov_lib;

#[derive(Debug, Clone)]
enum MkToken {
    Emoji(String),
    FunctionOpen { all: String, name: String, args: IndexMap<String, Option<String>> },
    FunctionClose,
    Char(String),
}

tokenizer! {
    fn get_tokenizer() -> DFATokenizer {
        character char;
        token MkToken;
        ":[a-zA-Z0-9_]+:": |s, _| MkToken::Emoji(s.to_string());
        "\\$\\[[a-zA-Z0-9]+(\\.[a-zA-Z0-9]+(=[a-zA-Z0-9\\-.]+)?(,[a-zA-Z0-9]+(=[a-zA-Z0-9\\-.]+)?)*)?\\s": |s, _| parse_function_open(s);
        "\\]": |_, _| MkToken::FunctionClose;
        ".|\\n": |s, _| MkToken::Char(s.to_owned());
    }
}

#[derive(Deserialize)]
struct Note {
    text: Option<String>,
}

fn parse_function_open(s: &str) -> MkToken {
    static FUNCTION_OPEN_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new("^\\$\\[(?<function_name>[a-zA-Z0-9]+)(?:\\.(?<params>[a-zA-Z0-9]+(?:=[a-zA-Z0-9\\-.]+)?(?:,[a-zA-Z0-9]+(?:=[a-zA-Z0-9\\-.]+)?)*))?\\s$").unwrap());
    static FUNCTION_PARAMS_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new("[a-zA-Z0-9]+(?:=[a-zA-Z0-9\\-.]+)?").unwrap());
    let captures = FUNCTION_OPEN_REGEX.captures(s).unwrap();
    let name = captures["function_name"].to_owned();
    let args = captures.name("params").map(|params| {
        FUNCTION_PARAMS_REGEX.captures_iter(params.as_str()).map(|captures| {
            let s = &captures[0];
            if s.contains('=') {
                let mut iter = s.split('=');
                (iter.next().unwrap().to_owned(), Some(iter.next().unwrap().to_owned()))
            } else {
                (s.to_owned(), None)
            }
        }).collect()
    }).unwrap_or_default();
    MkToken::FunctionOpen { all: s.to_string(), name, args }
}

fn main() {
    let notes = serde_json::from_slice::<Vec<Note>>(&fs::read("all_notes.json").unwrap()).unwrap().into_iter().filter_map(|Note { text }| text).collect::<Vec<_>>();

    let dfa_tokenizer = get_tokenizer();
    let dictionary = DictionaryConfig {
        kind: Some(DictionaryKind::IPADIC),
        path: None,
    };
    let config = TokenizerConfig {
        dictionary,
        user_dictionary: None,
        mode: Mode::Normal,
    };
    let lindera_tokenizer = Tokenizer::from_config(config).unwrap();

    let mut token_map = HashMap::new();
    let mut all_functions = Vec::new();

    for note in notes {
        let tokens = note.chars().tokenize(&dfa_tokenizer).scan(0usize, |function_open_count, token| {
            match token {
                token @ (MkToken::Emoji(_) | MkToken::Char(_)) => Some(token),
                token @ MkToken::FunctionOpen { .. } => {
                    *function_open_count += 1;
                    Some(token)
                }
                MkToken::FunctionClose => {
                    if *function_open_count > 0 {
                        *function_open_count -= 1;
                        Some(MkToken::FunctionClose)
                    } else {
                        Some(MkToken::Char("]".to_owned()))
                    }
                }
            }
        }).collect::<Vec<_>>();
        let tokens = tokens.into_iter().rev().scan(0usize, |function_close_count, token| {
            match token {
                token @ (MkToken::Emoji(_) | MkToken::Char(_)) => Some(token),
                MkToken::FunctionOpen { all, name, args } => {
                    if *function_close_count > 0 {
                        *function_close_count -= 1;
                        Some(MkToken::FunctionOpen { all, name, args })
                    } else {
                        Some(MkToken::Char(all))
                    }
                }
                MkToken::FunctionClose => {
                    *function_close_count += 1;
                    Some(MkToken::FunctionClose)
                }
            }
        }).collect::<Vec<_>>();
        let tokens = tokens.into_iter().rev().fold(Vec::new(), |mut acc, token| {
            match token {
                MkToken::Char(s) => {
                    if let Some(MkToken::Char(acc)) = acc.last_mut() {
                        acc.push_str(&s);
                    } else {
                        acc.push(MkToken::Char(s));
                    }
                }
                token => acc.push(token),
            }
            acc
        });
        let mut function_stack = Vec::new();
        let mut prev_token = MarkovToken::Start;
        for token in tokens {
            match token {
                MkToken::Emoji(emoji) => {
                    *token_map.entry((mem::replace(&mut prev_token, MarkovToken::String(emoji.clone())), MarkovToken::String(emoji))).or_insert(0usize) += 1;
                }
                MkToken::FunctionOpen { name, args, .. } => {
                    function_stack.push(name.clone());
                    *token_map.entry((mem::replace(&mut prev_token, MarkovToken::FunctionStart(name.clone())), MarkovToken::Function(name.clone()))).or_insert(0usize) += 1;
                    all_functions.push((name, args));
                }
                MkToken::FunctionClose => {
                    *token_map.entry((mem::replace(&mut prev_token, MarkovToken::Function(function_stack.pop().unwrap())), MarkovToken::End)).or_insert(0usize) += 1;
                }
                MkToken::Char(s) => {
                    for x in lindera_tokenizer.tokenize(&s).unwrap() {
                        *token_map.entry((mem::replace(&mut prev_token, MarkovToken::String(x.text.to_owned())), MarkovToken::String(x.text.to_owned()))).or_insert(0usize) += 1;
                    }
                }
            };
        }
        *token_map.entry((prev_token, MarkovToken::End)).or_insert(0usize) += 1;
    }
    let mut function_used_counts = HashMap::new();
    for (name, _) in &all_functions {
        *function_used_counts.entry(name.clone()).or_insert(0usize) += 1;
    }
    let mut function_param_map = HashMap::new();
    for (name, params) in all_functions {
        for (key, value) in params {
            *function_param_map.entry((name.clone(), key.clone(), FunctionParamValue::None)).or_insert_with(|| function_used_counts[&name]) -= 1;
            *function_param_map.entry((name.clone(), key, value.map_or(FunctionParamValue::ValueIsNull, FunctionParamValue::Value))).or_insert(0usize) += 1;
        }
    }

    let markov_data = MarkovData {
        token_map: token_map.into_iter().map(|((a, b), c)| (a, b, c)).collect(),
        function_param_map: function_param_map.into_iter().map(|((a, b, c), d)| (a, b, c, d)).collect(),
    };
    fs::write("markov_data.json", serde_json::to_string(&markov_data).unwrap()).unwrap();
}
