use std::collections::HashMap;
use crate::structs::channel::Channel;

pub struct SFUManager {
    // Key: ChannelID (from .NET), Value: The Channel object
    pub channels: HashMap<String, Channel>,
}

impl SFUManager {
    pub fn new() -> Self {
        Self { channels: HashMap::new() }
    }
}