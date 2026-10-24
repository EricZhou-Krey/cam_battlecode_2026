use crate::memory::champion_memory::ChampionMemory;

#[derive(Debug, Default, Clone, PartialEq)]
pub enum Action {
    #[default]
    Idle,
}

//precedence value for all commands, don't poll for command every turn just when needed, when idle

impl Action {
    fn try_prempt_current_action(&mut self) {
        // TODO
    }
    pub fn turn(&mut self, champion_memory: &mut ChampionMemory) {
        // TODO
    }
}
