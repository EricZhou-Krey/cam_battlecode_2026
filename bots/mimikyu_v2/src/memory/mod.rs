pub mod champion_memory;
pub mod champion_states;
pub mod map_cache;
pub mod message;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampGroupID(pub usize);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampID(pub usize);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AltarID(pub usize);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LaneID(pub usize);
