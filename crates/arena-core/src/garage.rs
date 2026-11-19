//! The Garage: chassis classes and cosmetics a player brings into a match.

use serde::{Deserialize, Serialize};

use crate::parts::Slot;

/// Chassis class of a player's tank.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChassisClass {
    /// Light and fast, thin-skinned, nitro from the start, fewer hardpoints (no Front
    /// or Skirts slots, so no Bulwark or Spaced Armour).
    Scout,
    /// The all-rounder (the default tank).
    #[default]
    Brawler,
    /// Heavy and slow, thick armour; its crew bolts parts on with fewer scrap.
    Hauler,
}

impl ChassisClass {
    pub const ALL: [ChassisClass; 3] = [ChassisClass::Scout, ChassisClass::Brawler, ChassisClass::Hauler];

    /// Can parts in this slot be fitted?
    pub fn allows(self, slot: Slot) -> bool {
        !(self == ChassisClass::Scout && matches!(slot, Slot::Front | Slot::Skirts))
    }
}

/// What a player picked in the Garage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Loadout {
    pub chassis: ChassisClass,
    /// Paint scheme index (the client's palette).
    pub paint: u8,
    /// Crew emblem index.
    pub emblem: u8,
    /// Horn sound index.
    pub horn: u8,
}
