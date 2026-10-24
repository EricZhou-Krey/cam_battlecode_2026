use crate::{action::Action, memory::AltarID};
use std::collections::VecDeque;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct ActionState {
    pub action_queue: VecDeque<Action>,

    pub buy_index: usize,
    pub last_bastion: Option<AltarID>,
}

impl ActionState {
    pub fn new() -> Self {
        Self {
            action_queue: VecDeque::new(),
            buy_index: 0,
            last_bastion: None,
        }
    }
}
