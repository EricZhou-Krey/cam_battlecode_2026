pub mod champion_info;
pub mod champion_memory;
pub mod lazy_map;
pub mod message;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampGroupID(pub usize);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampID(pub usize);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AltarID(pub usize);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortcutID(pub usize);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlPointID(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocationID {
    CampGroup(CampGroupID),
    Altar(AltarID),
    Shortcut(ShortcutID),
    ControlPoint(ControlPointID),
}
