use bot::strategy::Strategy;
use cambc::Result;

fn main() -> Result<()> {
    Strategy::determine_strategy().enter();
    Ok(())
}
