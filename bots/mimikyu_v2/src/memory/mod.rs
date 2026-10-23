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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocationType {
    CampGroup,
    Altar,
    Shortcut,
    ControlPoint,
}

impl LocationType {
    const CAMP_GROUP: u8 = Self::CampGroup as u8;
    const ALTAR: u8 = Self::Altar as u8;
    const SHORTCUT: u8 = Self::Shortcut as u8;
    const CONTROL_POINT: u8 = Self::ControlPoint as u8;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocationID {
    CampGroup(CampGroupID),
    Altar(AltarID),
    Shortcut(ShortcutID),
    ControlPoint(ControlPointID),
}

const LOCATION_TYPE_MASK: u8 = 0b1100_0000;
const LOCATION_ID_MASK: u8 = 0b0011_1111;

impl LocationID {
    pub fn as_byte(&self) -> u8 {
        let (location_type, id): (u8, u8) = match self {
            Self::CampGroup(id) => (LocationType::CAMP_GROUP, id.0 as u8),
            Self::Altar(id) => (LocationType::ALTAR, id.0 as u8),
            Self::Shortcut(id) => (LocationType::SHORTCUT, id.0 as u8),
            Self::ControlPoint(id) => (LocationType::CONTROL_POINT, id.0 as u8),
        };

        (location_type << 6) | id
    }

    pub fn from_byte(byte: u8) -> Self {
        let tag: u8 = (byte & LOCATION_TYPE_MASK) >> 6;
        let id: usize = (byte & LOCATION_ID_MASK) as usize;

        match tag {
            LocationType::CAMP_GROUP => Self::CampGroup(CampGroupID(id)),
            LocationType::ALTAR => Self::Altar(AltarID(id)),
            LocationType::SHORTCUT => Self::Shortcut(ShortcutID(id)),
            LocationType::CONTROL_POINT => Self::ControlPoint(ControlPointID(id)),
            _ => unreachable!(),
        }
    }
}
