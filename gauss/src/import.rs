//! Bounded, schema-driven Gaussian imports. Source coordinates and SH are preserved.
use std::{
    fmt,
    fs::File,
    io::{BufRead, BufReader, Read},
    path::Path,
};

pub const MAX_GAUSSIANS: usize = 2_000_000;
pub const SH_C0: f32 = 0.2820948;
#[derive(Clone, Debug)]
pub struct Gaussian {
    pub position: [f32; 3],
    pub scale: [f32; 3],
    /// Normalized quaternion, w,x,y,z (Graphdeco and antimatter15 convention).
    pub rotation: [f32; 4],
    pub opacity: f32,
    /// Real spherical harmonics, coefficient-major RGB, Graphdeco order.
    pub sh: [[f32; 3]; 16],
}
#[derive(Clone, Debug)]
pub struct Cloud {
    pub gaussians: Vec<Gaussian>,
    pub degree: u32,
    pub source: String,
    /// Robust framing only; source values are never normalized destructively.
    pub centre: [f32; 3],
    pub radius: f32,
}
#[derive(Debug)]
pub struct ImportError(pub String);
impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ImportError {}
impl From<std::io::Error> for ImportError {
    fn from(e: std::io::Error) -> Self {
        Self(e.to_string())
    }
}
type Result<T> = std::result::Result<T, ImportError>;
fn bad(message: impl Into<String>) -> ImportError {
    ImportError(message.into())
}

impl Gaussian {
    pub fn validate(&mut self, record: usize) -> Result<()> {
        if !self
            .position
            .iter()
            .chain(&self.scale)
            .chain(&self.rotation)
            .chain(self.sh.iter().flatten())
            .chain(std::iter::once(&self.opacity))
            .all(|v| v.is_finite())
        {
            return Err(bad(format!("Gaussian {record}: non-finite value")));
        }
        if self.scale.iter().any(|&s| s <= 0.) || !(0.0..=1.0).contains(&self.opacity) {
            return Err(bad(format!(
                "Gaussian {record}: invalid positive scale or opacity"
            )));
        }
        let norm = self.rotation.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm < 1e-8 || !norm.is_finite() {
            return Err(bad(format!("Gaussian {record}: zero/invalid quaternion")));
        }
        for q in &mut self.rotation {
            *q /= norm;
        }
        if self
            .scale
            .iter()
            .any(|&v| !((v as f64) * (v as f64)).is_finite() || v > 1e15)
        {
            return Err(bad(format!(
                "Gaussian {record}: scale exceeds safe covariance range"
            )));
        }
        Ok(())
    }
    /// Columns of R diag(s). Keeping this factor (rather than only its squared
    /// covariance) preserves very thin axes through GPU perspective projection.
    pub fn axes(&self) -> [[f32; 3]; 3] {
        let [w, x, y, z] = self.rotation;
        let columns = [
            [
                1. - 2. * (y * y + z * z),
                2. * (x * y + w * z),
                2. * (x * z - w * y),
            ],
            [
                2. * (x * y - w * z),
                1. - 2. * (x * x + z * z),
                2. * (y * z + w * x),
            ],
            [
                2. * (x * z + w * y),
                2. * (y * z - w * x),
                1. - 2. * (x * x + y * y),
            ],
        ];
        std::array::from_fn(|i| columns[i].map(|v| v * self.scale[i]))
    }
    /// R diag(s²) Rᵀ, packed xx,xy,xz,yy,yz,zz.
    pub fn covariance(&self) -> [f32; 6] {
        let [w, x, y, z] = self.rotation;
        let r = [
            [
                1. - 2. * (y * y + z * z),
                2. * (x * y - w * z),
                2. * (x * z + w * y),
            ],
            [
                2. * (x * y + w * z),
                1. - 2. * (x * x + z * z),
                2. * (y * z - w * x),
            ],
            [
                2. * (x * z - w * y),
                2. * (y * z + w * x),
                1. - 2. * (x * x + y * y),
            ],
        ];
        let c = |i: usize, j: usize| {
            (0..3)
                .map(|k| r[i][k] * r[j][k] * self.scale[k] * self.scale[k])
                .sum()
        };
        [c(0, 0), c(0, 1), c(0, 2), c(1, 1), c(1, 2), c(2, 2)]
    }
}
impl Cloud {
    pub fn new(gaussians: Vec<Gaussian>, degree: u32, source: String) -> Result<Self> {
        if gaussians.is_empty() {
            return Err(bad("No Gaussians in asset"));
        }
        if degree > 3 {
            return Err(bad("Only SH degrees 0 through 3 are supported"));
        }
        if gaussians.len() > MAX_GAUSSIANS {
            return Err(bad(format!(
                "Asset exceeds {MAX_GAUSSIANS} Gaussian safety limit"
            )));
        }
        let mut axes: [Vec<f32>; 3] = std::array::from_fn(|_| Vec::new());
        let stride = (gaussians.len() / 100_000).max(1);
        for g in gaussians
            .iter()
            .step_by(stride)
            .filter(|g| g.opacity > 0.01)
        {
            for (i, a) in axes.iter_mut().enumerate() {
                a.push(g.position[i]);
            }
        }
        if axes[0].is_empty() {
            return Err(bad("Asset has no visible Gaussians"));
        }
        let mut low = [0.; 3];
        let mut high = [0.; 3];
        for i in 0..3 {
            axes[i].sort_unstable_by(f32::total_cmp);
            let n = axes[i].len();
            // Frame the dense capture, not distant sky/initialization floaters.
            // These quantiles affect only the camera; all records remain resident.
            low[i] = axes[i][n * 5 / 100];
            high[i] = axes[i][(n - 1) * 95 / 100];
        }
        let centre = std::array::from_fn(|i| low[i] * 0.5 + high[i] * 0.5);
        let radius = (0..3)
            .map(|i| ((high[i] - low[i]) * 0.5).powi(2))
            .sum::<f32>()
            .sqrt()
            .max(0.001);
        if !radius.is_finite() {
            return Err(bad("Asset bounds overflow"));
        }
        Ok(Self {
            gaussians,
            degree,
            source,
            centre,
            radius,
        })
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut file = BufReader::new(File::open(path)?);
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let mut cloud = match ext.as_str() {
            "ply" => read_ply(&mut file)?,
            "splat" => read_splat(&mut file)?,
            _ => {
                return Err(bad(
                    "Supported imports: trained Gaussian .ply and antimatter15 .splat (not SPZ/compressed PLY yet)",
                ));
            }
        };
        cloud.source = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Ok(cloud)
    }
}
fn sigmoid(x: f32) -> f32 {
    if x >= 0. {
        1. / (1. + (-x).exp())
    } else {
        let v = x.exp();
        v / (1. + v)
    }
}

pub fn read_splat(reader: &mut impl Read) -> Result<Cloud> {
    let mut gaussians = Vec::new();
    let mut bytes = [0u8; 32];
    loop {
        let mut first = [0u8; 1];
        if reader.read(&mut first)? == 0 {
            break;
        }
        bytes[0] = first[0];
        reader
            .read_exact(&mut bytes[1..])
            .map_err(|_| bad("Truncated .splat: record must be exactly 32 bytes"))?;
        if gaussians.len() == MAX_GAUSSIANS {
            return Err(bad("Splat count exceeds safety limit"));
        }
        let f = |n: usize| f32::from_le_bytes(bytes[n..n + 4].try_into().unwrap());
        let mut sh = [[0.; 3]; 16];
        for (i, c) in sh[0].iter_mut().enumerate() {
            *c = (bytes[24 + i] as f32 / 255. - 0.5) / SH_C0;
        }
        let mut g = Gaussian {
            position: [f(0), f(4), f(8)],
            scale: [f(12), f(16), f(20)],
            rotation: std::array::from_fn(|i| (bytes[28 + i] as f32 - 128.) / 128.),
            opacity: bytes[27] as f32 / 255.,
            sh,
        };
        g.validate(gaussians.len())?;
        gaussians.push(g);
    }
    Cloud::new(gaussians, 0, "splat".into())
}
#[derive(Clone, Copy)]
enum Scalar {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    F32,
    F64,
}
impl Scalar {
    fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "char" | "int8" => Self::I8,
            "uchar" | "uint8" => Self::U8,
            "short" | "int16" => Self::I16,
            "ushort" | "uint16" => Self::U16,
            "int" | "int32" => Self::I32,
            "uint" | "uint32" => Self::U32,
            "float" | "float32" => Self::F32,
            "double" | "float64" => Self::F64,
            _ => return Err(bad(format!("Unsupported PLY scalar {s}"))),
        })
    }
    fn size(self) -> usize {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::F64 => 8,
        }
    }
    fn value(self, b: &[u8], le: bool) -> f32 {
        let mut bytes = [0u8; 8];
        bytes[..b.len()].copy_from_slice(b);
        if !le {
            bytes[..b.len()].reverse();
        }
        match self {
            Self::I8 => bytes[0] as i8 as f32,
            Self::U8 => bytes[0] as f32,
            Self::I16 => i16::from_le_bytes(bytes[..2].try_into().unwrap()) as f32,
            Self::U16 => u16::from_le_bytes(bytes[..2].try_into().unwrap()) as f32,
            Self::I32 => i32::from_le_bytes(bytes[..4].try_into().unwrap()) as f32,
            Self::U32 => u32::from_le_bytes(bytes[..4].try_into().unwrap()) as f32,
            Self::F32 => f32::from_le_bytes(bytes[..4].try_into().unwrap()),
            Self::F64 => f64::from_le_bytes(bytes) as f32,
        }
    }
}
/// PLY header properties determine offsets: field order, unknown scalar fields,
/// numeric types and either byte order are supported, not hard-coded strides.
pub fn read_ply(reader: &mut impl BufRead) -> Result<Cloud> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    if line.trim_end() != "ply" {
        return Err(bad("Missing PLY magic"));
    }
    let (mut ascii, mut le, mut format) = (false, true, false);
    let (mut count, mut vertex, mut ended) = (None, false, false);
    let mut properties = Vec::new();
    let mut header_bytes = 4;
    for _ in 0..2048 {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        header_bytes += line.len();
        if header_bytes > 128 * 1024 {
            return Err(bad("PLY header too large"));
        }
        let p: Vec<_> = line.split_whitespace().collect();
        if p.is_empty() {
            continue;
        }
        match p[0] {
            "format" if p.len() == 3 && p[2] == "1.0" => {
                match p[1] {
                    "ascii" => ascii = true,
                    "binary_little_endian" => le = true,
                    "binary_big_endian" => le = false,
                    _ => return Err(bad("Unsupported PLY encoding")),
                };
                format = true;
            }
            "comment" | "obj_info" => {}
            "element" if p.len() == 3 => {
                let n = p[2]
                    .parse::<usize>()
                    .map_err(|_| bad("Invalid PLY element count"))?;
                vertex = p[1] == "vertex";
                if vertex {
                    if count.is_some() {
                        return Err(bad("Duplicate vertex element"));
                    }
                    count = Some(n);
                } else if count.is_none() && n > 0 {
                    return Err(bad("Nonempty elements preceding vertices are unsupported"));
                }
            }
            "property" if vertex => {
                if p.len() != 3 || p[1] == "list" {
                    return Err(bad("Gaussian vertex list properties are unsupported"));
                }
                if properties.len() >= 256 {
                    return Err(bad("Too many PLY properties"));
                }
                if properties.iter().any(|(name, _)| name == p[2]) {
                    return Err(bad("Duplicate PLY property"));
                }
                properties.push((p[2].to_owned(), Scalar::parse(p[1])?));
            }
            "end_header" => {
                ended = true;
                break;
            }
            _ => {}
        }
    }
    if !ended || !format {
        return Err(bad("Incomplete PLY header"));
    }
    let count = count.ok_or_else(|| bad("No vertex element"))?;
    if count > MAX_GAUSSIANS {
        return Err(bad("PLY count exceeds safety limit"));
    }
    let index = |name: &str| {
        properties.iter().position(|(n,_)|n==name).ok_or_else(||bad(format!("Missing Gaussian PLY field {name}; ordinary point clouds/compressed PLY are not Gaussian PLY")))
    };
    let pos = [index("x")?, index("y")?, index("z")?];
    let scale = [index("scale_0")?, index("scale_1")?, index("scale_2")?];
    let rot = [
        index("rot_0")?,
        index("rot_1")?,
        index("rot_2")?,
        index("rot_3")?,
    ];
    let dc = [index("f_dc_0")?, index("f_dc_1")?, index("f_dc_2")?];
    let opacity = index("opacity")?;
    let rest_count = properties
        .iter()
        .filter(|(n, _)| n.starts_with("f_rest_"))
        .count();
    let degree = match rest_count {
        0 => 0,
        9 => 1,
        24 => 2,
        45 => 3,
        _ => return Err(bad(format!("Invalid SH coefficient count: {rest_count}"))),
    };
    let rest: Vec<_> = (0..rest_count)
        .map(|i| index(&format!("f_rest_{i}")))
        .collect::<Result<_>>()?;
    let stride: usize = properties.iter().map(|(_, t)| t.size()).sum();
    let mut record = vec![0u8; stride];
    let mut values = vec![0.; properties.len()];
    let mut gaussians = Vec::with_capacity(count);
    for n in 0..count {
        if ascii {
            line.clear();
            reader.read_line(&mut line)?;
            if line.len() > 64 * 1024 {
                return Err(bad("ASCII PLY record too long"));
            }
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() != values.len() {
                return Err(bad(format!("PLY record {n}: wrong field count")));
            }
            for (v, t) in values.iter_mut().zip(fields) {
                *v = t
                    .parse()
                    .map_err(|_| bad(format!("PLY record {n}: invalid number")))?;
            }
        } else {
            reader
                .read_exact(&mut record)
                .map_err(|_| bad(format!("Truncated PLY at vertex {n}")))?;
            let mut offset = 0;
            for (v, (_, t)) in values.iter_mut().zip(&properties) {
                *v = t.value(&record[offset..offset + t.size()], le);
                offset += t.size();
            }
        }
        let mut sh = [[0.; 3]; 16];
        for c in 0..3 {
            sh[0][c] = values[dc[c]];
            for j in 0..rest_count / 3 {
                sh[j + 1][c] = values[rest[c * (rest_count / 3) + j]];
            }
        }
        let mut g = Gaussian {
            position: pos.map(|i| values[i]),
            scale: scale.map(|i| values[i].exp()),
            rotation: rot.map(|i| values[i]),
            opacity: sigmoid(values[opacity]),
            sh,
        };
        if !values[opacity].is_finite() {
            return Err(bad(format!("PLY record {n}: non-finite opacity")));
        }
        g.validate(n)?;
        gaussians.push(g);
    }
    Cloud::new(gaussians, degree, "ply".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_splat_decodes_covariance_and_rejects_truncation() {
        let mut b = Vec::new();
        for f in [1f32, 2., 3., 0.1, 0.2, 0.3] {
            b.extend(f.to_le_bytes());
        }
        b.extend([128, 64, 255, 200, 255, 128, 128, 128]);
        let c = read_splat(&mut &b[..]).unwrap();
        assert_eq!(c.gaussians[0].position, [1., 2., 3.]);
        assert!((c.gaussians[0].covariance()[0] - 0.01).abs() < 1e-6);
        assert!((SH_C0 * c.gaussians[0].sh[0][1] + 0.5 - 64. / 255.).abs() < 1e-6);
        assert!(read_splat(&mut &b[..31]).is_err());
    }
    fn ply(format: &str) -> String {
        format!(
            "ply\nformat {format} 1.0\nelement vertex 1\nproperty float z\nproperty uchar extra\nproperty float x\nproperty float y\nproperty float scale_0\nproperty float scale_1\nproperty float scale_2\nproperty float rot_0\nproperty float rot_1\nproperty float rot_2\nproperty float rot_3\nproperty float opacity\nproperty float f_dc_0\nproperty float f_dc_1\nproperty float f_dc_2\nend_header\n"
        )
    }
    #[test]
    fn ply_property_order_logscale_sigmoid_and_binary_endianness() {
        for le in [true, false] {
            let mut b = ply(if le {
                "binary_little_endian"
            } else {
                "binary_big_endian"
            })
            .into_bytes();
            for (i, f) in [
                3f32, 1., 2., -2., -1., 0., 2., 0., 0., 0., 0., 0.1, 0.2, 0.3,
            ]
            .into_iter()
            .enumerate()
            {
                if i == 1 {
                    b.push(7);
                }
                b.extend(if le { f.to_le_bytes() } else { f.to_be_bytes() });
            }
            let c = read_ply(&mut &b[..]).unwrap();
            let g = &c.gaussians[0];
            assert_eq!(g.position, [1., 2., 3.]);
            assert_eq!(g.rotation, [1., 0., 0., 0.]);
            assert_eq!(g.opacity, 0.5);
            assert!((g.scale[0] - (-2f32).exp()).abs() < 1e-6);
        }
    }
    #[test]
    fn scale_rotation_factors_reconstruct_full_covariance() {
        let mut g = Gaussian {
            position: [0.; 3],
            scale: [0.8, 0.03, 0.0004],
            rotation: [0.6, -0.2, 0.3, 0.7],
            opacity: 0.8,
            sh: [[0.; 3]; 16],
        };
        g.validate(0).unwrap();
        let factors = g.axes();
        let covariance = g.covariance();
        for ((i, j), want) in [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 2)]
            .into_iter()
            .zip(covariance)
        {
            let got = (0..3).map(|k| factors[k][i] * factors[k][j]).sum::<f32>();
            assert!(
                (got - want).abs() < 1e-6,
                "factor covariance {i},{j}: {got} != {want}"
            );
        }
    }
    #[test]
    fn ascii_and_invalid_inputs() {
        let p = ply("ascii") + "3 7 1 2 -2 -1 0 1 0 0 0 0 .1 .2 .3\n";
        assert!(read_ply(&mut p.as_bytes()).is_ok());
        assert!(
            read_ply(
                &mut "ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nend_header\n0\n"
                    .as_bytes()
            )
            .is_err()
        );
        let p = p.replace("-2 -1 0", "NaN -1 0");
        assert!(read_ply(&mut p.as_bytes()).is_err());
        assert!(read_splat(&mut &b""[..]).is_err());
    }
}
