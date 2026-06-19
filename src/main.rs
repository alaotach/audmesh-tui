use std::fs::File;
use std::path::Path;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::formats::FormatOptions;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::get_probe;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::default::get_codecs;

pub struct AudioData {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new("assets/audmesh.mp3");
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let hint = Hint::new();
    let probed = get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())?;
    let mut format = probed.format;
    let track = format.default_track().unwrap();
    let mut decoder = get_codecs().make(&track.codec_params, &DecoderOptions::default())?;
    let mut samples = Vec::new();
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2) as u16;
    loop {
        let pc = match format.next_packet() {
            Ok(packet) => packet,
            Err(_) => break,
        };
        let decoded = match decoder.decode(&pc) {
            Ok(decoded) => decoded,
            Err(_) => continue,
        };
        let mut sample_buff = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        sample_buff.copy_interleaved_ref(decoded);
        samples.extend_from_slice(sample_buff.samples());
    }
    // println!("Samples {}", samples.len());
    println!("Sample Rate: {}", sample_rate);
    println!("Channels: {}", channels);
    // let audio_data = AudioData {
    //     sample_rate,
    //     channels,
    //     samples: samples.iter().map(|s| (*s * i16::MAX as f32) as i16).collect(),
    // };
    // println!("{:?}", track.id);
    // println!("{:?}", track.codec_params.codec);
    // println!("{:?}", track.codec_params.sample_rate);
    // println!("{:?}", track.codec_params.channels);

    Ok(())
}