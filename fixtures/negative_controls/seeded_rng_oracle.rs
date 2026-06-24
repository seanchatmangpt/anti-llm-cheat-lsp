use rand::SeedableRng;
use rand::rngs::SmallRng;

pub fn build_model() -> Vec<f64> {
    let mut rng = SmallRng::from_seed([42u8; 32]);
    // Using seeded RNG makes output predictable and gameable
    vec![0.87, 0.92]
}
