use cambc::Camp;

pub trait CampTypeExtension {
    fn camp_type(&self) -> CampType;
}

impl CampTypeExtension for Camp {
    fn camp_type(&self) -> CampType {
        if self.is_wolf() {
            CampType::Wolf
        } else if self.is_gem_cluster() {
            CampType::GemCluster
        } else if self.is_gold_cluster() {
            CampType::GoldCluster
        } else if self.is_elk() {
            CampType::Elk
        } else {
            CampType::Monolith
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CampType {
    Wolf,
    Elk,
    GemCluster,
    GoldCluster,
    Monolith,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CampGroupTyped {
    pub types: Vec<CampType>,
}
