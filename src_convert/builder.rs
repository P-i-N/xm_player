pub struct ChannelStream {
    data: Vec<u8>,
}

// Universal Module Builder
pub struct Builder {
    // Number of channels in the module
    num_channels: usize,

    // Number of samples
    num_samples: usize,

    // Number of instruments
    num_instruments: usize,

    channel_streams: Vec<ChannelStream>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            num_channels: 0,
            num_samples: 0,
            num_instruments: 0,
            channel_streams: Vec::new(),
        }
    }
}
