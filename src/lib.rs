use std::collections::VecDeque;

pub mod driver;

pub type Samples = VecDeque<Sample>;

#[derive(Clone, Copy, serde::Deserialize)]
pub struct Sample {
    pub _time: u128,
    pub value: f64,
}
