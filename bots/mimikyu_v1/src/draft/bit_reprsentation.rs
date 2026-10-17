#[derive(Debug, Clone, PartialEq)]
pub struct BitInitalChampionState {
    /// 3 bytes
    pub champion_index_starting_lane: u8, // 4 champion index + 4 starting lane index
    pub ward_location: [u8; 2], // 2 bytes
}

impl BitInitalChampionState {
    pub fn as_bytes(&self) -> [u8; 3] {
        [
            self.champion_index_starting_lane,
            self.ward_location[0],
            self.ward_location[1],
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BitLaneValue {
    /// 63 bytes
    pub camps: [[u8; 2]; 10], // 20 bytes
    pub altars: [[u8; 2]; 10],      // 20 bytes
    pub camp_altar_count: u8,       // 4 bit n_camps + 4 bit n_altars
    pub weight_champion_index: u16, // 4 bit weight + 3x4 bit champion index
    pub prefix_gold_sum: [u8; 10],  // 10 bytes
    pub prefix_gem_sum: [u8; 10],   // 10 bytes
}

impl BitLaneValue {
    pub fn as_bytes(&self) -> [u8; 63] {
        let mut bytes: [u8; 63] = [0u8; 63];

        let mut offset: usize = 0;
        for camp in &self.camps {
            bytes[offset..offset + 2].copy_from_slice(camp);
            offset += 2;
        }
        for altar in &self.altars {
            bytes[offset..offset + 2].copy_from_slice(altar);
            offset += 2;
        }

        bytes[offset] = self.camp_altar_count;
        offset += 1;

        bytes[offset..offset + 2].copy_from_slice(&self.weight_champion_index.to_le_bytes());
        offset += 2;

        bytes[offset..offset + 10].copy_from_slice(&self.prefix_gold_sum);
        offset += 10;
        bytes[offset..offset + 10].copy_from_slice(&self.prefix_gem_sum);

        bytes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BitLaneSafe {
    /// 61 bytes
    pub camps: [[u8; 2]; 10], // 20 bytes
    pub altars: [[u8; 2]; 10],     // 20 bytes
    pub camp_altar_count: u8,      // 4 bit n_camps + 4 bit n_altars
    pub prefix_gold_sum: [u8; 10], // 10 bytes
    pub prefix_gem_sum: [u8; 10],  // 10 bytes
}

impl BitLaneSafe {
    pub fn as_bytes(&self) -> [u8; 61] {
        let mut bytes: [u8; 61] = [0u8; 61];

        let mut offset: usize = 0;
        for cluster in &self.camps {
            bytes[offset..offset + 2].copy_from_slice(cluster);
            offset += 2;
        }
        for cluster in &self.altars {
            bytes[offset..offset + 2].copy_from_slice(cluster);
            offset += 2;
        }

        bytes[offset] = self.camp_altar_count;
        offset += 1;

        bytes[offset..offset + 10].copy_from_slice(&self.prefix_gold_sum);
        offset += 10;
        bytes[offset..offset + 10].copy_from_slice(&self.prefix_gem_sum);

        bytes
    }
}

/// NOTE: Not constructed but used for abstraction
#[derive(Debug, Clone, PartialEq)]
pub struct BitContestingCamps {
    /// 16 bytes
    pub contesting_camps: [[u8; 2]; 8], // 16 bytes
}

impl BitContestingCamps {
    pub fn as_bytes(&self) -> [u8; 16] {
        let mut bytes: [u8; 16] = [0u8; 16];
        let mut offset: usize = 0;
        for camp in &self.contesting_camps {
            bytes[offset..offset + 2].copy_from_slice(camp);
            offset += 2;
        }
        bytes
    }
}
