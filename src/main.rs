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
use hexasphere::shapes::IcoSphere;
use byteorder::{BigEndian, WriteBytesExt};
use fastnoise_lite::{FastNoiseLite, NoiseType};
use std::io::Write;

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
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

struct Attractor {
    pos: Vec3,
    strength: f32,
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

pub struct MeshData {
    pub vertices: Vec<Vec3>,
    pub faces: Vec<[u32; 3]>,
}

struct Noise {
    nx: FastNoiseLite,
}

impl Noise {
    fn new() -> Self {
        let mut nx = FastNoiseLite::new();
        nx.set_noise_type(Some(NoiseType::Perlin));
        nx.set_frequency(Some(0.3));
        nx.set_seed(Some(42));
        Self { nx }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cache_path = "cache/song.analysis";
    

    let audio_data = if Path::new(cache_path).exists() {
        load_data(cache_path)?
    } else {
        let path = Path::new("assets/audmesh_trim.mp3");
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
    let mut attractors = Vec::new();

    let ga = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
    let n = 32usize;
    for i in 0..n {
        let y = 1.0 - (i as f32 / (n as f32 - 1.0)) * 2.0;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let theta = ga * i as f32;
        attractors.push(Attractor {pos: Vec3::new(theta.cos() * r, y, theta.sin() * r), strength: 0.0 });
    }
    let sphere = IcoSphere::new(60, |point| {Vec3::new(point.x, point.y, point.z)});   
    let mut b_ver = Vec::new();
    let mut n = Vec::new();
    for p in sphere.raw_points() {
        let v = Vec3::new(p.x, p.y, p.z);
        b_ver.push(v);
        n.push(v.normalize());
    }
    let i = sphere.get_all_indices();
    let mut faces = Vec::new();
    for j in i.chunks_exact(3) {
        faces.push([j[0], j[1], j[2]]);
    }
    let mesh_data = MeshData {
        vertices: b_ver.clone(),
        faces: faces,
    };
    export_obj(&mesh_data, "base_mesh.obj").unwrap();
    println!("base_mesh.obj generated!");

    
    let mut anim_fs = Vec::new();
    let mut prev_ver = b_ver.clone();
    let curl = Noise::new();
    let mut strength = [0.0f32; 32];

    for (f, k) in audio_data.frames.iter().enumerate() {
        for i in 0..32 {
            let raw = k.bands[i];
            let a = if raw > strength[i] { 0.4 } else { 0.1 };
            strength[i] = strength[i] * (1.0 - a) + raw * a;
            attractors[i].strength = strength[i];
        }

        let mut curr_ver = vec![Vec3::zero(); b_ver.len()];
        curr_ver.copy_from_slice(&b_ver);
        deform(&mut curr_ver, &b_ver, &n, &attractors, &curl, f * 0.042);
        for i in 0..curr_ver.len() {
            curr_ver[i] = prev_ver[i].mul(0.7).add(curr_ver[i].mul(0.3));
        }
        prev_ver = curr_ver.clone();
        anim_fs.push(curr_ver);
    }

    export_mdd(&anim_fs, audio_data.fps as f32, "animation.mdd").unwrap();
    println!("animation.mdd done!");
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

fn deform(vertices: &mut [Vec3], b_ver: &[Vec3], n: &[Vec3], attractors: &[Attractor], noise: &Noise, t: f32) {
    let sigma_sq = 0.18f32 * 0.18;
    let h = 0.28f32;
    let amt = 0.18f32;
    for (i, vertex) in vertices.iter_mut().enumerate() {
        let base  = b_ver[i];
        let normal = n[i];
        let mut ds = 0.0f32;
        for attractor in attractors {
            if attractor.strength < 0.005 { continue; }
            let dot = (base.x * attractor.pos.x + base.y * attractor.pos.y + base.z * attractor.pos.z).clamp(-1.0, 1.0);
            let d = 1.0 - dot;
            ds += (-d / (2.0 * sigma_sq)).exp() * attractor.strength * h;
        }
        let nx = noise.nx.get_noise_3d(base.x * 3.0, base.y * 3.0, t);
        ds *= 1.0 + nx * amt;
        let ds = ds.clamp(0.0, 0.35);
        *vertex = base.add(normal.mul(ds));
    }
}

fn export_obj(mesh: &MeshData, path: &str) -> Result<(), std::io::Error> {
    let mut f = File::create(path)?;
    for i in &mesh.vertices {
        writeln!(f, "v {} {} {}", i.x, i.y, i.z)?;
    }
    for i in &mesh.faces {
        writeln!(f, "f {} {} {}", i[0] + 1, i[1] + 1, i[2] + 1)?;
    }
    Ok(())
}

fn export_mdd(frames: &[Vec<Vec3>], fps: f32, path: &str) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    f.write_u32::<BigEndian>(frames.len() as u32)?;
    f.write_u32::<BigEndian>(frames[0].len() as u32)?;
    for i in 0..frames.len() {
        f.write_f32::<BigEndian>(i as f32 / fps)?;
    }
    for i in frames {
        for j in i {
            f.write_f32::<BigEndian>(j.x)?;
            f.write_f32::<BigEndian>(j.y)?;
            f.write_f32::<BigEndian>(j.z)?;
        }
    }
    Ok(())
}