fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = prost_build::Config::new();
    config.bytes(&[
        ".sorock.CreateReq.data",
        ".sorock.ReadRep.data",
        ".sorock.SendPieceReq.data",
    ]);
    tonic_build::configure().compile_with_config(config, &["proto/sorock.proto"], &["proto"])?;
    Ok(())
}
