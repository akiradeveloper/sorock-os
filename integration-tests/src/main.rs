use cmd_lib::*;
fn main() -> anyhow::Result<()> {
    run_cmd!(docker-compose down -v)?;
    run_cmd!(docker-compose up -d)?;
    run_cmd!(docker-compose down -v)?;
    Ok(())
}
