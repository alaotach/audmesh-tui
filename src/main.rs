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
use rustfft::{num_complex::Complex, FftPlanner};
use serde::{Serialize, Deserialize};
use std::fs;
use rand::Rng;

#[derive(Serialize, Deserialize)]
pub struct AudioData {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
    pub fps: u32,
    pub frames: Vec<Frames>,
}

#[derive(Serialize, Deserialize)]
pub struct FFTbands{
    pub bands: [f32; 32],
}

#[derive(Serialize, Deserialize)]
pub struct Frames{
    pub time: f32,
    pub bands: [f32; 32],
}

#[derive(Clone, Copy, Debug)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

struct Particle {
    pos: Vec3,
    vel: Vec3,
}

struct Attractor {
    pos: Vec3,
    strength: f32,
}

struct ParticleFrame {
    positions: Vec<Vec3>,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
    pub fn add(self, other: Vec3) -> Vec3 {
        Vec3 {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }

    pub fn sub(self, other: Vec3) -> Vec3 {
        Vec3 {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }

    pub fn mul(self, num: f32) -> Vec3 {
        Vec3 {
            x: self.x * num,
            y: self.y * num,
            z: self.z * num,
        }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn normalize(self) -> Vec3 {
        let len = self.length();
        if len == 0.0 {
            return Vec3::zero();
        }
        self.mul(1.0 / len)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache_path = "cache/song.analysis";
    

    let audio_data = if Path::new(cache_path).exists() {
        load_data(cache_path)?
    } else {
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
        // println!("Sample Rate: {}", sample_rate);
        // println!("Channels: {}", channels);
        let mut frames = Vec::new();
        let size = 2048;
        let samples_chunk = ((sample_rate as f32 * channels as f32) / 24.0) as usize;
        let mut pos = 0;
        while pos + size < samples.len() {
            let bands = analyze_fft(&samples[pos..pos + size], sample_rate, channels);
            frames.push(Frames { 
                time: pos as f32 / (sample_rate as f32 * channels as f32),
                bands: bands.bands,
            });
            pos += samples_chunk;
        }
        println!("Frames: {}", frames.len());
        let mut max_bands = [0.0f32; 32];
        for f in &frames {
            for i in 0..32 {
                max_bands[i] = max_bands[i].max(f.bands[i]);
            }
        }
        for f in &mut frames {
            for i in 0..32 {
                if max_bands[i] > 0.0 {
                    f.bands[i] /= max_bands[i];
                }
            }
        }
        // for i in 0..10 {
        //     println!("{}", frames[200].bands[i]);
        // }
        let audio_data = AudioData {
            sample_rate,
            channels,
            samples,
            fps: 24,
            frames,
        };
        std::fs::create_dir_all("cache")?;
        save_data(cache_path, &audio_data)?;
        audio_data
    };
    let mut particles = gen_particles(100);
    let mut sim_frames = Vec::new();
    let frame = &audio_data.frames[200];
    let mut attractors = Vec::new();

    for i in 0..32 {
        let angle =
            i as f32 / 32.0 * std::f32::consts::TAU;
        let phi = i as f32 / 32.0 * std::f32::consts::PI;
        attractors.push(
            Attractor {pos: Vec3::new(angle.cos()* phi.sin(), phi.cos(), angle.sin()* phi.cos()),
                strength: 0.0,
            }
        );
    }
    for frame in &audio_data.frames {
        for i in 0..32 {
            attractors[i].strength = frame.bands[i] * 10.0;
        }
        sim_particle(&mut particles, &attractors);
        sim_frames.push(ParticleFrame {
            positions: particles.iter().map(|p| p.pos).collect(),
        });
        if sim_frames.len() % 500 == 0 {
            let (min, max) = particle_bounds(&particles);
        }
    }
    export_obj(&particles, "cloud.obj")?;
    // println!("{:?}", track.id);
    // println!("{:?}", track.codec_params.codec);
    // println!("{:?}", track.codec_params.sample_rate);
    // println!("{:?}", track.codec_params.channels);

    Ok(())
}

fn analyze_fft(samples: &[f32], sample_rate: u32, _channels: u16) -> FFTbands {
    // let mut bass = 0.0;
    // let mut mid = 0.0;
    // let mut treble = 0.0;
    let mut bands = [0.0f32; 32];
    let mut planner = FftPlanner::new();
    let size = samples.len();
    let fft = planner.plan_fft_forward(size);
    let mut buffer: Vec<Complex<f32>> = samples.iter().map(|&s| Complex{ re: s, im: 0.0 }).collect();
    fft.process(&mut buffer);
    for (i, v) in buffer.iter().enumerate() {
        let freq = i as f32 * sample_rate as f32 / size as f32;
        let mag = v.norm();
        if let Some(band) = fqband(freq) {
            bands[band] += mag;
        }
    }
    FFTbands { bands }
}

fn fqband(freq: f32) -> Option<usize> {
    if !(20.0..20000.0).contains(&freq) {
        return None;
    }
    let min = 20.0;
    let max = 20000.0;
    let normal = (freq / min).ln() / (max / min).ln();
    Some((normal * 31.0) as usize)
}

fn save_data(path: &str, data: &AudioData,) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = bincode::serde::encode_to_vec(data, bincode::config::standard())?;
    fs::write(path, bytes)?;
    Ok(())
}

fn load_data(path: &str,) -> Result<AudioData, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let (data, _) = bincode::serde::decode_from_slice(&bytes, bincode::config::standard() )?;
    Ok(data)
}

pub fn gen_particles(count: usize) -> Vec<Particle> {
    let mut rng = rand::rng();
    let mut particles = Vec::new();
    for _ in 0..count {
        particles.push(Particle {
            pos: Vec3 {
                x: rng.random_range(-1.0..1.0),
                y: rng.random_range(-1.0..1.0),
                z: rng.random_range(-1.0..1.0),
            },
            vel: Vec3::zero(),
        });
    }
    particles
}

fn sim_particle(p: &mut Vec<Particle>, attractors: &[Attractor]) {
    for i in 0..p.len() {
        let mut force = Vec3::zero();
        for attractor in attractors {
            let dir = attractor.pos.sub(p[i].pos);
            force = force.add(dir.normalize().mul(0.01 * attractor.strength));
        }
        p[i].vel = p[i].vel.add(force);
        p[i].vel = p[i].vel.mul(0.98);
        p[i].pos = p[i].pos.add(p[i].vel);
    }
}

fn particle_bounds(particles: &[Particle]) -> (Vec3, Vec3) {
    let mut min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
    let mut max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);
    for p in particles {
        min.x = min.x.min(p.pos.x);
        min.y = min.y.min(p.pos.y);
        min.z = min.z.min(p.pos.z);
        max.x = max.x.max(p.pos.x);
        max.y = max.y.max(p.pos.y);
        max.z = max.z.max(p.pos.z);
    }
    (min, max)
}

use std::io::Write;

fn export_obj(particles: &[Particle], path: &str,) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    for p in particles {
        writeln!(file,"v {} {} {}", p.pos.x, p.pos.y, p.pos.z)?;
    }
    Ok(())
}