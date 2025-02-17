pub struct SidInfo {
    pub title: String,
    pub author: String,
    pub released: String,
    pub load_address: i32,
    pub load_end_address: i32,
    pub init_address: i32,
    pub play_address: i32,
    pub number_of_songs: i32,
    pub default_song: i32,

    pub clock_frequency: i32,
    pub speed_flag: i32,
    pub speed_flags: i32,

    pub stil_entry: Option<String>,
    pub md5_hash: String,
    pub song_length: i32,

    pub mus_text: [u8; 32*5],
    pub mus_colors: [u8; 32*5],

    pub file_type: String,

    pub number_of_sids: i32,
    pub sid_models: Vec<i32>,
    pub sid_addresses: Vec<i32>,

    pub free_memory_address: i32,
    pub free_memory_end_address: i32,
    pub filename: String,
    pub file_format: String,

    pub basic_sid: bool
}

impl SidInfo {
    pub fn new() -> SidInfo {
        SidInfo {
            title: "".to_string(),
            author: "".to_string(),
            released: "".to_string(),
            default_song: 0,
            number_of_songs: 0,
            clock_frequency: 0,
            speed_flag: 0,
            speed_flags: 0,
            init_address: 0,
            play_address: 0,
            load_address: 0,
            load_end_address: 0,
            stil_entry: None,
            md5_hash: "".to_string(),
            song_length: 0,

            mus_text: [0; 32*5],
            mus_colors: [0; 32*5],

            file_type: "".to_string(),
            file_format: "".to_string(),
            filename: "".to_string(),

            basic_sid: false,

            number_of_sids: 0,
            sid_models: Vec::new(),
            sid_addresses: Vec::new(),

            free_memory_address: 0,
            free_memory_end_address: 0,
        }
    }
}

impl Default for SidInfo {
    fn default() -> Self {
        Self::new()
    }
}