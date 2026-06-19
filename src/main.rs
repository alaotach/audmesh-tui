use std::fs::File;
use std::path::Path;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::formats::FormatOptions;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::default::get_probe;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new("assets/audmesh.mp3");
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let hint = Hint::new();
    let probed = get_probe().format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())?;
    let format = probed.format;
    let track = format.default_track().unwrap();
    println!("{:?}", track.id);
    println!("{:?}", track.codec_params.codec);
    println!("{:?}", track.codec_params.sample_rate);
    println!("{:?}", track.codec_params.channels);

    Ok(())
}