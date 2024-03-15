use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum MarkovToken {
    Start,
    String(String),
    FunctionStart(String),
    Function(String),
    End,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Hash, Deserialize, Serialize)]
pub enum FunctionParamValue {
    None,
    ValueIsNull,
    Value(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MarkovData {
    pub token_map: Vec<(MarkovToken, MarkovToken, usize)>,
    pub function_param_map: Vec<(String, String, FunctionParamValue, usize)>,
}
