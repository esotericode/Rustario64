//! Bitwise differential tests for math_util: Rust port vs the pinned decomp C.
//! CI uses authored table contents; the ignored test uses the owner ROM's tables.
use rustario64::simulation::math::{
    ARCTAN_ENTRIES, SINE_ENTRIES, TrigTables, approach_f32, approach_s32,
};
use rustario64_oracle::MathOracle;

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn f(&mut self, scale: f32) -> f32 {
        (self.next() as f32 / u32::MAX as f32 - 0.5) * 2.0 * scale
    }
}

/// Authored, not game data: arbitrary bit patterns and monotone arctan values.
fn authored_tables() -> TrigTables {
    let mut rng = Lcg(77);
    let sine = (0..SINE_ENTRIES)
        .map(|_| f32::from_bits(rng.next() & 0xBFFF_FFFF))
        .collect();
    let arctan = (0..ARCTAN_ENTRIES)
        .map(|i| (i as u16).wrapping_mul(9) ^ 0x55)
        .collect();
    TrigTables::new(sine, arctan).unwrap()
}

fn compare(tables: &TrigTables) -> usize {
    let oracle = MathOracle::new(tables.sine_table(), tables.arctan_table());
    let mut n = 0;
    for x in (-0x10000..0x20000).step_by(1) {
        assert_eq!(
            tables.sins(x).to_bits(),
            oracle.sins(x).to_bits(),
            "sins {x}"
        );
        assert_eq!(
            tables.coss(x).to_bits(),
            oracle.coss(x).to_bits(),
            "coss {x}"
        );
        n += 2;
    }
    let specials = [
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.5,
        -0.5,
        1e-30,
        -1e-30,
        3.0e38,
        -3.0e38,
        1024.0,
        4096.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ];
    let mut points = vec![];
    for &y in &specials {
        for &x in &specials {
            if !(y.is_infinite() && x.is_infinite()) {
                points.push((y, x));
            }
        }
    }
    let mut rng = Lcg(5);
    for i in 0..400_000 {
        let scale = [1.0, 100.0, 8192.0, 1e-3][i % 4];
        let (y, x) = (rng.f(scale), rng.f(scale));
        points.push((y, x));
        points.push((x, x));
        points.push((-x, x));
        points.push((y, y * 0.999_999));
    }
    for (y, x) in points {
        assert_eq!(tables.atan2s(y, x), oracle.atan2s(y, x), "atan2s({y}, {x})");
        assert_eq!(
            tables.atan2f(y, x).to_bits(),
            oracle.atan2f(y, x).to_bits(),
            "atan2f({y}, {x})"
        );
        n += 2;
    }
    let mut rng = Lcg(9);
    for _ in 0..200_000 {
        let a = rng.next() as i32;
        let t = rng.next() as i32;
        let (inc, dec) = ((rng.next() >> 8) as i32, (rng.next() >> 8) as i32);
        assert_eq!(
            approach_s32(a, t, inc, dec),
            oracle.approach_s32(a, t, inc, dec)
        );
        let (c, t, inc, dec) = (rng.f(5000.0), rng.f(5000.0), rng.f(80.0), rng.f(80.0));
        assert_eq!(
            approach_f32(c, t, inc, dec).to_bits(),
            oracle.approach_f32(c, t, inc, dec).to_bits()
        );
        n += 2;
    }
    n
}

#[test]
fn authored_tables_match_decomp_math_util() {
    let n = compare(&authored_tables());
    assert!(n > 2_000_000, "{n}");
}

#[test]
#[ignore = "requires a privately supplied supported ROM via RUSTARIO64_ROM"]
fn rom_trig_tables_match_decomp_math_util() {
    use rustario64::import::{engine, rom::Rom};
    let path = std::env::var_os("RUSTARIO64_ROM").expect("set RUSTARIO64_ROM");
    let rom = Rom::open(std::path::Path::new(&path)).unwrap();
    let tables = engine::trig_tables(&rom).unwrap();
    // Spot values with known closed forms; table contents stay in the ROM.
    assert_eq!(tables.sins(0), 0.0);
    assert_eq!(tables.coss(0), 1.0);
    assert_eq!(tables.sins(0x4000), 1.0);
    assert_eq!(tables.atan2s(1.0, 1.0), 0x2000);
    let n = compare(&tables);
    println!("{n} math_util comparisons identical with ROM tables");
}
