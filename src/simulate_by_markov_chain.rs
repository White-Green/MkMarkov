use std::collections::{BTreeMap, HashMap};
use std::fs;
use rand::prelude::SliceRandom;
use rand::{Rng, thread_rng};
use crate::markov_lib::{FunctionParamValue, MarkovData, MarkovToken};

mod markov_lib;

fn main() {
    let MarkovData {
        token_map,
        function_param_map,
    } = serde_json::from_slice::<MarkovData>(&fs::read("markov_data.json").unwrap()).unwrap();
    let token_map = token_map.into_iter().map(|(a, b, count)| ([a, b], count)).collect::<BTreeMap<_, _>>();
    let mut map = HashMap::new();
    for (function_name, param_name, value, count) in function_param_map {
        map.entry(function_name).or_insert_with(HashMap::new).entry(param_name).or_insert_with(Vec::new).push((value, count));
    }
    let str = sim(MarkovToken::Start, &token_map, &map, 10, &mut thread_rng());
    println!("{str}");
}

fn sim(first_token: MarkovToken, token_map: &BTreeMap<[MarkovToken; 2], usize>, function_param_map: &HashMap<String, HashMap<String, Vec<(FunctionParamValue, usize)>>>, limited_depth: usize, rng: &mut impl Rng) -> String {
    let mut result = String::new();
    let mut current_token = first_token;
    loop {
        let tokens_iter = token_map.range([current_token.clone(), MarkovToken::Start]..).take_while(|([first, _], _)| first == &current_token)
            .map(|([prev, token], count)| (prev, token, *count));
        let tokens = if limited_depth == 0 {
            tokens_iter.filter(|(_, token, _)| !matches!(token, MarkovToken::Function(_))).collect::<Vec<_>>()
        } else {
            tokens_iter.collect()
        };
        let Ok(&(prev, token, _)) = tokens.choose_weighted(rng, |&(_, _, count)| count) else { return result; };
        eprintln!("{prev:?} - {token:?}");
        match token {
            MarkovToken::Start | MarkovToken::FunctionStart(_) => unreachable!(),
            MarkovToken::String(s) => result.push_str(&s),
            MarkovToken::Function(name) => {
                if limited_depth == 0 { continue; }
                result.push_str("$[");
                result.push_str(name);
                if let Some(params) = function_param_map.get(name) {
                    let mut parameters_iter = params.iter().filter_map(|(k, v)| {
                        let (value, _) = v.choose_weighted(rng, |&(_, count)| count).unwrap();
                        match value {
                            FunctionParamValue::None => None,
                            FunctionParamValue::ValueIsNull => Some(k.clone()),
                            FunctionParamValue::Value(v) => Some(format!("{k}={v}")),
                        }
                    });
                    if let Some(p) = parameters_iter.next() {
                        result.push('.');
                        result.push_str(&p);
                        for p in parameters_iter {
                            result.push(',');
                            result.push_str(&p);
                        }
                    }
                }
                result.push(' ');
                let f = sim(MarkovToken::FunctionStart(name.clone()), token_map, function_param_map, limited_depth - 1, rng);
                result.push_str(&f);
                result.push(']');
            }
            MarkovToken::End => return result,
        }
        current_token = token.clone();
    }
}
