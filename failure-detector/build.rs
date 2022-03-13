fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = prost_build::Config::new();
    tonic_build::configure().compile_with_config(config, &["proto/fd.proto"], &["proto"])?;
    Ok(())
}
